use lilypalooza_audio::{ProcessorKind, ProcessorState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ProcessorPresetLibrary {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    presets: Vec<ProcessorPreset>,
    next_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProcessorPreset {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: ProcessorKind,
    pub(crate) origin: ProcessorPresetOrigin,
    pub(crate) state: ProcessorState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProcessorPresetOrigin {
    User,
}

impl ProcessorPresetLibrary {
    pub(crate) fn save_user_preset(
        &mut self,
        name: impl Into<String>,
        kind: ProcessorKind,
        state: ProcessorState,
    ) -> String {
        let id = format!("user-{}", self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.presets.push(ProcessorPreset {
            id: id.clone(),
            name: name.into(),
            kind,
            origin: ProcessorPresetOrigin::User,
            state,
        });
        id
    }

    pub(crate) fn presets_for(&self, kind: &ProcessorKind) -> Vec<&ProcessorPreset> {
        self.presets
            .iter()
            .filter(|preset| &preset.kind == kind)
            .collect()
    }

    pub(crate) fn state_for(&self, kind: &ProcessorKind, id: &str) -> Option<&ProcessorState> {
        self.presets
            .iter()
            .find(|preset| &preset.kind == kind && preset.id == id)
            .map(|preset| &preset.state)
    }

    pub(crate) fn rename_user_preset(
        &mut self,
        kind: &ProcessorKind,
        id: &str,
        name: impl Into<String>,
    ) -> bool {
        let name = name.into();
        let Some(preset) = self
            .presets
            .iter_mut()
            .find(|preset| &preset.kind == kind && preset.id == id)
        else {
            return false;
        };
        if name.trim().is_empty() {
            return false;
        }
        preset.name = name;
        true
    }

    pub(crate) fn delete_user_preset(&mut self, kind: &ProcessorKind, id: &str) -> bool {
        let Some(index) = self
            .presets
            .iter()
            .position(|preset| &preset.kind == kind && preset.id == id)
        else {
            return false;
        };
        self.presets.remove(index);
        true
    }
}
