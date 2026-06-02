//! Audio Unit v2 adapter.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, RwLock},
};

use lilypalooza_audio::{
    ParameterInfo, ProcessorDescriptor, SlotState,
    instrument::{
        Controller, ControllerError, EditorError, EffectProcessor, EffectRuntimeContext,
        EffectRuntimeSpec, InstrumentProcessor, InstrumentRuntimeContext, InstrumentRuntimeSpec,
        MidiEvent, Processor, ProcessorState, ProcessorStateError, RuntimeBinding,
        RuntimeFactoryError, registry,
    },
};
use serde::{Deserialize, Serialize};

mod editor;
mod probe;
mod runtime;

pub use probe::{
    AuComponentId, AuPluginMetadata, AuProbeError, FORMAT, ValidationReport, candidate_paths,
    is_au_candidate, probe, stable_processor_id,
};

#[cfg(test)]
mod au_tests;

static METADATA: OnceLock<RwLock<HashMap<String, AuPluginMetadata>>> = OnceLock::new();

fn metadata_store() -> &'static RwLock<HashMap<String, AuPluginMetadata>> {
    METADATA.get_or_init(|| RwLock::new(HashMap::new()))
}

fn plugin_metadata(plugin_id: &str) -> Option<AuPluginMetadata> {
    metadata_store()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(plugin_id)
        .cloned()
}

/// Registers validated AUv2 plugins with the processor registry.
pub fn register_plugins(plugins: impl IntoIterator<Item = AuPluginMetadata>) {
    let plugins = plugins.into_iter().collect::<Vec<_>>();
    {
        let mut metadata = metadata_store()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for plugin in &plugins {
            metadata.insert(plugin.processor_id.clone(), plugin.clone());
        }
    }
    registry::register(plugins.into_iter().map(registry_entry_for_plugin));
}

fn registry_entry_for_plugin(plugin: AuPluginMetadata) -> registry::Entry {
    let descriptor = Box::leak(Box::new(ProcessorDescriptor {
        name: Box::leak(plugin.name.clone().into_boxed_str()),
        params: &[],
        editor: Some(editor::DEFAULT_AU_EDITOR_DESCRIPTOR),
    }));
    let runtime = match plugin.role {
        registry::Role::Instrument => registry::RuntimeFactory::Instrument(create_au_instrument),
        registry::Role::Effect => registry::RuntimeFactory::Effect(create_au_effect),
    };
    registry::Entry::plugin_processor(
        plugin.processor_id,
        plugin.name,
        registry::Backend::Au,
        plugin.vendor,
        descriptor,
        runtime,
    )
}

fn create_au_instrument(
    slot: &SlotState,
    context: &InstrumentRuntimeContext<'_>,
) -> Result<Option<InstrumentRuntimeSpec>, RuntimeFactoryError> {
    let Some((metadata, descriptor)) = metadata_and_descriptor(slot)? else {
        return Ok(None);
    };
    log::trace!(
        target: "lilypalooza_au",
        "create AU instrument plugin_id={} sample_rate={} block_size={} state_bytes={}",
        metadata.processor_id,
        context.soundfont_settings.sample_rate,
        context.soundfont_settings.block_size,
        slot.state.0.len()
    );
    let shared = instantiate_shared(
        &metadata,
        descriptor,
        usize::try_from(context.soundfont_settings.sample_rate.max(1)).unwrap_or(44_100),
        context.soundfont_settings.block_size.max(1),
        &slot.state,
    )?;
    Ok(Some(InstrumentRuntimeSpec {
        processor: Box::new(AuProcessor {
            shared: shared.clone(),
            midi: Vec::new(),
        }),
        binding: Box::new(AuBinding { shared }),
    }))
}

fn create_au_effect(
    slot: &SlotState,
    context: &EffectRuntimeContext,
) -> Result<Option<EffectRuntimeSpec>, RuntimeFactoryError> {
    let Some((metadata, descriptor)) = metadata_and_descriptor(slot)? else {
        return Ok(None);
    };
    log::trace!(
        target: "lilypalooza_au",
        "create AU effect plugin_id={} sample_rate={} block_size={} state_bytes={}",
        metadata.processor_id,
        context.sample_rate,
        context.block_size,
        slot.state.0.len()
    );
    let shared = instantiate_shared(
        &metadata,
        descriptor,
        context.sample_rate,
        context.block_size,
        &slot.state,
    )?;
    Ok(Some(EffectRuntimeSpec {
        processor: Box::new(AuProcessor {
            shared: shared.clone(),
            midi: Vec::new(),
        }),
        binding: Some(Box::new(AuBinding { shared })),
    }))
}

fn metadata_and_descriptor(
    slot: &SlotState,
) -> Result<Option<(AuPluginMetadata, &'static ProcessorDescriptor)>, RuntimeFactoryError> {
    let lilypalooza_audio::ProcessorKind::Plugin { plugin_id } = &slot.kind else {
        return Ok(None);
    };
    let Some(metadata) = plugin_metadata(plugin_id) else {
        return Err(RuntimeFactoryError::Backend(format!(
            "AU plugin `{plugin_id}` is not registered"
        )));
    };
    let descriptor = registry::entry(plugin_id)
        .map(|entry| entry.descriptor)
        .ok_or_else(|| {
            RuntimeFactoryError::Backend(format!("AU plugin `{plugin_id}` is not registered"))
        })?;
    Ok(Some((metadata, descriptor)))
}

fn instantiate_shared(
    metadata: &AuPluginMetadata,
    descriptor: &'static ProcessorDescriptor,
    sample_rate: usize,
    block_size: usize,
    state: &ProcessorState,
) -> Result<Arc<Mutex<runtime::AuRuntime>>, RuntimeFactoryError> {
    let mut runtime =
        runtime::AuRuntime::instantiate(metadata, descriptor, sample_rate, block_size)
            .map_err(|error| RuntimeFactoryError::Backend(error.to_string()))?;
    runtime
        .load_state(state)
        .map_err(RuntimeFactoryError::State)?;
    Ok(Arc::new(Mutex::new(runtime)))
}

struct AuBinding {
    shared: Arc<Mutex<runtime::AuRuntime>>,
}

impl RuntimeBinding for AuBinding {
    fn controller(&self) -> Box<dyn Controller> {
        Box::new(AuController {
            shared: self.shared.clone(),
        })
    }

    fn latency_samples(&self) -> u32 {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .latency_samples()
    }

    fn prepare_destroy(&self) {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .prepare_destroy();
    }
}

struct AuController {
    shared: Arc<Mutex<runtime::AuRuntime>>,
}

impl Controller for AuController {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .descriptor()
    }

    fn parameters(&self) -> Vec<ParameterInfo> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .parameters()
    }

    fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_param(id)
    }

    fn set_param(&self, id: &str, normalized: f32) -> Result<(), ControllerError> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_param(id, normalized)
    }

    fn save_state(&self) -> Result<ProcessorState, ControllerError> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .save_state()
    }

    fn load_state(&self, state: &ProcessorState) -> Result<(), ControllerError> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .load_state(state)
            .map_err(|error| ControllerError::Backend(error.to_string()))
    }

    fn create_editor_session(
        &self,
    ) -> Result<Option<Box<dyn lilypalooza_audio::EditorSession>>, EditorError> {
        editor::create_editor_session(self.shared.clone())
    }
}

struct AuProcessor {
    shared: Arc<Mutex<runtime::AuRuntime>>,
    midi: Vec<MidiEvent>,
}

impl Processor for AuProcessor {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .descriptor()
    }

    fn set_param(&mut self, id: &str, normalized: f32) -> bool {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_param(id, normalized)
            .is_ok()
    }

    fn get_param(&self, id: &str) -> Option<f32> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_param(id)
            .ok()
    }

    fn save_state(&self) -> ProcessorState {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .save_state()
            .unwrap_or_default()
    }

    fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .load_state(state)
    }

    fn reset(&mut self) {
        self.midi.clear();
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reset();
    }

    fn latency_samples(&self) -> u32 {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .latency_samples()
    }
}

impl InstrumentProcessor for AuProcessor {
    fn handle_midi(&mut self, event: MidiEvent) {
        self.midi.push(event);
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
        let events = std::mem::take(&mut self.midi);
        if let Err(error) = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .render_instrument(left, right, &events)
        {
            log_runtime_error(error);
        }
    }
}

impl EffectProcessor for AuProcessor {
    fn process(
        &mut self,
        in_left: &[f32],
        in_right: &[f32],
        out_left: &mut [f32],
        out_right: &mut [f32],
    ) {
        if let Err(error) = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .process_effect(in_left, in_right, out_left, out_right)
        {
            out_left.copy_from_slice(in_left);
            out_right.copy_from_slice(in_right);
            log_runtime_error(error);
        }
    }
}

fn log_runtime_error(error: runtime::AuRuntimeError) {
    log::warn!("AU runtime error: {error}");
}
