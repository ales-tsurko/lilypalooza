//! MIDI-track sequencer and score scheduler.

use std::{
    collections::{BTreeSet, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    },
};

use arc_swap::ArcSwap;
use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};
use knyst::{
    prelude::{
        Beats, KnystCommands, MultiThreadedKnystCommands, SchedulerChange, SchedulerExtension,
        SchedulerExtensionContext, TransportState,
    },
    scheduling::{MusicalTimeMap, TempoChange},
    time::{SUBBEAT_TESIMALS_PER_BEAT, Seconds},
};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use num_traits::ToPrimitive;

use crate::{
    instrument::{InstrumentRuntimeHandle, MidiEvent as EngineMidiEvent},
    mixer::{INSTRUMENT_TRACK_COUNT, TrackId},
    transport::TransportError,
};

mod midi_events;
mod scheduler;

pub use scheduler::{Sequencer, SequencerError, SequencerHandle};

#[cfg(test)]
mod sequencer_tests;
