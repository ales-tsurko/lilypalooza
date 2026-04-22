//! Processor registry.

use crate::instrument::gain_effect;
use crate::instrument::metronome_synth;
use crate::instrument::soundfont_synth;
use crate::instrument::{
    BUILTIN_GAIN_ID, BUILTIN_METRONOME_ID, BUILTIN_NONE_ID, BUILTIN_SOUNDFONT_ID,
    EffectRuntimeSpec, InstrumentRuntimeContext, InstrumentRuntimeSpec, ProcessorDescriptor,
    ProcessorKind, RuntimeFactoryError, SlotState,
};

/// Stable processor identifier type used by the registry catalog.
pub type Id = &'static str;

/// Processor role in the mixer graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Instrument slot processor.
    Instrument,
    /// Effect slot processor.
    Effect,
}

/// Backend family that provides the processor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Built into the application.
    BuiltIn,
    /// CLAP plugin backend.
    Clap,
    /// VST3 plugin backend.
    Vst3,
}

/// One discoverable processor entry.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// Stable processor id.
    pub id: Id,
    /// User-visible processor name.
    pub name: &'static str,
    /// Processor role.
    pub role: Role,
    /// Backend family.
    pub backend: Backend,
    /// Static processor descriptor.
    pub descriptor: &'static ProcessorDescriptor,
    factory: Factory,
}

#[derive(Debug, Clone, Copy)]
struct Factory {
    is_empty: bool,
    create_instrument: Option<CreateInstrument>,
    create_effect: Option<CreateEffect>,
}

type CreateInstrument = fn(
    &SlotState,
    &InstrumentRuntimeContext<'_>,
) -> Result<Option<InstrumentRuntimeSpec>, RuntimeFactoryError>;
type CreateEffect = fn(&SlotState) -> Result<Option<EffectRuntimeSpec>, RuntimeFactoryError>;

const NONE_DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
    name: "None",
    params: &[],
    editor: None,
};

const NONE: Entry = Entry {
    id: BUILTIN_NONE_ID,
    name: "None",
    role: Role::Instrument,
    backend: Backend::BuiltIn,
    descriptor: &NONE_DESCRIPTOR,
    factory: Factory {
        is_empty: true,
        create_instrument: None,
        create_effect: None,
    },
};

const SOUNDFONT: Entry = Entry {
    id: BUILTIN_SOUNDFONT_ID,
    name: "SoundFont",
    role: Role::Instrument,
    backend: Backend::BuiltIn,
    descriptor: soundfont_synth::DESCRIPTOR,
    factory: Factory {
        is_empty: false,
        create_instrument: Some(soundfont_synth::create_runtime),
        create_effect: None,
    },
};

const GAIN: Entry = Entry {
    id: BUILTIN_GAIN_ID,
    name: "Gain",
    role: Role::Effect,
    backend: Backend::BuiltIn,
    descriptor: gain_effect::DESCRIPTOR,
    factory: Factory {
        is_empty: false,
        create_instrument: None,
        create_effect: Some(gain_effect::create_runtime),
    },
};

const METRONOME: Entry = Entry {
    id: BUILTIN_METRONOME_ID,
    name: "Metronome",
    role: Role::Instrument,
    backend: Backend::BuiltIn,
    descriptor: metronome_synth::DESCRIPTOR,
    factory: Factory {
        is_empty: false,
        create_instrument: None,
        create_effect: None,
    },
};

const ENTRIES: &[Entry] = &[NONE, SOUNDFONT, GAIN, METRONOME];

/// Returns the full processor catalog.
#[must_use]
pub fn all() -> &'static [Entry] {
    ENTRIES
}

/// Returns one catalog entry by stable id.
#[must_use]
pub fn entry(id: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|entry| entry.id == id)
}

/// Resolves one persisted processor kind into a catalog entry.
#[must_use]
pub fn resolve(kind: &ProcessorKind) -> Option<&'static Entry> {
    match kind {
        ProcessorKind::BuiltIn { processor_id } => entry(processor_id),
        ProcessorKind::Plugin { .. } => None,
    }
}

#[must_use]
pub(crate) fn is_empty(kind: &ProcessorKind) -> bool {
    resolve(kind).is_some_and(|entry| entry.factory.is_empty)
}

pub(crate) fn create_instrument_runtime(
    slot: &SlotState,
    context: &InstrumentRuntimeContext<'_>,
) -> Result<Option<InstrumentRuntimeSpec>, RuntimeFactoryError> {
    let Some(factory) = resolve(&slot.kind).and_then(|entry| entry.factory.create_instrument)
    else {
        return Ok(None);
    };
    factory(slot, context)
}

pub(crate) fn create_effect_runtime(
    slot: &SlotState,
) -> Result<Option<EffectRuntimeSpec>, RuntimeFactoryError> {
    let Some(factory) = resolve(&slot.kind).and_then(|entry| entry.factory.create_effect) else {
        return Ok(None);
    };
    factory(slot)
}

#[cfg(test)]
mod tests {
    use super::{Backend, Role, all, entry, is_empty};
    use crate::instrument::{BUILTIN_GAIN_ID, BUILTIN_SOUNDFONT_ID, ProcessorKind};

    #[test]
    fn builtins_are_registered_in_one_catalog() {
        let entries = all();

        assert!(entries.iter().any(|entry| entry.id == BUILTIN_SOUNDFONT_ID
            && entry.role == Role::Instrument
            && entry.backend == Backend::BuiltIn));
        assert!(entries.iter().any(|entry| entry.id == BUILTIN_GAIN_ID
            && entry.role == Role::Effect
            && entry.backend == Backend::BuiltIn));
    }

    #[test]
    fn built_in_lookup_resolves_from_kind() {
        let kind = ProcessorKind::BuiltIn {
            processor_id: BUILTIN_GAIN_ID.to_string(),
        };

        assert!(!is_empty(&kind));
        assert_eq!(entry(BUILTIN_GAIN_ID).map(|entry| entry.name), Some("Gain"));
    }
}
