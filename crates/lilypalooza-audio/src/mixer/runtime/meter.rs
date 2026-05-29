use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

use num_traits::ToPrimitive;

use super::db_to_amplitude;
use crate::mixer::{
    ChannelMeterSnapshot, STRIP_METER_MAX_DB, STRIP_METER_MIN_DB, StripMeterSnapshot,
};

const METER_FLOOR: f32 = 0.00003162278;

#[derive(Debug, Clone)]
pub(super) struct SharedStripMeter {
    inner: Arc<SharedStripMeterInner>,
    sample_rate: usize,
    sample_rate_hz: f32,
    block_size: usize,
}

#[derive(Debug, Default)]
pub(super) struct SharedStripMeterInner {
    peak_l: AtomicU32,
    peak_r: AtomicU32,
    hold_l: AtomicU32,
    hold_r: AtomicU32,
    clip_latched: AtomicBool,
}

impl SharedStripMeter {
    pub(super) fn new(sample_rate: usize, block_size: usize) -> Self {
        let sample_rate = sample_rate.max(1);
        Self {
            inner: Arc::new(SharedStripMeterInner::default()),
            sample_rate,
            sample_rate_hz: sample_rate.to_f32().unwrap_or(f32::MAX).max(1.0),
            block_size: block_size.max(1),
        }
    }

    pub(super) fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    pub(super) fn observe_stereo(&self, left: f32, right: f32) {
        let left = left.abs();
        let right = right.abs();

        let peak_l = f32::from_bits(self.inner.peak_l.load(Ordering::Relaxed));
        let peak_r = f32::from_bits(self.inner.peak_r.load(Ordering::Relaxed));
        let displayed_l = apply_meter_release(peak_l, left, self.sample_rate_hz, self.block_size);
        let displayed_r = apply_meter_release(peak_r, right, self.sample_rate_hz, self.block_size);

        self.inner
            .peak_l
            .store(displayed_l.to_bits(), Ordering::Relaxed);
        self.inner
            .peak_r
            .store(displayed_r.to_bits(), Ordering::Relaxed);

        let hold_l = f32::from_bits(self.inner.hold_l.load(Ordering::Relaxed));
        if left > hold_l {
            self.inner.hold_l.store(left.to_bits(), Ordering::Relaxed);
        }

        let hold_r = f32::from_bits(self.inner.hold_r.load(Ordering::Relaxed));
        if right > hold_r {
            self.inner.hold_r.store(right.to_bits(), Ordering::Relaxed);
        }

        if left >= 1.0 || right >= 1.0 {
            self.inner.clip_latched.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn reset(&self) {
        self.inner.peak_l.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.inner.peak_r.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.inner.hold_l.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.inner.hold_r.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.inner.clip_latched.store(false, Ordering::Relaxed);
    }

    pub(super) fn snapshot(&self) -> StripMeterSnapshot {
        let peak_l = f32::from_bits(self.inner.peak_l.load(Ordering::Relaxed));
        let peak_r = f32::from_bits(self.inner.peak_r.load(Ordering::Relaxed));
        let hold_l = f32::from_bits(self.inner.hold_l.load(Ordering::Relaxed));
        let hold_r = f32::from_bits(self.inner.hold_r.load(Ordering::Relaxed));

        StripMeterSnapshot {
            left: ChannelMeterSnapshot {
                level: normalize_meter_level(peak_l),
                hold: normalize_meter_level(hold_l),
                hold_db: amplitude_to_db(hold_l).max(STRIP_METER_MIN_DB),
            },
            right: ChannelMeterSnapshot {
                level: normalize_meter_level(peak_r),
                hold: normalize_meter_level(hold_r),
                hold_db: amplitude_to_db(hold_r).max(STRIP_METER_MIN_DB),
            },
            clip_latched: self.inner.clip_latched.load(Ordering::Relaxed),
        }
    }
}

impl Default for SharedStripMeter {
    fn default() -> Self {
        Self::new(44_100, 64)
    }
}

#[derive(Debug, Clone)]
pub(super) struct SharedStripLevel {
    inner: Arc<SharedStripLevelInner>,
}

#[derive(Debug)]
pub(super) struct SharedStripLevelInner {
    gain: AtomicU32,
    pan: AtomicU32,
}

impl SharedStripLevel {
    pub(super) fn new(gain: f32, pan: f32) -> Self {
        Self {
            inner: Arc::new(SharedStripLevelInner {
                gain: AtomicU32::new(gain.to_bits()),
                pan: AtomicU32::new(pan.to_bits()),
            }),
        }
    }

    pub(super) fn set(&self, gain: f32, pan: f32) {
        self.inner.gain.store(gain.to_bits(), Ordering::Relaxed);
        self.inner.pan.store(pan.to_bits(), Ordering::Relaxed);
    }

    pub(super) fn gain(&self) -> f32 {
        f32::from_bits(self.inner.gain.load(Ordering::Relaxed))
    }

    pub(super) fn pan(&self) -> f32 {
        f32::from_bits(self.inner.pan.load(Ordering::Relaxed))
    }
}

pub(super) fn normalize_meter_level(amplitude: f32) -> f32 {
    let db = 20.0 * amplitude.abs().max(METER_FLOOR).log10();
    ((db - STRIP_METER_MIN_DB) / (STRIP_METER_MAX_DB - STRIP_METER_MIN_DB)).clamp(0.0, 1.0)
}

const METER_RELEASE_DB_PER_SECOND: f32 = 18.0;

fn amplitude_to_db(amplitude: f32) -> f32 {
    20.0 * amplitude.abs().max(METER_FLOOR).log10()
}

fn apply_meter_release(current: f32, observed: f32, sample_rate: f32, block_size: usize) -> f32 {
    if observed >= current {
        return observed;
    }

    let current_db = amplitude_to_db(current);
    let observed_db = amplitude_to_db(observed);
    let block_seconds = block_size as f32 / sample_rate.max(1.0);
    let released_db = current_db - METER_RELEASE_DB_PER_SECOND * block_seconds;
    db_to_amplitude(released_db.max(observed_db).max(STRIP_METER_MIN_DB))
}
