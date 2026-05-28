use super::*;

fn track_runtime_build_context<'a>(
    settings: &'a AudioEngineSettings,
    mixer: &'a MixerState,
    master_input: NodeId,
    bus_inputs: &'a HashMap<BusId, NodeId>,
    soundfonts: &'a HashMap<String, LoadedSoundfont>,
    soundfont_settings: SoundfontSynthSettings,
) -> TrackRuntimeBuildContext<'a> {
    TrackRuntimeBuildContext {
        settings,
        mixer,
        master_input,
        bus_inputs,
        soundfont_resources: mixer.soundfonts(),
        soundfonts,
        soundfont_settings,
    }
}

fn new_track_runtime(
    context: &KnystContext,
    commands: &mut MultiThreadedKnystCommands,
    track: &Track,
    build: TrackRuntimeBuildContext<'_>,
) -> Result<TrackRuntime, MixerRuntimeError> {
    TrackRuntime::new(context, commands, track, build)
}

struct TrackRuntimeSyncTarget<'a> {
    track: &'a Track,
    runtime: &'a mut Option<TrackRuntime>,
    settings: &'a AudioEngineSettings,
    mixer: &'a MixerState,
    master_input: NodeId,
    bus_inputs: HashMap<BusId, NodeId>,
    soundfonts: &'a HashMap<String, LoadedSoundfont>,
    soundfont_settings: SoundfontSynthSettings,
}

impl TrackRuntimeSyncTarget<'_> {
    fn needs_runtime(&self) -> bool {
        track_needs_runtime(self.track)
    }

    fn remove_runtime(&mut self) -> bool {
        let Some(runtime) = self.runtime.take() else {
            return false;
        };
        runtime.free();
        true
    }

    fn insert_new_runtime(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
    ) -> Result<(), MixerRuntimeError> {
        *self.runtime = Some(new_track_runtime(
            context,
            commands,
            self.track,
            track_runtime_build_context(
                self.settings,
                self.mixer,
                self.master_input,
                &self.bus_inputs,
                self.soundfonts,
                self.soundfont_settings,
            ),
        )?);
        Ok(())
    }

    fn sync_source(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
    ) -> Result<Option<bool>, MixerRuntimeError> {
        let Some(runtime) = self.runtime.as_mut() else {
            return Ok(None);
        };
        runtime
            .sync_source(
                context,
                commands,
                self.track,
                self.mixer.soundfonts(),
                self.soundfonts,
                self.soundfont_settings,
            )
            .map(Some)
    }

    fn rebuild_existing_effects(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
    ) -> Option<bool> {
        let mut existing = self.runtime.take()?;
        let reused = existing.rebuild_effects(context, commands, self.track, self.settings);
        if reused {
            *self.runtime = Some(existing);
        } else {
            existing.free();
        }
        Some(reused)
    }
}

pub(crate) struct MixerRuntime {
    master: MasterRuntime,
    metronome: MetronomeRuntime,
    pub(super) tracks: Vec<Option<TrackRuntime>>,
    buses: HashMap<BusId, BusRuntime>,
    soundfonts: HashMap<String, LoadedSoundfont>,
    soundfont_settings: SoundfontSynthSettings,
    meter_settings: AudioEngineSettings,
}

impl MixerRuntime {
    pub(crate) fn free(self) {
        self.master.free();
        self.metronome.free();
        for runtime in self.tracks.into_iter().flatten() {
            runtime.free();
        }
        for runtime in self.buses.into_values() {
            runtime.free();
        }
    }
}

impl MixerRuntime {
    pub(crate) fn meter_settings(&self) -> AudioEngineSettings {
        self.meter_settings
    }

    pub(crate) fn controller(
        &self,
        mixer: &MixerState,
        address: SlotAddress,
    ) -> Result<Option<Box<dyn Controller>>, MixerRuntimeError> {
        let Some(strip) = mixer.strip_by_index(address.strip_index) else {
            return Ok(None);
        };
        let Some(_slot) = strip.slot(address.slot_index) else {
            return Ok(None);
        };

        if address.strip_index == 0 {
            return Ok(self
                .master
                .effects
                .get(address.slot_index.checked_sub(1).unwrap_or(usize::MAX))
                .and_then(|runtime| runtime.as_ref())
                .and_then(EffectRuntime::controller));
        }

        if let Some(track_offset) = address.strip_index.checked_sub(1)
            && track_offset < mixer.track_count()
        {
            let Some(runtime) = self
                .tracks
                .get(track_offset)
                .and_then(|runtime| runtime.as_ref())
            else {
                return Ok(None);
            };
            return Ok(match address.slot_index {
                0 => runtime
                    .instrument
                    .as_ref()
                    .map(TrackInstrumentRuntime::controller),
                effect_index => runtime
                    .effects
                    .get(effect_index - 1)
                    .and_then(|runtime| runtime.as_ref())
                    .and_then(EffectRuntime::controller),
            });
        }

        let Some(bus_id) = strip.bus_id else {
            return Ok(None);
        };
        let Some(runtime) = self.buses.get(&bus_id) else {
            return Ok(None);
        };
        Ok(runtime
            .effects
            .get(address.slot_index.checked_sub(1).unwrap_or(usize::MAX))
            .and_then(|runtime| runtime.as_ref())
            .and_then(EffectRuntime::controller))
    }

    pub(crate) fn instrument_handle(&self, track_id: TrackId) -> Option<InstrumentRuntimeHandle> {
        Some(
            self.tracks
                .get(track_id.index())?
                .as_ref()?
                .instrument
                .as_ref()?
                .handle
                .clone(),
        )
    }

    pub(crate) fn metronome_handle(&self) -> InstrumentRuntimeHandle {
        self.metronome.handle.clone()
    }

    pub(crate) fn set_metronome_gain_db(&self, gain_db: f32) {
        self.metronome.shared.set_gain_db(gain_db);
    }

    pub(crate) fn set_metronome_pitch(&self, pitch: f32) {
        self.metronome.shared.set_pitch(pitch);
    }

    pub(crate) fn meter_snapshot(&self, mixer: &MixerState) -> MixerMeterSnapshot {
        MixerMeterSnapshot {
            main: self.master.meter.snapshot(),
            tracks: mixer
                .tracks()
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    self.tracks
                        .get(index)
                        .and_then(|runtime| runtime.as_ref())
                        .map_or_else(StripMeterSnapshot::default, |runtime| {
                            runtime.meter.snapshot()
                        })
                })
                .collect(),
            buses: mixer
                .buses()
                .iter()
                .filter_map(|bus| {
                    let bus_id = bus.bus_id?;
                    Some((
                        bus_id,
                        self.buses
                            .get(&bus_id)
                            .map_or_else(StripMeterSnapshot::default, |runtime| {
                                runtime.meter.snapshot()
                            }),
                    ))
                })
                .collect(),
        }
    }

    pub(crate) fn meter_snapshot_window(
        &self,
        mixer: &MixerState,
        track_range: std::ops::Range<usize>,
        bus_range: std::ops::Range<usize>,
    ) -> MixerMeterSnapshotWindow {
        let track_end = track_range.end.min(mixer.tracks().len());
        let bus_end = bus_range.end.min(mixer.buses().len());

        MixerMeterSnapshotWindow {
            main: self.master.meter.snapshot(),
            tracks: mixer
                .tracks()
                .get(track_range.start.min(track_end)..track_end)
                .unwrap_or(&[])
                .iter()
                .enumerate()
                .map(|(offset, _)| {
                    let index = track_range.start + offset;
                    self.tracks
                        .get(index)
                        .and_then(|runtime| runtime.as_ref())
                        .map_or_else(StripMeterSnapshot::default, |runtime| {
                            runtime.meter.snapshot()
                        })
                })
                .collect(),
            buses: mixer
                .buses()
                .get(bus_range.start.min(bus_end)..bus_end)
                .unwrap_or(&[])
                .iter()
                .enumerate()
                .filter_map(|(offset, _)| {
                    let index = bus_range.start + offset;
                    let id = mixer.buses().get(index)?.bus_id?;
                    Some(
                        self.buses
                            .get(&id)
                            .map_or_else(StripMeterSnapshot::default, |runtime| {
                                runtime.meter.snapshot()
                            }),
                    )
                })
                .collect(),
        }
    }

    pub(crate) fn reset_meters(&self) {
        self.master.meter.reset();
        for runtime in self.tracks.iter().flatten() {
            runtime.meter.reset();
        }
        for runtime in self.buses.values() {
            runtime.meter.reset();
        }
    }

    pub(crate) fn reset_master_meter(&self) {
        self.master.meter.reset();
    }

    pub(crate) fn reset_track_meter(&self, id: TrackId) -> Result<(), MixerRuntimeError> {
        let runtime = self
            .tracks
            .get(id.index())
            .ok_or(MixerError::InvalidTrackId(id))?
            .as_ref()
            .ok_or(MixerError::InvalidTrackId(id))?;
        runtime.meter.reset();
        Ok(())
    }

    pub(crate) fn reset_bus_meter(&self, id: BusId) -> Result<(), MixerRuntimeError> {
        let runtime = self.buses.get(&id).ok_or(MixerError::InvalidBusId(id))?;
        runtime.meter.reset();
        Ok(())
    }

    pub(crate) fn attach(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        settings: &AudioEngineSettings,
        mixer: &MixerState,
    ) -> Result<Self, MixerRuntimeError> {
        context.with_activation(|| {
            let master = MasterRuntime::new(context, commands, settings, mixer);
            let metronome = MetronomeRuntime::new(context, master.input_node(), settings);
            let soundfont_settings =
                SoundfontSynthSettings::new(settings.sample_rate as i32, settings.block_size);

            let mut buses = HashMap::with_capacity(mixer.buses().len());
            for bus_track in mixer.buses() {
                if let Some(bus_id) = bus_track.bus_id {
                    buses.insert(
                        bus_id,
                        BusRuntime::new(context, commands, settings, bus_track, mixer),
                    );
                }
            }

            let mut runtime = Self {
                master,
                metronome,
                tracks: Vec::with_capacity(mixer.tracks().len()),
                buses,
                soundfonts: HashMap::new(),
                soundfont_settings,
                meter_settings: *settings,
            };
            runtime.sync_soundfonts(mixer)?;

            let mut tracks = Vec::with_capacity(mixer.tracks().len());
            for track in mixer.tracks() {
                tracks.push(if track_needs_runtime(track) {
                    let bus_inputs = runtime.bus_input_nodes();
                    Some(TrackRuntime::new(
                        context,
                        commands,
                        track,
                        TrackRuntimeBuildContext {
                            settings,
                            mixer,
                            master_input: runtime.master.input_node(),
                            bus_inputs: &bus_inputs,
                            soundfont_resources: mixer.soundfonts(),
                            soundfonts: &runtime.soundfonts,
                            soundfont_settings: runtime.soundfont_settings,
                        },
                    )?)
                } else {
                    None
                });
            }
            runtime.tracks = tracks;
            runtime.sync_all_routing(context, commands, mixer)?;
            runtime.sync_all_levels(mixer);
            Ok(runtime)
        })
    }

    pub(crate) fn sync_soundfonts(&mut self, mixer: &MixerState) -> Result<(), MixerRuntimeError> {
        let resources: HashMap<_, _> = mixer
            .soundfonts()
            .iter()
            .map(|resource| (resource.id.clone(), resource))
            .collect();

        self.soundfonts.retain(|id, _| resources.contains_key(id));

        for resource in mixer.soundfonts() {
            let should_reload = self
                .soundfonts
                .get(&resource.id)
                .is_none_or(|loaded| loaded.path != resource.path);
            if should_reload {
                let loaded = LoadedSoundfont::load(resource)?;
                self.soundfonts.insert(resource.id.clone(), loaded);
            }
        }

        Ok(())
    }

    pub(crate) fn sync_tracks_after_soundfonts_changed(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
    ) -> Result<(), MixerRuntimeError> {
        for (track_id, track) in mixer.tracks_with_ids() {
            if track.instrument_slot().is_none() {
                continue;
            }
            if matches!(
                self.sync_track_instrument(context, commands, mixer, track_id)?,
                TrackInstrumentSync::GraphChanged
            ) {
                self.sync_track_routing(context, commands, mixer, track_id)?;
            }
        }
        Ok(())
    }

    pub(crate) fn add_bus(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        bus_id: BusId,
    ) -> Result<(), MixerRuntimeError> {
        let bus_track = mixer.bus(bus_id)?;
        context.with_activation(|| {
            self.buses.insert(
                bus_id,
                BusRuntime::new(context, commands, &self.meter_settings, bus_track, mixer),
            );
        });
        self.sync_bus_routing(context, commands, mixer, bus_id)?;
        self.sync_all_levels(mixer);
        Ok(())
    }

    pub(crate) fn remove_bus(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        bus_id: BusId,
    ) -> Result<(), MixerRuntimeError> {
        if let Some(runtime) = self.buses.remove(&bus_id) {
            runtime.free();
        }
        self.sync_all_routing_no_create(commands, mixer)?;
        self.sync_all_levels(mixer);
        Ok(())
    }

    pub(crate) fn sync_track_instrument(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        track_id: TrackId,
    ) -> Result<TrackInstrumentSync, MixerRuntimeError> {
        let mut target = self.track_runtime_sync_target(mixer, track_id)?;
        if !target.needs_runtime() {
            target.remove_runtime();
            return Ok(TrackInstrumentSync::GraphChanged);
        }
        if target.sync_source(context, commands)? == Some(true) {
            return Ok(TrackInstrumentSync::UpdatedInPlace);
        }
        if target.runtime.is_none() {
            target.insert_new_runtime(context, commands)?;
        }
        Ok(TrackInstrumentSync::GraphChanged)
    }

    pub(crate) fn sync_track_effects(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        track_id: TrackId,
    ) -> Result<bool, MixerRuntimeError> {
        let mut target = self.track_runtime_sync_target(mixer, track_id)?;
        if !target.needs_runtime() {
            return Ok(target.remove_runtime());
        }
        if target.rebuild_existing_effects(context, commands) == Some(true) {
            return Ok(false);
        }
        target.insert_new_runtime(context, commands)?;
        Ok(true)
    }

    fn track_runtime_sync_target<'a>(
        &'a mut self,
        mixer: &'a MixerState,
        track_id: TrackId,
    ) -> Result<TrackRuntimeSyncTarget<'a>, MixerRuntimeError> {
        let track = mixer.track(track_id)?;
        let master_input = self.master.input_node();
        let bus_inputs = self.bus_input_nodes();
        let runtime = self
            .tracks
            .get_mut(track_id.index())
            .ok_or(MixerError::InvalidTrackId(track_id))?;
        Ok(TrackRuntimeSyncTarget {
            track,
            runtime,
            settings: &self.meter_settings,
            mixer,
            master_input,
            bus_inputs,
            soundfonts: &self.soundfonts,
            soundfont_settings: self.soundfont_settings,
        })
    }

    pub(crate) fn sync_track_strip(
        &mut self,
        mixer: &MixerState,
        track_id: TrackId,
    ) -> Result<(), MixerRuntimeError> {
        let amplitude = track_effective_amplitude(mixer, mixer.track(track_id)?);
        let track = mixer.track(track_id)?;
        let runtime = self
            .tracks
            .get_mut(track_id.index())
            .ok_or(MixerError::InvalidTrackId(track_id))?;
        if let Some(runtime) = runtime.as_mut() {
            runtime.apply_strip(track, amplitude);
        }
        Ok(())
    }

    pub(crate) fn sync_bus_strip(
        &mut self,
        mixer: &MixerState,
        bus_id: BusId,
    ) -> Result<(), MixerRuntimeError> {
        let amplitude = bus_effective_amplitude(mixer.bus(bus_id)?);
        let bus = mixer.bus(bus_id)?;
        let runtime = self
            .buses
            .get_mut(&bus_id)
            .ok_or(MixerError::InvalidBusId(bus_id))?;
        runtime.apply_strip(bus, amplitude);
        Ok(())
    }

    pub(crate) fn sync_bus_effects(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        bus_id: BusId,
    ) -> Result<(), MixerRuntimeError> {
        let bus = mixer.bus(bus_id)?;
        let runtime = self
            .buses
            .get_mut(&bus_id)
            .ok_or(MixerError::InvalidBusId(bus_id))?;
        runtime.rebuild_effects(context, commands, bus, &self.meter_settings);
        Ok(())
    }

    pub(crate) fn sync_master_effects(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
    ) -> Result<(), MixerRuntimeError> {
        self.master
            .rebuild_effects(context, commands, mixer.master(), &self.meter_settings);
        Ok(())
    }

    pub(crate) fn sync_track_routing(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        track_id: TrackId,
    ) -> Result<(), MixerRuntimeError> {
        let bus_inputs = self.bus_input_nodes();
        let pdc_plan = self.pdc_plan(mixer);
        let track = mixer.track(track_id)?;
        let runtime_slot = self
            .tracks
            .get_mut(track_id.index())
            .ok_or(MixerError::InvalidTrackId(track_id))?;
        let needs_rebuild = runtime_slot
            .as_ref()
            .is_some_and(|runtime| !runtime.matches_signal_path(track));
        if needs_rebuild && let Some(old_runtime) = runtime_slot.take() {
            old_runtime.free();
            *runtime_slot = Some(TrackRuntime::new(
                context,
                commands,
                track,
                TrackRuntimeBuildContext {
                    settings: &self.meter_settings,
                    mixer,
                    master_input: self.master.input_node(),
                    bus_inputs: &bus_inputs,
                    soundfont_resources: mixer.soundfonts(),
                    soundfonts: &self.soundfonts,
                    soundfont_settings: self.soundfont_settings,
                },
            )?);
            return Ok(());
        }
        if let Some(runtime) = runtime_slot.as_mut() {
            let targets = RoutingTargets {
                master_input: self.master.input_node(),
                bus_inputs: &bus_inputs,
                pdc_plan: &pdc_plan,
            };
            runtime.sync_routing(context, commands, mixer, targets, track)?;
        }
        Ok(())
    }

    pub(crate) fn sync_bus_routing(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        bus_id: BusId,
    ) -> Result<(), MixerRuntimeError> {
        let bus_inputs = self.bus_input_nodes();
        let pdc_plan = self.pdc_plan(mixer);
        let bus = mixer.bus(bus_id)?;
        let runtime = self
            .buses
            .get_mut(&bus_id)
            .ok_or(MixerError::InvalidBusId(bus_id))?;
        let targets = RoutingTargets {
            master_input: self.master.input_node(),
            bus_inputs: &bus_inputs,
            pdc_plan: &pdc_plan,
        };
        runtime.sync_routing(context, commands, mixer, targets, bus)?;
        Ok(())
    }

    pub(crate) fn sync_all_routing(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
    ) -> Result<(), MixerRuntimeError> {
        let bus_inputs = self.bus_input_nodes();
        let pdc_plan = self.pdc_plan(mixer);
        let targets = RoutingTargets {
            master_input: self.master.input_node(),
            bus_inputs: &bus_inputs,
            pdc_plan: &pdc_plan,
        };
        for (track_id, track) in mixer.tracks_with_ids() {
            if let Some(runtime) = self
                .tracks
                .get_mut(track_id.index())
                .ok_or(MixerError::InvalidTrackId(track_id))?
                .as_mut()
            {
                runtime.sync_routing(context, commands, mixer, targets, track)?;
            }
        }
        let bus_ids: Vec<_> = mixer.buses_with_ids().map(|(bus_id, _)| bus_id).collect();
        for bus_id in bus_ids {
            let bus = mixer.bus(bus_id)?;
            if let Some(runtime) = self.buses.get_mut(&bus_id) {
                runtime.sync_routing(context, commands, mixer, targets, bus)?;
            }
        }
        self.sync_all_levels(mixer);
        Ok(())
    }

    pub(crate) fn sync_all_send_levels(&self, mixer: &MixerState) {
        for (track_id, track) in mixer.tracks_with_ids() {
            if let Some(runtime) = self
                .tracks
                .get(track_id.index())
                .and_then(|runtime| runtime.as_ref())
            {
                runtime.sync_send_levels(&track.routing.sends);
            }
        }
        for (bus_id, bus) in mixer.buses_with_ids() {
            if let Some(runtime) = self.buses.get(&bus_id) {
                runtime.sync_send_levels(&bus.routing.sends);
            }
        }
    }

    pub(crate) fn sync_slot_bypass(
        &self,
        mixer: &MixerState,
        address: SlotAddress,
    ) -> Result<(), MixerRuntimeError> {
        let Some(slot) = mixer.slot(address) else {
            return Err(MixerError::InvalidSlotAddress {
                strip_index: address.strip_index,
                slot_index: address.slot_index,
            }
            .into());
        };
        let effect_index = address.slot_index.saturating_sub(1);
        match address.strip_index {
            0 => {
                self.master.sync_effect_bypass(effect_index, slot.bypassed);
            }
            strip_index if strip_index <= mixer.track_count() => {
                let track_id = TrackId((strip_index - 1) as u16);
                let runtime = self
                    .tracks
                    .get(track_id.index())
                    .ok_or(MixerError::InvalidTrackId(track_id))?;
                if let Some(runtime) = runtime.as_ref() {
                    runtime.sync_effect_bypass(effect_index, slot.bypassed);
                }
            }
            _ => {
                let Some(bus_id) = mixer
                    .strip_by_index(address.strip_index)
                    .and_then(|t| t.bus_id)
                else {
                    return Err(MixerError::InvalidSlotAddress {
                        strip_index: address.strip_index,
                        slot_index: address.slot_index,
                    }
                    .into());
                };
                if let Some(runtime) = self.buses.get(&bus_id) {
                    runtime.sync_effect_bypass(effect_index, slot.bypassed);
                }
            }
        }
        Ok(())
    }

    pub(super) fn sync_all_routing_no_create(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
    ) -> Result<(), MixerRuntimeError> {
        let bus_inputs = self.bus_input_nodes();
        let pdc_plan = self.pdc_plan(mixer);
        let targets = RoutingTargets {
            master_input: self.master.input_node(),
            bus_inputs: &bus_inputs,
            pdc_plan: &pdc_plan,
        };
        for (track_id, track) in mixer.tracks_with_ids() {
            let runtime = self
                .tracks
                .get_mut(track_id.index())
                .ok_or(MixerError::InvalidTrackId(track_id))?;
            if let Some(runtime) = runtime.as_mut() {
                runtime.sync_routing_existing(commands, mixer, targets, track)?;
            }
        }
        for (bus_id, bus) in mixer.buses_with_ids() {
            if let Some(runtime) = self.buses.get_mut(&bus_id) {
                runtime.sync_routing_existing(commands, mixer, targets, bus)?;
            }
        }
        Ok(())
    }

    pub(crate) fn sync_all_levels(&mut self, mixer: &MixerState) {
        self.master.set_level(
            db_to_amplitude(mixer.master().state.gain_db),
            mixer.master().state.pan,
        );
        let track_amplitudes: Vec<_> = mixer
            .tracks()
            .iter()
            .map(|track| track_effective_amplitude(mixer, track))
            .collect();
        for (runtime, (track, amplitude)) in self
            .tracks
            .iter_mut()
            .zip(mixer.tracks().iter().zip(track_amplitudes))
        {
            if let Some(runtime) = runtime.as_mut() {
                runtime.apply_strip(track, amplitude);
            }
        }
        for (bus_id, bus) in mixer.buses_with_ids() {
            if let Some(runtime) = self.buses.get_mut(&bus_id) {
                runtime.apply_strip(bus, bus_effective_amplitude(bus));
            }
        }
    }

    pub(super) fn bus_input_nodes(&self) -> HashMap<BusId, NodeId> {
        self.buses
            .iter()
            .map(|(bus_id, runtime)| (*bus_id, runtime.input_node()))
            .collect()
    }

    pub(super) fn pdc_plan(&self, mixer: &MixerState) -> PdcPlan {
        let track_latencies = mixer
            .tracks_with_ids()
            .map(|(track_id, track)| {
                let latency = self
                    .tracks
                    .get(track_id.index())
                    .and_then(|runtime| runtime.as_ref())
                    .map_or_else(StripLatency::default, |runtime| runtime.latencies(track));
                (track_id, latency)
            })
            .collect();
        let bus_effect_latencies = mixer
            .buses_with_ids()
            .map(|(bus_id, bus)| {
                let latency = self
                    .buses
                    .get(&bus_id)
                    .map_or(0, |runtime| runtime.latencies(bus, 0).output);
                (bus_id, latency)
            })
            .collect();
        compute_pdc_plan_from_latencies(mixer, &track_latencies, &bus_effect_latencies)
    }
}
