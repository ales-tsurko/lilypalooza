use knyst::prelude::{BlockSize, GenState, Sample, impl_gen};
use wide::f32x4;

use super::{SharedStripLevel, SharedStripMeter};
use crate::instrument::{
    InstrumentProcessor, ScheduledInstrumentEvent, SharedAudioValue, SharedInstrumentResetState,
    SmoothedAudioValue, decode_instrument_event, generation_is_current_or_newer,
};

pub(crate) fn process_stereo_balance_meter_scalar(
    left_in: &[Sample],
    right_in: &[Sample],
    left_out: &mut [Sample],
    right_out: &mut [Sample],
    left_mul: f32,
    right_mul: f32,
    frames: usize,
) -> (f32, f32) {
    let mut peak_left = 0.0_f32;
    let mut peak_right = 0.0_f32;
    for ((left_in, right_in), (left_out, right_out)) in left_in
        .iter()
        .copied()
        .zip(right_in.iter().copied())
        .zip(left_out.iter_mut().zip(right_out.iter_mut()))
        .take(frames)
    {
        let left = left_in * left_mul;
        let right = right_in * right_mul;
        *left_out = left;
        *right_out = right;
        peak_left = peak_left.max(left.abs());
        peak_right = peak_right.max(right.abs());
    }
    (peak_left, peak_right)
}

pub(crate) fn process_stereo_balance_meter_simd(
    left_in: &[Sample],
    right_in: &[Sample],
    left_out: &mut [Sample],
    right_out: &mut [Sample],
    left_mul: f32,
    right_mul: f32,
    frames: usize,
) -> (f32, f32) {
    let simd_width = 4;
    let left_mul4 = f32x4::splat(left_mul);
    let right_mul4 = f32x4::splat(right_mul);
    let mut peak_left4 = f32x4::splat(0.0);
    let mut peak_right4 = f32x4::splat(0.0);

    let simd_frames = frames / simd_width * simd_width;
    for frame in (0..simd_frames).step_by(simd_width) {
        let Some(left_chunk) = left_in.get(frame..frame + simd_width) else {
            break;
        };
        let Some(right_chunk) = right_in.get(frame..frame + simd_width) else {
            break;
        };
        let Some(left_out_chunk) = left_out.get_mut(frame..frame + simd_width) else {
            break;
        };
        let Some(right_out_chunk) = right_out.get_mut(frame..frame + simd_width) else {
            break;
        };
        let Ok(left_chunk) = <&[Sample; 4]>::try_from(left_chunk) else {
            break;
        };
        let Ok(right_chunk) = <&[Sample; 4]>::try_from(right_chunk) else {
            break;
        };
        let left = f32x4::from(*left_chunk) * left_mul4;
        let right = f32x4::from(*right_chunk) * right_mul4;

        let left_arr = left.to_array();
        let right_arr = right.to_array();
        left_out_chunk.copy_from_slice(&left_arr);
        right_out_chunk.copy_from_slice(&right_arr);

        peak_left4 = peak_left4.max(left.abs());
        peak_right4 = peak_right4.max(right.abs());
    }

    let left_peak_arr = peak_left4.to_array();
    let right_peak_arr = peak_right4.to_array();
    let mut peak_left = left_peak_arr.into_iter().fold(0.0_f32, f32::max);
    let mut peak_right = right_peak_arr.into_iter().fold(0.0_f32, f32::max);

    if simd_frames < frames {
        let Some(left_tail) = left_in.get(simd_frames..frames) else {
            return (peak_left, peak_right);
        };
        let Some(right_tail) = right_in.get(simd_frames..frames) else {
            return (peak_left, peak_right);
        };
        let Some(left_out_tail) = left_out.get_mut(simd_frames..frames) else {
            return (peak_left, peak_right);
        };
        let Some(right_out_tail) = right_out.get_mut(simd_frames..frames) else {
            return (peak_left, peak_right);
        };
        let (tail_left, tail_right) = process_stereo_balance_meter_scalar(
            left_tail,
            right_tail,
            left_out_tail,
            right_out_tail,
            left_mul,
            right_mul,
            frames - simd_frames,
        );
        peak_left = peak_left.max(tail_left);
        peak_right = peak_right.max(tail_right);
    }

    (peak_left, peak_right)
}

pub(super) struct StereoGain {
    level: SharedAudioValue,
    gain: SmoothedAudioValue,
}

struct StereoProcessBlock<'a> {
    left_in: &'a [Sample],
    right_in: &'a [Sample],
    left_out: &'a mut [Sample],
    right_out: &'a mut [Sample],
    block_size: BlockSize,
}

impl<'a> StereoProcessBlock<'a> {
    fn new(
        left_in: &'a [Sample],
        right_in: &'a [Sample],
        left_out: &'a mut [Sample],
        right_out: &'a mut [Sample],
        block_size: BlockSize,
    ) -> Self {
        Self {
            left_in,
            right_in,
            left_out,
            right_out,
            block_size,
        }
    }

    fn process(
        self,
        mut process: impl FnMut(Sample, Sample) -> (Sample, Sample),
    ) -> (Sample, Sample) {
        let mut peak_left = 0.0_f32;
        let mut peak_right = 0.0_f32;
        for ((left_in, right_in), (left_out, right_out)) in self
            .left_in
            .iter()
            .copied()
            .zip(self.right_in.iter().copied())
            .zip(self.left_out.iter_mut().zip(self.right_out.iter_mut()))
            .take(self.block_size.0)
        {
            let (left, right) = process(left_in, right_in);
            *left_out = left;
            *right_out = right;
            peak_left = peak_left.max(left.abs());
            peak_right = peak_right.max(right.abs());
        }
        (peak_left, peak_right)
    }
}

fn stereo_block<'a>(
    inputs: (&'a [Sample], &'a [Sample]),
    outputs: (&'a mut [Sample], &'a mut [Sample]),
    block_size: BlockSize,
) -> StereoProcessBlock<'a> {
    let (left_in, right_in) = inputs;
    let (left_out, right_out) = outputs;
    StereoProcessBlock::new(left_in, right_in, left_out, right_out, block_size)
}

#[impl_gen]
impl StereoGain {
    #[new]
    pub(super) fn new(level: SharedAudioValue, sample_rate: usize) -> Self {
        Self {
            gain: SmoothedAudioValue::new(level.get(), sample_rate),
            level,
        }
    }

    #[rustfmt::skip]
    #[process]
    fn process(&mut self, left_in: &[Sample], right_in: &[Sample], left_out: &mut [Sample], right_out: &mut [Sample], block_size: BlockSize) -> GenState {
        self.process_gain_block(stereo_block((left_in, right_in), (left_out, right_out), block_size));
        GenState::Continue
    }

    fn process_gain_block(&mut self, block: StereoProcessBlock<'_>) {
        self.gain.set_target(self.level.get());
        block.process(|left_in, right_in| {
            let gain = self.gain.next_sample();
            (left_in * gain, right_in * gain)
        });
    }
}

pub(super) struct StereoDelay {
    delay_samples: usize,
    left: Vec<Sample>,
    right: Vec<Sample>,
    cursor: usize,
}

#[impl_gen]
impl StereoDelay {
    #[rustfmt::skip]
    #[process]
    fn process(&mut self, left_in: &[Sample], right_in: &[Sample], left_out: &mut [Sample], right_out: &mut [Sample], block_size: BlockSize) -> GenState {
        self.process_delay_line(stereo_block((left_in, right_in), (left_out, right_out), block_size));
        GenState::Continue
    }

    #[new]
    pub(super) fn new(delay_samples: usize) -> Self {
        let delay_samples = delay_samples.max(1);
        Self {
            delay_samples,
            left: vec![0.0; delay_samples],
            right: vec![0.0; delay_samples],
            cursor: 0,
        }
    }

    fn process_delay_line(&mut self, block: StereoProcessBlock<'_>) {
        block.process(|left_in, right_in| {
            let cursor = self.cursor;
            let (Some(left_delay), Some(right_delay)) =
                (self.left.get_mut(cursor), self.right.get_mut(cursor))
            else {
                self.cursor = 0;
                return (left_in, right_in);
            };
            let delayed = (*left_delay, *right_delay);
            *left_delay = left_in;
            *right_delay = right_in;
            self.cursor = (self.cursor + 1) % self.delay_samples;
            delayed
        });
    }
}

#[cfg(test)]
pub(super) struct StereoBalanceGain {
    level: SharedStripLevel,
}

#[cfg(test)]
#[impl_gen]
impl StereoBalanceGain {
    #[rustfmt::skip]
    #[process]
    fn process(&mut self, left_in: &[Sample], right_in: &[Sample], left_out: &mut [Sample], right_out: &mut [Sample], block_size: BlockSize) -> GenState {
        self.process_balance_gain(stereo_block((left_in, right_in), (left_out, right_out), block_size));
        GenState::Continue
    }

    #[new]
    pub(super) fn new(level: SharedStripLevel) -> Self {
        Self { level }
    }

    fn process_balance_gain(&mut self, block: StereoProcessBlock<'_>) {
        let (gain, left_gain, right_gain) = self.balance_coefficients();
        apply_balance_gain(block, gain, left_gain, right_gain);
    }

    fn balance_coefficients(&self) -> (Sample, Sample, Sample) {
        let gain = self.level.gain();
        let pan = self.level.pan().clamp(-1.0, 1.0);
        (
            gain,
            if pan > 0.0 { 1.0 - pan } else { 1.0 },
            if pan < 0.0 { 1.0 + pan } else { 1.0 },
        )
    }
}

#[cfg(test)]
fn apply_balance_gain(
    block: StereoProcessBlock<'_>,
    gain: Sample,
    left_gain: Sample,
    right_gain: Sample,
) {
    block.process(|left_in, right_in| (left_in * gain * left_gain, right_in * gain * right_gain));
}

#[derive(Debug, Clone)]
pub(super) struct StereoBalanceMeter {
    level: SharedStripLevel,
    meter: SharedStripMeter,
    gain: SmoothedAudioValue,
    pan: SmoothedAudioValue,
}

#[impl_gen]
impl StereoBalanceMeter {
    #[rustfmt::skip]
    #[process]
    fn process(&mut self, left_in: &[Sample], right_in: &[Sample], left_out: &mut [Sample], right_out: &mut [Sample], block_size: BlockSize) -> GenState {
        self.process_metered_balance(stereo_block((left_in, right_in), (left_out, right_out), block_size));
        GenState::Continue
    }

    #[new]
    pub(super) fn new(
        level: SharedStripLevel,
        meter: SharedStripMeter,
        sample_rate: usize,
    ) -> Self {
        Self {
            gain: SmoothedAudioValue::new(level.gain(), sample_rate),
            pan: SmoothedAudioValue::new(level.pan(), sample_rate),
            level,
            meter,
        }
    }

    fn process_metered_balance(&mut self, block: StereoProcessBlock<'_>) {
        self.gain.set_target(self.level.gain());
        self.pan.set_target(self.level.pan().clamp(-1.0, 1.0));
        let (peak_left, peak_right) = block.process(|left_in, right_in| {
            let gain = self.gain.next_sample();
            let pan = self.pan.next_sample().clamp(-1.0, 1.0);
            let left_gain = if pan > 0.0 { 1.0 - pan } else { 1.0 };
            let right_gain = if pan < 0.0 { 1.0 + pan } else { 1.0 };
            (left_in * gain * left_gain, right_in * gain * right_gain)
        });
        self.meter.observe_stereo(peak_left, peak_right);
    }
}

pub(super) struct TrackInstrumentStripNode {
    active_generation: u32,
    reset_state: SharedInstrumentResetState,
    processor: Box<dyn InstrumentProcessor>,
    scratch_left: Vec<Sample>,
    scratch_right: Vec<Sample>,
    level: SharedStripLevel,
    meter: SharedStripMeter,
    gain: SmoothedAudioValue,
    pan: SmoothedAudioValue,
}

#[derive(Debug, Clone, Copy, Default)]
struct StereoSample {
    left: Sample,
    right: Sample,
}

#[derive(Debug, Clone, Copy, Default)]
struct StereoPeak {
    left: f32,
    right: f32,
}

impl StereoPeak {
    fn observe(&mut self, sample: StereoSample) {
        self.left = self.left.max(sample.left.abs());
        self.right = self.right.max(sample.right.abs());
    }
}

fn pan_gains(pan: f32) -> StereoSample {
    StereoSample {
        left: if pan > 0.0 { 1.0 - pan } else { 1.0 },
        right: if pan < 0.0 { 1.0 + pan } else { 1.0 },
    }
}

fn gained_sample_pair(
    gain: &mut SmoothedAudioValue,
    pan: &mut SmoothedAudioValue,
    left: Sample,
    right: Sample,
) -> StereoSample {
    let gain = gain.next_sample();
    let pan = pan.next_sample().clamp(-1.0, 1.0);
    let pan_gain = pan_gains(pan);
    StereoSample {
        left: left * gain * pan_gain.left,
        right: right * gain * pan_gain.right,
    }
}

impl TrackInstrumentStripNode {
    pub(super) fn new(
        processor: Box<dyn InstrumentProcessor>,
        reset_state: SharedInstrumentResetState,
        level: SharedStripLevel,
        meter: SharedStripMeter,
        sample_rate: usize,
    ) -> Self {
        Self {
            active_generation: 0,
            reset_state,
            processor,
            scratch_left: Vec::new(),
            scratch_right: Vec::new(),
            gain: SmoothedAudioValue::new(level.gain(), sample_rate),
            pan: SmoothedAudioValue::new(level.pan(), sample_rate),
            level,
            meter,
        }
    }

    fn resize_scratch(&mut self, frames: usize) {
        self.scratch_left.resize(frames, 0.0);
        self.scratch_right.resize(frames, 0.0);
    }

    fn apply_requested_reset(&mut self) {
        let requested_reset = self.reset_state.load();
        if generation_is_current_or_newer(requested_reset, self.active_generation)
            && requested_reset != self.active_generation
        {
            self.reset_to_generation(requested_reset);
        }
    }

    fn reset_to_generation(&mut self, generation: u32) {
        self.active_generation = generation;
        self.processor.reset();
    }

    fn apply_scheduled_instrument_event(&mut self, event: ScheduledInstrumentEvent) {
        match event {
            ScheduledInstrumentEvent::Reset { generation } => {
                if generation_is_current_or_newer(generation, self.active_generation) {
                    self.reset_to_generation(generation);
                }
            }
            ScheduledInstrumentEvent::Midi { generation, event } => {
                if generation == self.active_generation {
                    self.processor.handle_midi(event);
                }
            }
        }
    }

    fn render_processor_to_scratch(&mut self, frames: usize) -> bool {
        let Some(scratch_left) = self.scratch_left.get_mut(..frames) else {
            return false;
        };
        let Some(scratch_right) = self.scratch_right.get_mut(..frames) else {
            return false;
        };
        self.processor.render(scratch_left, scratch_right);
        true
    }

    fn write_scratch_to_outputs(
        &mut self,
        frames: usize,
        left_out: &mut [Sample],
        right_out: &mut [Sample],
    ) {
        self.gain.set_target(self.level.gain());
        self.pan.set_target(self.level.pan().clamp(-1.0, 1.0));

        let mut peak = StereoPeak::default();
        let gain = &mut self.gain;
        let pan = &mut self.pan;
        for ((scratch_left, scratch_right), (left_out, right_out)) in self
            .scratch_left
            .iter()
            .take(frames)
            .copied()
            .zip(self.scratch_right.iter().take(frames).copied())
            .zip(left_out.iter_mut().zip(right_out.iter_mut()))
        {
            let sample = gained_sample_pair(gain, pan, scratch_left, scratch_right);
            *left_out = sample.left;
            *right_out = sample.right;
            peak.observe(sample);
        }
        self.meter.observe_stereo(peak.left, peak.right);
    }

    fn gen_state(&self) -> GenState {
        if self.processor.is_sleeping() {
            GenState::Sleep
        } else {
            GenState::Continue
        }
    }
}

impl knyst::r#gen::Gen for TrackInstrumentStripNode {
    fn process(
        &mut self,
        ctx: knyst::r#gen::GenContext<'_, '_, '_>,
        _resources: &mut knyst::Resources,
    ) -> GenState {
        let frames = ctx.outputs.block_size();
        self.resize_scratch(frames);
        self.apply_requested_reset();
        for event in ctx.events {
            if event.input != 0 {
                continue;
            }
            let knyst::graph::EventPayload::Bytes(bytes) = &event.payload else {
                continue;
            };
            let Some(event) = decode_instrument_event(bytes.as_ref()) else {
                continue;
            };
            self.apply_scheduled_instrument_event(event);
        }
        if !self.render_processor_to_scratch(frames) {
            return GenState::Continue;
        }

        let mut outputs = ctx.outputs.iter_mut();
        let Some(left_out) = outputs.next() else {
            return GenState::Continue;
        };
        let Some(right_out) = outputs.next() else {
            return GenState::Continue;
        };

        let Some(left_out) = left_out.get_mut(..frames) else {
            return GenState::Continue;
        };
        let Some(right_out) = right_out.get_mut(..frames) else {
            return GenState::Continue;
        };
        self.write_scratch_to_outputs(frames, left_out, right_out);
        self.gen_state()
    }

    fn num_inputs(&self) -> usize {
        0
    }

    fn num_outputs(&self) -> usize {
        2
    }

    fn num_event_inputs(&self) -> usize {
        1
    }

    fn event_input_desc(&self, input: usize) -> &'static str {
        match input {
            0 => "event",
            _ => "",
        }
    }

    fn name(&self) -> &'static str {
        "TrackInstrumentStripNode"
    }
}
