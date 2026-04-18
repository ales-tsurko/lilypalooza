use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use knyst::KnystError;
use knyst::audio_backend::{AudioBackend, AudioBackendError};
use knyst::controller::Controller;
use knyst::controller::KnystCommands;
use knyst::graph::{Graph, RunGraph, RunGraphSettings};
use knyst::inspection::GraphInspection;
use knyst::modal_interface::KnystContext;
use knyst::prelude::{Beats, KnystSphere, MultiThreadedKnystCommands, Sample, SphereSettings};
use knyst::resources::{Resources, ResourcesSettings};
use midly::num::{u4, u7, u15, u24, u28};
use midly::{
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, Track, TrackEvent, TrackEventKind,
};

use crate::engine::AudioEngineSettings;
use crate::instrument::soundfont_synth::SoundfontResource;

pub(crate) struct OfflineHarness {
    backend: TestBackend,
    controller: Controller,
    sphere: KnystSphere,
    commands: MultiThreadedKnystCommands,
    context: KnystContext,
    errors: Arc<Mutex<Vec<String>>>,
    settings: AudioEngineSettings,
}

impl OfflineHarness {
    pub(crate) fn new(sample_rate: usize, block_size: usize) -> Self {
        Self::new_with_outputs(sample_rate, block_size, 2)
    }

    pub(crate) fn new_with_outputs(
        sample_rate: usize,
        block_size: usize,
        num_outputs: usize,
    ) -> Self {
        let mut backend = TestBackend::new(sample_rate, block_size, num_outputs);
        let errors = Arc::new(Mutex::new(Vec::new()));
        let errors_for_handler = Arc::clone(&errors);
        let (sphere, controller) = KnystSphere::start_return_controller(
            &mut backend,
            SphereSettings {
                num_inputs: 0,
                num_outputs,
                ..Default::default()
            },
            Box::new(move |error: KnystError| {
                errors_for_handler
                    .lock()
                    .expect("error store should not be poisoned")
                    .push(error.to_string());
            }),
        )
        .expect("offline sphere should start");
        let context = sphere.context();
        let commands = sphere.commands();
        let mut harness = Self {
            backend,
            controller,
            sphere,
            commands,
            context,
            errors,
            settings: AudioEngineSettings {
                sample_rate,
                block_size,
            },
        };
        harness.commands.transport_pause();
        harness.commands.transport_seek_to_beats(Beats::ZERO);
        for _ in 0..64 {
            harness.process_block();
            if let Some(snapshot) = harness.commands.current_transport_snapshot()
                && snapshot.state == knyst::prelude::TransportState::Paused
                && snapshot.beats.unwrap_or(Beats::ZERO) == Beats::ZERO
            {
                break;
            }
        }
        harness
    }

    pub(crate) fn commands(&mut self) -> &mut MultiThreadedKnystCommands {
        &mut self.commands
    }

    pub(crate) fn context(&self) -> &KnystContext {
        &self.context
    }

    pub(crate) fn settings(&self) -> AudioEngineSettings {
        self.settings
    }

    pub(crate) fn process_block(&mut self) {
        self.controller.run(10_000);
        self.backend.process_block();
    }

    pub(crate) fn process_blocks(&mut self, count: usize) {
        for _ in 0..count {
            self.process_block();
        }
    }

    pub(crate) fn wait_for_graph_settled(&mut self) {
        let receiver = self.commands.request_graph_settled();
        for _ in 0..256 {
            self.process_block();
            if receiver.try_recv().is_ok() {
                return;
            }
        }
        panic!("graph should settle in offline harness");
    }

    pub(crate) fn wait_for_transport_settled(&mut self) {
        let receiver = self.commands.request_transport_settled();
        for _ in 0..256 {
            self.process_block();
            if receiver.try_recv().is_ok() {
                return;
            }
        }
        panic!("transport should settle in offline harness");
    }

    pub(crate) fn output_channel(&self, channel: usize) -> &[Sample] {
        self.backend.output_channel(channel)
    }

    pub(crate) fn output_has_signal(&self) -> bool {
        self.output_channel(0)
            .iter()
            .chain(self.output_channel(1).iter())
            .any(|sample| sample.abs() > 1.0e-6)
    }

    pub(crate) fn errors(&self) -> Vec<String> {
        self.errors
            .lock()
            .expect("error store should not be poisoned")
            .clone()
    }

    pub(crate) fn inspection(&mut self) -> GraphInspection {
        let receiver = self.commands.request_inspection();
        for _ in 0..100 {
            self.controller.run(10_000);
            if let Ok(inspection) = receiver.try_recv() {
                return inspection;
            }
        }
        panic!("inspection should be returned");
    }

    #[allow(dead_code)]
    pub(crate) fn sphere(&self) -> &KnystSphere {
        &self.sphere
    }
}

pub(crate) fn test_soundfont_resource() -> SoundfontResource {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/soundfonts/FluidR3_GM.sf2")
        .canonicalize()
        .expect("test SoundFont should exist");
    SoundfontResource {
        id: "default".to_string(),
        name: "FluidR3".to_string(),
        path,
    }
}

pub(crate) fn simple_midi_bytes(ppq: u16) -> Vec<u8> {
    midi_bytes_with_note(ppq, 0)
}

pub(crate) fn delayed_note_midi_bytes(ppq: u16, start_tick: u32) -> Vec<u8> {
    midi_bytes_with_note(ppq, start_tick)
}

pub(crate) fn sustained_note_midi_bytes(ppq: u16, duration_ticks: u32) -> Vec<u8> {
    let header = Header::new(Format::Parallel, Timing::Metrical(u15::from(ppq)));
    let tempo_track: Track<'static> = vec![
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(500_000))),
        },
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];
    let note_track: Track<'static> = vec![
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOn {
                    key: u7::from(60),
                    vel: u7::from(100),
                },
            },
        },
        TrackEvent {
            delta: u28::from(duration_ticks),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOff {
                    key: u7::from(60),
                    vel: u7::from(0),
                },
            },
        },
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];
    let smf = Smf {
        header,
        tracks: vec![tempo_track, note_track],
    };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes)
        .expect("test MIDI should serialize");
    bytes
}

pub(crate) fn four_track_midi_bytes(ppq: u16) -> Vec<u8> {
    let header = Header::new(Format::Parallel, Timing::Metrical(u15::from(ppq)));
    let tempo_track: Track<'static> = vec![
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(500_000))),
        },
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];

    let note_tracks: Vec<Track<'static>> = (0_u8..4)
        .map(|channel| {
            vec![
                TrackEvent {
                    delta: u28::from(0),
                    kind: TrackEventKind::Midi {
                        channel: u4::from(channel),
                        message: MidiMessage::NoteOn {
                            key: u7::from(60 + channel * 2),
                            vel: u7::from(100),
                        },
                    },
                },
                TrackEvent {
                    delta: u28::from(u32::from(ppq)),
                    kind: TrackEventKind::Midi {
                        channel: u4::from(channel),
                        message: MidiMessage::NoteOff {
                            key: u7::from(60 + channel * 2),
                            vel: u7::from(0),
                        },
                    },
                },
                TrackEvent {
                    delta: u28::from(0),
                    kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
                },
            ]
        })
        .collect();

    let mut tracks = vec![tempo_track];
    tracks.extend(note_tracks);
    let smf = Smf { header, tracks };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes)
        .expect("test MIDI should serialize");
    bytes
}

fn midi_bytes_with_note(ppq: u16, start_tick: u32) -> Vec<u8> {
    let header = Header::new(Format::Parallel, Timing::Metrical(u15::from(ppq)));
    let tempo_track: Track<'static> = vec![
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(500_000))),
        },
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];
    let note_track: Track<'static> = vec![
        TrackEvent {
            delta: u28::from(start_tick),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOn {
                    key: u7::from(60),
                    vel: u7::from(100),
                },
            },
        },
        TrackEvent {
            delta: u28::from(u32::from(ppq)),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOff {
                    key: u7::from(60),
                    vel: u7::from(0),
                },
            },
        },
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];
    let smf = Smf {
        header,
        tracks: vec![tempo_track, note_track],
    };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes)
        .expect("test MIDI should serialize");
    bytes
}

pub(crate) struct TestBackend {
    sample_rate: usize,
    block_size: usize,
    num_outputs: usize,
    run_graph: Option<RunGraph>,
}

pub(crate) struct SharedTestBackend {
    sample_rate: usize,
    block_size: usize,
    num_outputs: usize,
    shared: Arc<Mutex<SharedTestBackendState>>,
}

pub(crate) struct SharedTestBackendHandle {
    block_size: usize,
    sample_rate: usize,
    shared: Arc<Mutex<SharedTestBackendState>>,
}

struct SharedTestBackendState {
    run_graph: Option<RunGraph>,
}

pub(crate) struct SharedTestBackendDriver {
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SharedTestBackend {
    pub(crate) fn new(
        sample_rate: usize,
        block_size: usize,
        num_outputs: usize,
    ) -> (Self, SharedTestBackendHandle) {
        let shared = Arc::new(Mutex::new(SharedTestBackendState { run_graph: None }));
        (
            Self {
                sample_rate,
                block_size,
                num_outputs,
                shared: Arc::clone(&shared),
            },
            SharedTestBackendHandle {
                block_size,
                sample_rate,
                shared,
            },
        )
    }
}

impl SharedTestBackendHandle {
    pub(crate) fn process_block(&self) {
        let mut shared = self
            .shared
            .lock()
            .expect("shared test backend state should not be poisoned");
        if let Some(run_graph) = &mut shared.run_graph {
            run_graph.run_resources_communication(10_000);
            run_graph.process_block();
        }
    }

    pub(crate) fn output_channel(&self, channel: usize) -> Vec<Sample> {
        let shared = self
            .shared
            .lock()
            .expect("shared test backend state should not be poisoned");
        let run_graph = shared.run_graph.as_ref().expect("run graph should exist");
        run_graph
            .graph_output_buffers()
            .get_channel(channel)
            .to_vec()
    }

    pub(crate) fn output_has_signal(&self) -> bool {
        self.output_channel(0)
            .into_iter()
            .chain(self.output_channel(1))
            .any(|sample| sample.abs() > 1.0e-6)
    }

    pub(crate) fn start_realtime(&self) -> SharedTestBackendDriver {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shared = Arc::clone(&self.shared);
        let shutdown_flag = Arc::clone(&shutdown);
        let block_duration =
            Duration::from_secs_f64(self.block_size as f64 / self.sample_rate as f64);
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::Acquire) {
                let mut state = shared
                    .lock()
                    .expect("shared test backend state should not be poisoned");
                if let Some(run_graph) = &mut state.run_graph {
                    run_graph.run_resources_communication(10_000);
                    run_graph.process_block();
                }
                drop(state);
                thread::sleep(block_duration);
            }
        });
        SharedTestBackendDriver {
            shutdown,
            thread: Some(thread),
        }
    }
}

impl Drop for SharedTestBackendDriver {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl TestBackend {
    pub(crate) fn new(sample_rate: usize, block_size: usize, num_outputs: usize) -> Self {
        Self {
            sample_rate,
            block_size,
            num_outputs,
            run_graph: None,
        }
    }

    fn process_block(&mut self) {
        if let Some(run_graph) = &mut self.run_graph {
            run_graph.run_resources_communication(10_000);
            run_graph.process_block();
        }
    }

    fn output_channel(&self, channel: usize) -> &[Sample] {
        let run_graph = self.run_graph.as_ref().expect("run graph should exist");
        run_graph.graph_output_buffers().get_channel(channel)
    }
}

impl AudioBackend for TestBackend {
    fn start_processing_return_controller(
        &mut self,
        mut graph: Graph,
        resources: Resources,
        run_graph_settings: RunGraphSettings,
        error_handler: Box<dyn FnMut(knyst::KnystError) + Send + 'static>,
    ) -> Result<Controller, AudioBackendError> {
        if self.run_graph.is_some() {
            return Err(AudioBackendError::BackendAlreadyRunning);
        }

        let (run_graph, resources_command_sender, resources_command_receiver) =
            RunGraph::new(&mut graph, resources, run_graph_settings)?;
        let controller = Controller::new(
            graph,
            error_handler,
            resources_command_sender,
            resources_command_receiver,
        );
        self.run_graph = Some(run_graph);
        Ok(controller)
    }

    fn stop(&mut self) -> Result<(), AudioBackendError> {
        if self.run_graph.take().is_some() {
            Ok(())
        } else {
            Err(AudioBackendError::BackendNotRunning)
        }
    }

    fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    fn block_size(&self) -> Option<usize> {
        Some(self.block_size)
    }

    fn native_output_channels(&self) -> Option<usize> {
        Some(self.num_outputs)
    }

    fn native_input_channels(&self) -> Option<usize> {
        Some(0)
    }
}

impl AudioBackend for SharedTestBackend {
    fn start_processing_return_controller(
        &mut self,
        mut graph: Graph,
        resources: Resources,
        run_graph_settings: RunGraphSettings,
        error_handler: Box<dyn FnMut(knyst::KnystError) + Send + 'static>,
    ) -> Result<Controller, AudioBackendError> {
        let mut shared = self
            .shared
            .lock()
            .expect("shared test backend state should not be poisoned");
        if shared.run_graph.is_some() {
            return Err(AudioBackendError::BackendAlreadyRunning);
        }

        let (run_graph, resources_command_sender, resources_command_receiver) =
            RunGraph::new(&mut graph, resources, run_graph_settings)?;
        let controller = Controller::new(
            graph,
            error_handler,
            resources_command_sender,
            resources_command_receiver,
        );
        shared.run_graph = Some(run_graph);
        Ok(controller)
    }

    fn stop(&mut self) -> Result<(), AudioBackendError> {
        let mut shared = self
            .shared
            .lock()
            .expect("shared test backend state should not be poisoned");
        if shared.run_graph.take().is_some() {
            Ok(())
        } else {
            Err(AudioBackendError::BackendNotRunning)
        }
    }

    fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    fn block_size(&self) -> Option<usize> {
        Some(self.block_size)
    }

    fn native_output_channels(&self) -> Option<usize> {
        Some(self.num_outputs)
    }

    fn native_input_channels(&self) -> Option<usize> {
        Some(0)
    }
}

fn _test_resources() -> Resources {
    Resources::new(ResourcesSettings::default()).expect("test resources should initialize")
}
