use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicUsize, Ordering},
    },
};

use knyst::{
    controller::KnystCommands,
    inputs,
    modal_interface::knyst_commands,
    prelude::{BlockSize, GenState, InputBundle, Sample, bus, graph_output, handle, impl_gen},
    time::Beats,
};
use num_traits::ToPrimitive;

use super::{
    InstrumentProcessorNode,
    InstrumentRuntimeHandle,
    MasterRuntime,
    MeterTap,
    MixerRuntime,
    MixerRuntimeError,
    RuntimeFactoryError,
    SharedInstrumentResetState,
    SharedStripMeter,
    StripLatency,
    compute_pdc_plan_from_latencies,
    connect_stereo,
    db_to_amplitude,
    node_id_of,
    normalize_meter_level,
};
use crate::{
    instrument::{
        BUILTIN_SOUNDFONT_ID,
        Controller,
        ControllerError,
        EffectProcessor,
        EffectRuntimeSpec,
        InstrumentProcessor,
        InstrumentRuntimeContext,
        InstrumentRuntimeSpec,
        MidiEvent,
        Processor,
        ProcessorDescriptor,
        ProcessorKind,
        ProcessorState,
        ProcessorStateError,
        RuntimeBinding,
        SlotState,
        registry,
        registry::{Entry, RuntimeFactory},
    },
    mixer::{BusSend, Mixer, MixerState, SlotAddress, TrackId, TrackRoute},
    test_utils::{OfflineHarness, test_soundfont_resource},
};

const TEST_LATENCY_EFFECT_ID: &str = "org.lilypalooza.test.latency-effect";
static TEST_INSTRUMENT_PREPARE_DESTROY_COUNT: AtomicUsize = AtomicUsize::new(0);
static TEST_EFFECT_PREPARE_DESTROY_COUNT: AtomicUsize = AtomicUsize::new(0);

fn schedule_test_note(harness: &mut OfflineHarness, handle: InstrumentRuntimeHandle) {
    let scheduled_at = harness
        .commands()
        .current_transport_snapshot()
        .and_then(|snapshot| snapshot.beats)
        .unwrap_or(Beats::ZERO)
        + Beats::from_beats_f64(0.01);
    handle.schedule_midi_at_with_offset(
        harness.commands(),
        scheduled_at,
        0,
        MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        },
    );
}

fn pause_seek_and_wait(harness: &mut OfflineHarness, beats: f64) {
    harness.commands().transport_pause();
    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats_f64(beats));
    harness.wait_for_transport_settled();
}

fn play_note_and_assert_signal(
    harness: &mut OfflineHarness,
    handle: InstrumentRuntimeHandle,
    failure: &str,
) {
    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    schedule_test_note(harness, handle);
    harness.process_blocks(16);
    assert!(harness.output_has_signal(), "{failure}");
}

fn unity_strip_gain() -> super::SharedStripLevel {
    super::SharedStripLevel::new(1.0, 0.0)
}

fn assert_f32_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= f32::EPSILON,
        "expected {actual} to equal {expected}"
    );
}

fn soundfont_slot(program: u8) -> SlotState {
    register_test_soundfont_builtin();
    SlotState::built_in(BUILTIN_SOUNDFONT_ID, ProcessorState(vec![program]))
}

fn latency_effect_slot() -> SlotState {
    register_test_latency_effect_builtin();
    SlotState::built_in(TEST_LATENCY_EFFECT_ID, ProcessorState::default())
}

fn register_test_soundfont_builtin() {
    registry::register([Entry::built_in_processor(
        BUILTIN_SOUNDFONT_ID,
        "SoundFont",
        test_instrument_descriptor(),
        RuntimeFactory::Instrument(create_test_soundfont_runtime),
    )]);
}

fn register_test_latency_effect_builtin() {
    registry::register([Entry::built_in_processor(
        TEST_LATENCY_EFFECT_ID,
        "Latency",
        test_latency_effect_descriptor(),
        RuntimeFactory::Effect(create_test_latency_effect_runtime),
    )]);
}

fn test_instrument_descriptor() -> &'static ProcessorDescriptor {
    static DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
        name: "Test Instrument",
        params: &[],
        editor: None,
    };
    &DESCRIPTOR
}

fn test_latency_effect_descriptor() -> &'static ProcessorDescriptor {
    static DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
        name: "Latency",
        params: &[],
        editor: None,
    };
    &DESCRIPTOR
}

fn create_test_soundfont_runtime(
    slot: &SlotState,
    context: &InstrumentRuntimeContext<'_>,
) -> Result<Option<InstrumentRuntimeSpec>, RuntimeFactoryError> {
    if !matches!(
        slot.kind,
        ProcessorKind::BuiltIn { ref processor_id } if processor_id == BUILTIN_SOUNDFONT_ID
    ) || context.soundfonts.is_empty()
    {
        return Ok(None);
    }
    Ok(Some(InstrumentRuntimeSpec {
        processor: Box::<TestInstrumentProcessor>::default(),
        binding: Box::new(TestInstrumentBinding),
    }))
}

fn create_test_latency_effect_runtime(
    slot: &SlotState,
    _context: &crate::instrument::EffectRuntimeContext,
) -> Result<Option<EffectRuntimeSpec>, RuntimeFactoryError> {
    if !matches!(
        slot.kind,
        ProcessorKind::BuiltIn { ref processor_id } if processor_id == TEST_LATENCY_EFFECT_ID
    ) {
        return Ok(None);
    }
    let latency = Arc::new(AtomicU32::new(0));
    Ok(Some(EffectRuntimeSpec {
        processor: Box::new(TestLatencyEffect),
        binding: Some(Box::new(TestLatencyEffectBinding { latency })),
    }))
}

#[derive(Default)]
struct TestInstrumentProcessor {
    active: bool,
}

impl Processor for TestInstrumentProcessor {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        test_instrument_descriptor()
    }

    fn set_param(&mut self, id: &str, normalized: f32) -> bool {
        id.is_empty() && normalized == 0.0
    }

    fn get_param(&self, id: &str) -> Option<f32> {
        (id.is_empty()).then_some(0.0)
    }

    fn save_state(&self) -> ProcessorState {
        ProcessorState::default()
    }

    fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError> {
        if state.0.is_empty() {
            Ok(())
        } else {
            Err(ProcessorStateError::Decode(
                "test instrument state must be empty".to_string(),
            ))
        }
    }

    fn reset(&mut self) {
        self.active = false;
    }
}

impl InstrumentProcessor for TestInstrumentProcessor {
    fn handle_midi(&mut self, event: MidiEvent) {
        match event {
            MidiEvent::NoteOn { velocity, .. } if velocity > 0 => self.active = true,
            MidiEvent::NoteOff { .. }
            | MidiEvent::AllNotesOff { .. }
            | MidiEvent::AllSoundOff { .. } => self.active = false,
            _ => {}
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let value = if self.active { 0.25 } else { 0.0 };
        left.fill(value);
        right.fill(value);
    }

    fn is_sleeping(&self) -> bool {
        !self.active
    }
}

struct TestLatencyEffect;

impl Processor for TestLatencyEffect {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        test_latency_effect_descriptor()
    }

    fn set_param(&mut self, id: &str, normalized: f32) -> bool {
        id.is_empty() && normalized == 0.0
    }

    fn get_param(&self, id: &str) -> Option<f32> {
        (id.is_empty()).then_some(0.0)
    }

    fn save_state(&self) -> ProcessorState {
        ProcessorState::default()
    }

    fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError> {
        if state.0.is_empty() {
            Ok(())
        } else {
            Err(ProcessorStateError::Decode(
                "test latency effect state must be empty".to_string(),
            ))
        }
    }

    fn reset(&mut self) {}
}

impl EffectProcessor for TestLatencyEffect {
    fn process(
        &mut self,
        left: &[f32],
        right: &[f32],
        left_out: &mut [f32],
        right_out: &mut [f32],
    ) {
        left_out.copy_from_slice(left);
        right_out.copy_from_slice(right);
    }
}

struct TestLatencyEffectBinding {
    latency: Arc<AtomicU32>,
}

impl RuntimeBinding for TestLatencyEffectBinding {
    fn controller(&self) -> Box<dyn Controller> {
        Box::new(TestLatencyEffectController {
            latency: Arc::clone(&self.latency),
        })
    }

    fn latency_samples(&self) -> u32 {
        self.latency.load(Ordering::Relaxed)
    }

    fn prepare_destroy(&self) {
        TEST_EFFECT_PREPARE_DESTROY_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

struct TestLatencyEffectController {
    latency: Arc<AtomicU32>,
}

impl Controller for TestLatencyEffectController {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        test_latency_effect_descriptor()
    }

    fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
        if id == "latency_samples" {
            Ok(self.latency.load(Ordering::Relaxed) as f32 / 128.0)
        } else {
            Err(ControllerError::UnknownParameter(id.to_string()))
        }
    }

    fn set_param(&self, id: &str, normalized: f32) -> Result<(), ControllerError> {
        if id == "latency_samples" {
            let latency = (normalized.clamp(0.0, 1.0) * 128.0)
                .round()
                .to_u32()
                .unwrap_or(0);
            self.latency.store(latency, Ordering::Relaxed);
            Ok(())
        } else {
            Err(ControllerError::UnknownParameter(id.to_string()))
        }
    }

    fn save_state(&self) -> Result<ProcessorState, ControllerError> {
        Ok(ProcessorState::default())
    }

    fn load_state(&self, state: &ProcessorState) -> Result<(), ControllerError> {
        if state.0.is_empty() {
            Ok(())
        } else {
            Err(ControllerError::Backend(
                "test latency effect state must be empty".to_string(),
            ))
        }
    }
}

struct TestInstrumentBinding;

impl RuntimeBinding for TestInstrumentBinding {
    fn controller(&self) -> Box<dyn Controller> {
        Box::new(TestInstrumentController)
    }

    fn update_in_place(&self, slot: &SlotState) -> Result<bool, ProcessorStateError> {
        Ok(matches!(
            slot.kind,
            ProcessorKind::BuiltIn { ref processor_id } if processor_id == BUILTIN_SOUNDFONT_ID
        ))
    }

    fn prepare_destroy(&self) {
        TEST_INSTRUMENT_PREPARE_DESTROY_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

struct TestInstrumentController;

impl Controller for TestInstrumentController {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        test_instrument_descriptor()
    }

    fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
        Err(ControllerError::UnknownParameter(id.to_string()))
    }

    fn set_param(&self, id: &str, normalized: f32) -> Result<(), ControllerError> {
        if id.is_empty() && normalized == 0.0 {
            Ok(())
        } else {
            Err(ControllerError::UnknownParameter(id.to_string()))
        }
    }

    fn save_state(&self) -> Result<ProcessorState, ControllerError> {
        Ok(ProcessorState::default())
    }

    fn load_state(&self, state: &ProcessorState) -> Result<(), ControllerError> {
        if state.0.is_empty() {
            Ok(())
        } else {
            Err(ControllerError::Backend(
                "test instrument state must be empty".to_string(),
            ))
        }
    }
}

#[test]
fn strip_meter_captures_stereo_peak_and_hold() {
    let meter = SharedStripMeter::default();

    meter.observe_stereo(0.25, 0.5);
    let snapshot = meter.snapshot();

    assert!(snapshot.left.level > 0.0);
    assert!(snapshot.right.level > snapshot.left.level);
    assert_f32_close(snapshot.left.hold, snapshot.left.level);
    assert_f32_close(snapshot.right.hold, snapshot.right.level);
    assert!(!snapshot.clip_latched);
}

#[test]
fn strip_meter_hold_and_clip_stick_until_reset() {
    let meter = SharedStripMeter::default();

    meter.observe_stereo(1.1, 0.2);
    let hot = meter.snapshot();
    meter.observe_stereo(0.1, 0.05);
    let cooled = meter.snapshot();

    assert!(hot.clip_latched);
    assert!(cooled.clip_latched);
    assert_f32_close(cooled.left.hold, hot.left.hold);

    meter.reset();
    let reset = meter.snapshot();
    assert!(!reset.clip_latched);
    assert_f32_close(reset.left.hold, 0.0);
    assert_f32_close(reset.right.hold, 0.0);
}

#[test]
fn strip_meter_release_is_ballistic_not_instant() {
    let meter = SharedStripMeter::default();

    meter.observe_stereo(1.0, 0.5);
    let hot = meter.snapshot();
    meter.observe_stereo(0.05, 0.025);
    let falling = meter.snapshot();

    assert!(falling.left.level < hot.left.level);
    assert!(falling.left.level > normalize_meter_level(0.05));
    assert!(falling.right.level < hot.right.level);
    assert!(falling.right.level > normalize_meter_level(0.025));
}

#[test]
fn mixer_gain_floor_is_silence() {
    assert_f32_close(db_to_amplitude(-60.0), 0.0);
    assert!(db_to_amplitude(-59.5) > 0.0);
}

#[test]
fn pdc_plan_delays_faster_direct_master_paths() {
    let mixer = MixerState::new();
    let track_latencies = HashMap::from([
        (
            TrackId(0),
            StripLatency {
                pre_fader: 0,
                post_fader: 64,
                output: 64,
            },
        ),
        (TrackId(1), StripLatency::default()),
    ]);
    let plan = compute_pdc_plan_from_latencies(&mixer, &track_latencies, &HashMap::new());

    assert_eq!(plan.master_input_latency, 64);
    assert_eq!(plan.route_delay(TrackRoute::Master, 0), 64);
    assert_eq!(plan.route_delay(TrackRoute::Master, 64), 0);
}

#[test]
fn pdc_plan_uses_pre_or_post_send_source_latency() {
    let mut mixer = MixerState::new();
    let pre_bus = mixer.add_bus("Pre");
    let post_bus = mixer.add_bus("Post");
    mixer
        .add_track_bus_send(
            TrackId(0),
            BusSend {
                bus_id: pre_bus,
                gain_db: 0.0,
                enabled: true,
                pre_fader: true,
            },
        )
        .expect("pre send should be valid");
    mixer
        .add_track_bus_send(
            TrackId(0),
            BusSend {
                bus_id: post_bus,
                gain_db: 0.0,
                enabled: true,
                pre_fader: false,
            },
        )
        .expect("post send should be valid");
    let track_latencies = HashMap::from([(
        TrackId(0),
        StripLatency {
            pre_fader: 10,
            post_fader: 30,
            output: 30,
        },
    )]);
    let plan = compute_pdc_plan_from_latencies(&mixer, &track_latencies, &HashMap::new());

    assert_eq!(plan.bus_input_latency(pre_bus), 10);
    assert_eq!(plan.bus_input_latency(post_bus), 30);
    assert_eq!(plan.bus_send_delay(pre_bus, 10), 0);
    assert_eq!(plan.bus_send_delay(post_bus, 10), 20);
}

#[test]
fn pdc_plan_propagates_bus_effect_latency_to_master() {
    let mut mixer = MixerState::new();
    let bus = mixer.add_bus("Bus");
    mixer
        .set_track_route(TrackId(0), TrackRoute::Bus(bus))
        .expect("track route should be valid");
    let track_latencies = HashMap::from([(
        TrackId(0),
        StripLatency {
            pre_fader: 0,
            post_fader: 10,
            output: 10,
        },
    )]);
    let bus_effect_latencies = HashMap::from([(bus, 32)]);
    let plan = compute_pdc_plan_from_latencies(&mixer, &track_latencies, &bus_effect_latencies);

    assert_eq!(plan.bus_input_latency(bus), 10);
    assert_eq!(plan.master_input_latency, 42);
    assert_eq!(plan.route_delay(TrackRoute::Master, 0), 42);
}

#[test]
fn pdc_plan_uses_live_reported_latency_after_resync() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mut mixer = MixerState::new();
    mixer
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_effects(vec![latency_effect_slot()]);
    mixer
        .track_mut(TrackId(1))
        .expect("track 1 should exist")
        .set_effects(vec![latency_effect_slot()]);
    let context = harness.context().clone();
    let settings = harness.settings();
    let mut runtime = MixerRuntime::attach(&context, harness.commands(), &settings, &mixer)
        .expect("runtime should attach");

    assert_eq!(runtime.pdc_plan(&mixer).master_input_latency, 0);
    let controller = runtime
        .controller(
            &mixer,
            SlotAddress {
                strip_index: 1,
                slot_index: 1,
            },
        )
        .expect("controller lookup should succeed")
        .expect("latency effect should expose a controller");

    controller
        .set_param("latency_samples", 0.5)
        .expect("latency update should be accepted");
    assert_eq!(runtime.pdc_plan(&mixer).master_input_latency, 64);

    runtime
        .sync_all_routing(&context, harness.commands(), &mixer)
        .expect("routing should resync after latency change");
    assert!(
        runtime.tracks[1]
            .as_ref()
            .expect("track 1 runtime should exist")
            .route_delay_node
            .is_some(),
        "faster tracks need inserted compensation delay after latency change"
    );
}

#[test]
fn strip_meter_release_eventually_reaches_floor() {
    let meter = SharedStripMeter::default();

    meter.observe_stereo(1.0, 1.0);
    for _ in 0..4_000 {
        meter.observe_stereo(0.0, 0.0);
    }
    let cooled = meter.snapshot();

    assert!(cooled.left.level <= 0.001);
    assert!(cooled.right.level <= 0.001);
}

#[test]
fn meter_snapshot_normalizes_db_monotonically() {
    assert!(normalize_meter_level(0.05) < normalize_meter_level(0.5));
    assert!(normalize_meter_level(0.5) < normalize_meter_level(1.0));
}

#[test]
fn strip_level_updates_apply_without_scheduled_parameter_changes() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let level = super::SharedStripLevel::new(1.0, 0.0);
    let strip = super::handle_with_inputs(
        harness.commands(),
        super::StereoBalanceGain::new(level.clone()),
        inputs!(),
    );
    harness.context().with_activation(|| {
        let signal = handle(TestSineGen::new(44_100.0, 440.0));
        graph_output(0, strip.channels(2));
        connect_stereo(node_id_of(signal), node_id_of(strip));
    });

    harness.process_blocks(8);
    assert!(harness.output_has_signal());

    level.set(0.0, 0.0);
    harness.process_blocks(8);

    assert!(
        !harness.output_has_signal(),
        "strip level updates should affect audio without going through scheduled parameter changes"
    );
}

#[test]
fn simd_strip_pass_matches_scalar_path() {
    let frames = 11;
    let left_in = vec![-0.9, -0.6, -0.3, 0.0, 0.2, 0.4, 0.6, 0.8, -0.7, 0.5, -0.1];
    let right_in = vec![0.7, -0.5, 0.3, -0.1, 0.0, 0.2, -0.4, 0.6, -0.8, 0.9, -0.2];
    let mut scalar_left = vec![0.0; frames];
    let mut scalar_right = vec![0.0; frames];
    let mut simd_left = vec![0.0; frames];
    let mut simd_right = vec![0.0; frames];

    let scalar = super::process_stereo_balance_meter_scalar(
        &left_in,
        &right_in,
        &mut scalar_left,
        &mut scalar_right,
        0.75,
        0.25,
        frames,
    );
    let simd = super::process_stereo_balance_meter_simd(
        &left_in,
        &right_in,
        &mut simd_left,
        &mut simd_right,
        0.75,
        0.25,
        frames,
    );

    for (a, b) in scalar_left.iter().zip(simd_left.iter()) {
        assert!((a - b).abs() < 1.0e-6);
    }
    for (a, b) in scalar_right.iter().zip(simd_right.iter()) {
        assert!((a - b).abs() < 1.0e-6);
    }
    assert!((scalar.0 - simd.0).abs() < 1.0e-6);
    assert!((scalar.1 - simd.1).abs() < 1.0e-6);
}

#[test]
fn reset_one_strip_meter_does_not_touch_another() {
    let left = SharedStripMeter::default();
    let right = SharedStripMeter::default();

    left.observe_stereo(1.1, 0.7);
    right.observe_stereo(0.9, 0.8);

    left.reset();

    let left_snapshot = left.snapshot();
    let right_snapshot = right.snapshot();

    assert!(!left_snapshot.clip_latched);
    assert_f32_close(left_snapshot.left.hold, 0.0);
    assert_f32_close(left_snapshot.right.hold, 0.0);

    assert!(!right_snapshot.clip_latched);
    assert!(right_snapshot.left.hold > 0.0);
    assert!(right_snapshot.right.hold > 0.0);
}

struct TestSineGen {
    phase: f32,
    phase_increment: f32,
}

#[impl_gen]
impl TestSineGen {
    #[new]
    fn new(sample_rate: f32, frequency: f32) -> Self {
        Self {
            phase: 0.0,
            phase_increment: std::f32::consts::TAU * frequency / sample_rate,
        }
    }

    #[process]
    fn process(
        &mut self,
        left_out: &mut [Sample],
        right_out: &mut [Sample],
        block_size: BlockSize,
    ) -> GenState {
        for frame in 0..block_size.0 {
            let sample = self.phase.sin();
            left_out[frame] = sample;
            right_out[frame] = sample * 0.5;
            self.phase += self.phase_increment;
        }
        GenState::Continue
    }
}

fn build_soundfont_mixer(harness: &mut OfflineHarness) -> Result<Mixer, MixerRuntimeError> {
    let mut state = MixerState::new();
    state.set_soundfont(test_soundfont_resource());
    for track_id in 1..crate::mixer::INSTRUMENT_TRACK_COUNT {
        let track_id = TrackId(u16::try_from(track_id).unwrap_or(u16::MAX));
        state
            .track_mut(track_id)?
            .set_instrument_slot(SlotState::new(
                ProcessorKind::Plugin {
                    plugin_id: "none".to_string(),
                },
                ProcessorState::default(),
            ));
    }
    state
        .track_mut(TrackId(0))?
        .set_instrument_slot(soundfont_slot(0));
    let context = harness.context().clone();
    let settings = harness.settings();
    let mixer = Mixer::new(&context, harness.commands(), &settings, state)?;
    harness.wait_for_graph_settled();
    Ok(mixer)
}

#[test]
fn raw_commands_and_active_context_use_same_graph() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let raw_graph = harness.commands().current_graph();
    let active_graph = harness
        .context()
        .with_activation(|| knyst_commands().current_graph());
    assert_eq!(raw_graph, active_graph);

    let _mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
    let raw_graph = harness.commands().current_graph();
    let active_graph = harness
        .context()
        .with_activation(|| knyst_commands().current_graph());
    assert_eq!(raw_graph, active_graph);
}

#[test]
fn inspect_mixer_graph() {
    let mut harness = OfflineHarness::new_with_outputs(44_100, 64, 4);
    let mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");
    harness.context().with_activation(|| {
        graph_output(2, handle.raw_handle().channels(2));
    });
    let inspection = harness.inspection();
    eprintln!("outputs: {}", inspection.num_outputs);
    eprintln!(
        "graph output edges: {:?}",
        inspection.graph_output_input_edges
    );
    for (index, node) in inspection.nodes.iter().enumerate() {
        eprintln!(
            "{index}: {} {:?} inputs={:?} outputs={:?}",
            node.name, node.address, node.input_edges, node.output_channels
        );
    }
}

#[test]
fn track_soundfont_reaches_master_output_with_thread_local_note_on() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");
    harness.process_blocks(50);
    harness.commands().transport_play();
    harness.wait_for_transport_settled();

    schedule_test_note(&mut harness, handle);

    harness.process_blocks(50);

    assert!(harness.errors().is_empty(), "{:?}", harness.errors());
    assert!(harness.output_has_signal());
}

#[test]
fn combined_track_soundfont_preserves_stereo_output_at_center_pan() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");
    harness.process_blocks(50);
    harness.commands().transport_play();
    harness.wait_for_transport_settled();

    schedule_test_note(&mut harness, handle);
    harness.process_blocks(50);

    let left_peak = harness
        .output_channel(0)
        .iter()
        .copied()
        .map(f32::abs)
        .fold(0.0, f32::max);
    let right_peak = harness
        .output_channel(1)
        .iter()
        .copied()
        .map(f32::abs)
        .fold(0.0, f32::max);

    assert!(left_peak > 0.001, "left channel stayed silent");
    assert!(right_peak > 0.001, "right channel stayed silent");
}

#[test]
fn direct_sine_node_to_bus_preserves_expected_samples() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let _bus_handle = harness.context().with_activation(|| {
        let signal = handle(TestSineGen::new(44_100.0, 440.0));
        let bus_handle = bus(2);
        graph_output(0, bus_handle.channels(2));
        connect_stereo(node_id_of(signal), node_id_of(bus_handle));
        bus_handle
    });

    harness.process_block();

    let phase_increment = std::f32::consts::TAU * 440.0 / 44_100.0;
    for frame in 0..8 {
        let expected_left = (phase_increment * frame as f32).sin();
        let expected_right = expected_left * 0.5;
        assert!((harness.output_channel(0)[frame] - expected_left).abs() < 1.0e-5);
        assert!((harness.output_channel(1)[frame] - expected_right).abs() < 1.0e-5);
    }
}

#[test]
fn disabled_track_send_creates_silent_runtime_send_node() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mut state = MixerState::new();
    state.set_soundfont(test_soundfont_resource());
    let bus_id = state.add_bus("Verb");
    let mut send = BusSend::new(bus_id, -6.0, false);
    send.enabled = false;
    state
        .add_track_bus_send(TrackId(0), send)
        .expect("send should be accepted");
    state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(soundfont_slot(0));
    let context = harness.context().clone();
    let settings = harness.settings();

    let mixer = Mixer::new(&context, harness.commands(), &settings, state)
        .expect("mixer should initialize");

    let track_runtime = mixer.runtime.tracks[0]
        .as_ref()
        .expect("track runtime should exist");
    assert_eq!(track_runtime.sends.len(), 1);
    assert_f32_close(track_runtime.sends[0].level.get(), 0.0);
}

#[test]
fn effect_bypass_updates_wet_target_without_rebuilding_effect_node() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mut state = MixerState::new();
    state.set_soundfont(test_soundfont_resource());
    let track = state.track_mut(TrackId(0)).expect("track 0 should exist");
    track.set_instrument_slot(soundfont_slot(0));
    track.set_effects(vec![latency_effect_slot()]);
    let context = harness.context().clone();
    let settings = harness.settings();

    let mut mixer = Mixer::new(&context, harness.commands(), &settings, state)
        .expect("mixer should initialize");
    let address = SlotAddress {
        strip_index: 1,
        slot_index: 1,
    };
    let effect = mixer.runtime.tracks[0]
        .as_ref()
        .and_then(|track| track.effects[0].as_ref())
        .expect("effect runtime should exist");
    let node = effect.node_id();
    assert_f32_close(effect.wet.get(), 1.0);

    mixer
        .state
        .slot_mut(address)
        .expect("slot should exist")
        .bypassed = true;
    mixer
        .runtime
        .sync_slot_bypass(&mixer.state, address)
        .expect("bypass sync should succeed");

    let effect = mixer.runtime.tracks[0]
        .as_ref()
        .and_then(|track| track.effects[0].as_ref())
        .expect("effect runtime should still exist");
    assert_eq!(effect.node_id(), node);
    assert_f32_close(effect.wet.get(), 0.0);
}

#[test]
fn pre_fader_track_send_ignores_track_gain() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mut state = MixerState::new();
    state.set_soundfont(test_soundfont_resource());
    let bus_id = state.add_bus("Cue");
    state
        .add_track_bus_send(TrackId(0), BusSend::new(bus_id, 0.0, true))
        .expect("send should be accepted");
    let track = state.track_mut(TrackId(0)).expect("track 0 should exist");
    track.set_instrument_slot(soundfont_slot(0));
    track.state.gain_db = -60.0;
    let context = harness.context().clone();
    let settings = harness.settings();

    let mixer = Mixer::new(&context, harness.commands(), &settings, state)
        .expect("mixer should initialize");
    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");
    harness.commands().transport_play();
    harness.wait_for_transport_settled();

    schedule_test_note(&mut harness, handle);
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "pre-fader send should still feed its bus when the track fader is closed"
    );
}

#[test]
fn post_fader_track_send_follows_track_gain() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mut state = MixerState::new();
    state.set_soundfont(test_soundfont_resource());
    let bus_id = state.add_bus("Verb");
    state
        .add_track_bus_send(TrackId(0), BusSend::new(bus_id, 0.0, false))
        .expect("send should be accepted");
    let track = state.track_mut(TrackId(0)).expect("track 0 should exist");
    track.set_instrument_slot(soundfont_slot(0));
    track.state.gain_db = -60.0;
    let context = harness.context().clone();
    let settings = harness.settings();

    let mixer = Mixer::new(&context, harness.commands(), &settings, state)
        .expect("mixer should initialize");
    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");
    harness.commands().transport_play();
    harness.wait_for_transport_settled();

    schedule_test_note(&mut harness, handle);
    harness.process_blocks(16);

    assert!(
        !harness.output_has_signal(),
        "post-fader send should be silent when the track fader is closed"
    );
}

#[test]
fn muted_track_stays_silent() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mut state = MixerState::new();
    state.set_soundfont(test_soundfont_resource());
    let track = state.track_mut(TrackId(0)).expect("track 0 should exist");
    track.set_instrument_slot(soundfont_slot(0));
    track.state.muted = true;
    let context = harness.context().clone();
    let settings = harness.settings();
    let mixer = Mixer::new(&context, harness.commands(), &settings, state)
        .expect("mixer should initialize");
    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");
    harness.commands().transport_play();
    harness.wait_for_transport_settled();

    schedule_test_note(&mut harness, handle);

    harness.wait_for_transport_settled();

    assert!(!harness.output_has_signal());
}

#[test]
fn live_created_track_runtime_routes_to_master_output() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let context = harness.context().clone();
    let settings = harness.settings();
    let mut mixer = Mixer::new(&context, harness.commands(), &settings, MixerState::new())
        .expect("mixer should initialize");

    mixer.state.set_soundfont(test_soundfont_resource());
    mixer
        .runtime
        .sync_soundfonts(&mixer.state)
        .expect("soundfont should sync");
    mixer
        .state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(soundfont_slot(40));
    mixer
        .runtime
        .sync_track_instrument(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track instrument should sync");
    mixer
        .runtime
        .sync_track_routing(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track routing should sync");
    harness.wait_for_graph_settled();

    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats_f64(1.0));
    harness.wait_for_transport_settled();

    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");
    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    schedule_test_note(&mut harness, handle);
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "live-created track runtime stayed silent"
    );
}

#[test]
fn live_created_track_runtime_routes_to_master_output_without_extra_routing_pass() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let context = harness.context().clone();
    let settings = harness.settings();
    let mut mixer = Mixer::new(&context, harness.commands(), &settings, MixerState::new())
        .expect("mixer should initialize");

    mixer.state.set_soundfont(test_soundfont_resource());
    mixer
        .runtime
        .sync_soundfonts(&mixer.state)
        .expect("soundfont should sync");
    mixer
        .state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(soundfont_slot(40));
    mixer
        .runtime
        .sync_track_instrument(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track instrument should sync");
    harness.wait_for_graph_settled();

    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");
    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    schedule_test_note(&mut harness, handle);
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "live-created track runtime stayed silent without explicit routing sync"
    );
}

#[test]
fn inserting_first_effect_keeps_track_instrument_runtime() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mut state = MixerState::new();
    state.set_soundfont(test_soundfont_resource());
    state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(soundfont_slot(40));
    let context = harness.context().clone();
    let settings = harness.settings();
    let mut mixer = Mixer::new(&context, harness.commands(), &settings, state)
        .expect("mixer should initialize");
    let before = mixer.runtime.tracks[0]
        .as_ref()
        .and_then(|runtime| runtime.instrument.as_ref())
        .map(|instrument| instrument.handle.node_id())
        .expect("track instrument should exist");

    mixer
        .state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_effects(vec![latency_effect_slot()]);
    let graph_changed = mixer
        .runtime
        .sync_track_effects(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track effects should sync");
    let after = mixer.runtime.tracks[0]
        .as_ref()
        .and_then(|runtime| runtime.instrument.as_ref())
        .map(|instrument| instrument.handle.node_id())
        .expect("track instrument should still exist");

    assert!(!graph_changed);
    assert_eq!(after, before);
}

#[test]
fn replacing_track_instrument_prepares_binding_for_destroy_before_freeing_node() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mut state = MixerState::new();
    state.set_soundfont(test_soundfont_resource());
    state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(soundfont_slot(40));
    let context = harness.context().clone();
    let settings = harness.settings();
    let mut mixer = Mixer::new(&context, harness.commands(), &settings, state)
        .expect("mixer should initialize");

    TEST_INSTRUMENT_PREPARE_DESTROY_COUNT.store(0, Ordering::Relaxed);
    mixer
        .state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(SlotState::default());
    mixer
        .runtime
        .sync_track_instrument(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track instrument should sync");

    assert_eq!(
        TEST_INSTRUMENT_PREPARE_DESTROY_COUNT.load(Ordering::Relaxed),
        1
    );
}

#[test]
fn removing_track_effect_prepares_binding_for_destroy_before_freeing_node() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mut state = MixerState::new();
    state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_effects(vec![latency_effect_slot()]);
    let context = harness.context().clone();
    let settings = harness.settings();
    let mut mixer = Mixer::new(&context, harness.commands(), &settings, state)
        .expect("mixer should initialize");

    TEST_EFFECT_PREPARE_DESTROY_COUNT.store(0, Ordering::Relaxed);
    mixer
        .state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_effects(Vec::new());
    mixer
        .runtime
        .sync_track_effects(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track effects should sync");

    assert_eq!(TEST_EFFECT_PREPARE_DESTROY_COUNT.load(Ordering::Relaxed), 1);
}

#[test]
fn bus_chain_created_after_transport_reset_passes_signal() {
    assert_created_bus_chain_passes_signal(
        0.0,
        "bus chain created after transport reset stayed silent",
    );
}

#[test]
fn bus_chain_created_after_nonzero_transport_seek_passes_signal() {
    assert_created_bus_chain_passes_signal(
        1.0,
        "bus chain created after non-zero transport seek stayed silent",
    );
}

fn assert_created_bus_chain_passes_signal(seek_beats: f64, failure: &str) {
    let mut harness = OfflineHarness::new(44_100, 64);
    pause_seek_and_wait(&mut harness, seek_beats);
    harness.context().with_activation(|| {
        let signal = handle(TestSineGen::new(44_100.0, 440.0));
        let first_bus = bus(2);
        let second_bus = bus(2);
        graph_output(0, second_bus.channels(2));
        connect_stereo(node_id_of(signal), node_id_of(first_bus));
        connect_stereo(node_id_of(first_bus), node_id_of(second_bus));
    });

    harness.wait_for_transport_settled();
    assert!(harness.output_has_signal(), "{failure}");
}

#[test]
fn raw_soundfont_strip_chain_after_nonzero_transport_seek_produces_signal() {
    assert_raw_soundfont_chain_produces_signal(
        RawSoundfontChain::Strip,
        "raw soundfont strip chain after non-zero transport seek stayed silent",
    );
}

#[test]
fn raw_soundfont_source_bus_chain_after_nonzero_transport_seek_produces_signal() {
    assert_raw_soundfont_chain_produces_signal(
        RawSoundfontChain::SourceBus,
        "raw soundfont source-bus chain after non-zero transport seek stayed silent",
    );
}

#[derive(Clone, Copy)]
enum RawSoundfontChain {
    Strip,
    SourceBus,
}

fn assert_raw_soundfont_chain_produces_signal(topology: RawSoundfontChain, failure: &str) {
    let mut harness = OfflineHarness::new(44_100, 64);
    pause_seek_and_wait(&mut harness, 1.0);
    let handle = raw_soundfont_chain_handle(&mut harness, topology);
    harness.wait_for_graph_settled();
    play_note_and_assert_signal(&mut harness, handle, failure);
}

fn raw_soundfont_chain_handle(
    harness: &mut OfflineHarness,
    topology: RawSoundfontChain,
) -> InstrumentRuntimeHandle {
    let processor = TestInstrumentProcessor::default();
    let strip = super::handle_with_inputs(
        harness.commands(),
        super::StereoBalanceGain::new(unity_strip_gain()),
        inputs!(),
    );

    harness.context().with_activation(|| {
        let meter = SharedStripMeter::new(44_100, 64);
        let meter_node = handle(MeterTap::new(meter));
        let route_bus = bus(2);
        let reset_state = SharedInstrumentResetState::default();
        let instrument = handle(InstrumentProcessorNode::new(
            Box::new(processor),
            reset_state.clone(),
        ));
        graph_output(0, route_bus.channels(2));
        match topology {
            RawSoundfontChain::Strip => {
                connect_stereo(node_id_of(instrument), node_id_of(strip));
            }
            RawSoundfontChain::SourceBus => {
                let source_bus = bus(2);
                connect_stereo(node_id_of(instrument), node_id_of(source_bus));
                connect_stereo(node_id_of(source_bus), node_id_of(strip));
            }
        }
        connect_stereo(node_id_of(strip), node_id_of(meter_node));
        connect_stereo(node_id_of(meter_node), node_id_of(route_bus));
        InstrumentRuntimeHandle::new(instrument, reset_state)
    })
}

#[test]
fn raw_soundfont_chain_routed_into_master_after_nonzero_transport_seek_produces_signal() {
    let mut harness = OfflineHarness::new(44_100, 64);
    harness.commands().transport_pause();
    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats_f64(1.0));
    harness.wait_for_transport_settled();

    let mixer_state = MixerState::new();
    let context = harness.context().clone();
    let settings = harness.settings();
    let master = MasterRuntime::new(&context, harness.commands(), &settings, &mixer_state);

    let processor = TestInstrumentProcessor::default();

    let strip = super::handle_with_inputs(
        harness.commands(),
        super::StereoBalanceGain::new(unity_strip_gain()),
        inputs!(),
    );
    let handle = harness.context().with_activation(|| {
        let source_bus = bus(2);
        let meter = SharedStripMeter::new(44_100, 64);
        let meter_node = handle(MeterTap::new(meter));
        let route_bus = bus(2);
        let reset_state = SharedInstrumentResetState::default();
        let instrument = handle(InstrumentProcessorNode::new(
            Box::new(processor),
            reset_state.clone(),
        ));
        connect_stereo(node_id_of(instrument), node_id_of(source_bus));
        connect_stereo(node_id_of(source_bus), node_id_of(strip));
        connect_stereo(node_id_of(strip), node_id_of(meter_node));
        connect_stereo(node_id_of(meter_node), node_id_of(route_bus));
        connect_stereo(node_id_of(route_bus), master.input_node());
        InstrumentRuntimeHandle::new(instrument, reset_state)
    });
    harness.wait_for_graph_settled();

    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    schedule_test_note(&mut harness, handle);
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "raw soundfont chain routed into master after non-zero transport seek stayed silent"
    );
}

#[test]
fn preexisting_raw_soundfont_chain_survives_nonzero_transport_seek() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mixer_state = MixerState::new();
    let context = harness.context().clone();
    let settings = harness.settings();
    let master = MasterRuntime::new(&context, harness.commands(), &settings, &mixer_state);

    let processor = TestInstrumentProcessor::default();

    let strip = super::handle_with_inputs(
        harness.commands(),
        super::StereoBalanceGain::new(unity_strip_gain()),
        inputs!(),
    );
    let handle = harness.context().with_activation(|| {
        let source_bus = bus(2);
        let meter = SharedStripMeter::new(44_100, 64);
        let meter_node = handle(MeterTap::new(meter));
        let route_bus = bus(2);
        let reset_state = SharedInstrumentResetState::default();
        let instrument = handle(InstrumentProcessorNode::new(
            Box::new(processor),
            reset_state.clone(),
        ));
        connect_stereo(node_id_of(instrument), node_id_of(source_bus));
        connect_stereo(node_id_of(source_bus), node_id_of(strip));
        connect_stereo(node_id_of(strip), node_id_of(meter_node));
        connect_stereo(node_id_of(meter_node), node_id_of(route_bus));
        connect_stereo(node_id_of(route_bus), master.input_node());
        InstrumentRuntimeHandle::new(instrument, reset_state)
    });
    harness.wait_for_graph_settled();

    harness.commands().transport_pause();
    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats_f64(1.0));
    harness.wait_for_transport_settled();
    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    schedule_test_note(&mut harness, handle);
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "preexisting raw soundfont chain went silent after non-zero transport seek"
    );
}

#[test]
fn preexisting_sine_chain_survives_nonzero_transport_seek() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mixer_state = MixerState::new();
    let context = harness.context().clone();
    let settings = harness.settings();
    let master = MasterRuntime::new(&context, harness.commands(), &settings, &mixer_state);

    let strip = super::handle_with_inputs(
        harness.commands(),
        super::StereoBalanceGain::new(unity_strip_gain()),
        inputs!(),
    );
    harness.context().with_activation(|| {
        let source_bus = bus(2);
        let signal = handle(TestSineGen::new(44_100.0, 440.0));
        let meter = SharedStripMeter::new(44_100, 64);
        let meter_node = handle(MeterTap::new(meter));
        let route_bus = bus(2);
        connect_stereo(node_id_of(signal), node_id_of(source_bus));
        connect_stereo(node_id_of(source_bus), node_id_of(strip));
        connect_stereo(node_id_of(strip), node_id_of(meter_node));
        connect_stereo(node_id_of(meter_node), node_id_of(route_bus));
        connect_stereo(node_id_of(route_bus), master.input_node());
    });

    harness.commands().transport_pause();
    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats_f64(1.0));
    harness.wait_for_transport_settled();
    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "preexisting sine chain went silent after non-zero transport seek"
    );
}

#[test]
fn preexisting_bus_chain_survives_nonzero_transport_seek() {
    let mut harness = OfflineHarness::new(44_100, 64);
    harness.context().with_activation(|| {
        let signal = handle(TestSineGen::new(44_100.0, 440.0));
        let first_bus = bus(2);
        let second_bus = bus(2);
        graph_output(0, second_bus.channels(2));
        connect_stereo(node_id_of(signal), node_id_of(first_bus));
        connect_stereo(node_id_of(first_bus), node_id_of(second_bus));
    });

    harness.commands().transport_pause();
    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats_f64(1.0));
    harness.wait_for_transport_settled();
    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "preexisting bus chain went silent after non-zero transport seek"
    );
}

#[test]
fn preexisting_sine_strip_survives_nonzero_transport_seek() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let strip = super::handle_with_inputs(
        harness.commands(),
        super::StereoBalanceGain::new(unity_strip_gain()),
        inputs!(),
    );
    harness.context().with_activation(|| {
        let signal = handle(TestSineGen::new(44_100.0, 440.0));
        graph_output(0, strip.channels(2));
        connect_stereo(node_id_of(signal), node_id_of(strip));
    });

    harness.commands().transport_pause();
    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats_f64(1.0));
    harness.wait_for_transport_settled();
    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "preexisting sine strip went silent after non-zero transport seek"
    );
}

#[test]
fn settled_preexisting_sine_strip_survives_nonzero_transport_seek() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let strip = super::handle_with_inputs(
        harness.commands(),
        super::StereoBalanceGain::new(unity_strip_gain()),
        inputs!(),
    );
    harness.context().with_activation(|| {
        let signal = handle(TestSineGen::new(44_100.0, 440.0));
        graph_output(0, strip.channels(2));
        connect_stereo(node_id_of(signal), node_id_of(strip));
    });

    harness.process_blocks(16);
    assert!(
        harness.output_has_signal(),
        "preexisting sine strip should be audible before seek"
    );

    harness.commands().transport_pause();
    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats_f64(1.0));
    harness.wait_for_transport_settled();
    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "settled preexisting sine strip went silent after non-zero transport seek"
    );
}

#[test]
fn live_created_track_runtime_after_transport_reset_routes_to_master_output() {
    assert_live_created_track_routes_after_seek(
        0.0,
        "live-created track runtime after transport reset stayed silent",
    );
}

#[test]
fn live_created_track_runtime_after_transport_reset_routes_without_extra_routing_pass() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let context = harness.context().clone();
    let settings = harness.settings();
    let mut mixer = Mixer::new(&context, harness.commands(), &settings, MixerState::new())
        .expect("mixer should initialize");

    harness.commands().transport_pause();
    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats_f64(0.0));
    harness.wait_for_transport_settled();

    mixer.state.set_soundfont(test_soundfont_resource());
    mixer
        .runtime
        .sync_soundfonts(&mixer.state)
        .expect("soundfont should sync");
    mixer
        .state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(soundfont_slot(40));
    mixer
        .runtime
        .sync_track_instrument(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track instrument should sync");
    harness.wait_for_graph_settled();

    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");

    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    schedule_test_note(&mut harness, handle);
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "live-created track runtime after transport reset stayed silent without explicit routing \
         sync"
    );
}

#[test]
fn live_created_track_runtime_after_nonzero_transport_seek_routes_to_master_output() {
    assert_live_created_track_routes_after_seek(
        1.0,
        "live-created track runtime after non-zero transport seek stayed silent",
    );
}

fn assert_live_created_track_routes_after_seek(seek_beats: f64, failure: &str) {
    let mut harness = OfflineHarness::new(44_100, 64);
    let context = harness.context().clone();
    let settings = harness.settings();
    let mut mixer = Mixer::new(&context, harness.commands(), &settings, MixerState::new())
        .expect("mixer should initialize");
    pause_seek_and_wait(&mut harness, seek_beats);

    mixer.state.set_soundfont(test_soundfont_resource());
    mixer
        .runtime
        .sync_soundfonts(&mixer.state)
        .expect("soundfont should sync");
    mixer
        .state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(soundfont_slot(40));
    mixer
        .runtime
        .sync_track_instrument(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track instrument should sync");
    mixer
        .runtime
        .sync_track_routing(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track routing should sync");
    harness.wait_for_graph_settled();

    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");
    play_note_and_assert_signal(&mut harness, handle, failure);
}

#[test]
fn same_soundfont_program_change_keeps_existing_instrument_node() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let context = harness.context().clone();
    let mut mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");

    let original_node = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist")
        .node_id();

    mixer
        .state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(soundfont_slot(40));
    mixer
        .runtime
        .sync_track_instrument(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track instrument should sync");

    let updated_node = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should still exist")
        .node_id();

    assert_eq!(
        updated_node, original_node,
        "same-soundfont program changes should keep the existing instrument node alive"
    );
}

#[test]
fn same_soundfont_program_change_stays_audible_without_routing_resync() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let context = harness.context().clone();
    let mut mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");

    mixer
        .state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(soundfont_slot(40));
    let sync = mixer
        .runtime
        .sync_track_instrument(&context, harness.commands(), &mixer.state, TrackId(0))
        .expect("track instrument should sync");

    assert!(
        matches!(sync, super::TrackInstrumentSync::UpdatedInPlace),
        "same-soundfont program changes should not require graph rebuild"
    );

    let handle = mixer
        .instrument_handle(TrackId(0))
        .expect("track instrument should exist");
    harness.commands().transport_play();
    harness.wait_for_transport_settled();
    schedule_test_note(&mut harness, handle);
    harness.process_blocks(16);

    assert!(
        harness.output_has_signal(),
        "same-soundfont program change should stay audible without routing resync"
    );
}
