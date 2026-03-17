use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, Stream, StreamConfig};
use midi_player::{Player, PlayerController, PositionObserver, Settings};

pub(crate) struct MidiPlayback {
    controller: PlayerController,
    position_observer: PositionObserver,
    _stream: Stream,
    soundfont_path: PathBuf,
    current_file: Option<PathBuf>,
}

impl MidiPlayback {
    pub(crate) fn new(soundfont_path: impl Into<PathBuf>) -> Result<Self, String> {
        let soundfont_path = soundfont_path.into();
        let host = cpal::default_host();
        let output_device = host
            .default_output_device()
            .ok_or_else(|| "No output audio device is available".to_string())?;
        let output_config = output_device.default_output_config().map_err(|error| {
            format!("Failed to query default output audio configuration: {error}")
        })?;

        if output_config.sample_format() != SampleFormat::F32 {
            return Err(format!(
                "Unsupported output sample format: {:?}. Only f32 is currently supported",
                output_config.sample_format()
            ));
        }

        let settings = Settings::builder()
            .sample_rate(output_config.sample_rate().0)
            .build();
        let (player, controller) = Player::new(
            soundfont_path
                .to_str()
                .ok_or_else(|| "Soundfont path contains invalid UTF-8".to_string())?,
            settings.clone(),
        )
        .map_err(|error| format!("Failed to initialize MIDI player: {error}"))?;
        let position_observer = controller.new_position_observer();

        let stream = build_f32_stream(output_device, output_config.config(), settings, player)?;
        stream
            .play()
            .map_err(|error| format!("Failed to start audio stream: {error}"))?;

        Ok(Self {
            controller,
            position_observer,
            _stream: stream,
            soundfont_path,
            current_file: None,
        })
    }

    pub(crate) fn soundfont_path(&self) -> &Path {
        &self.soundfont_path
    }

    pub(crate) fn current_file(&self) -> Option<&Path> {
        self.current_file.as_deref()
    }

    pub(crate) fn load_file(&mut self, path: Option<&Path>) -> Result<(), String> {
        match path {
            Some(path) => {
                self.controller
                    .set_file(Some(path.to_path_buf()))
                    .map_err(|error| format!("Failed to load MIDI file for playback: {error}"))?;
                self.current_file = Some(path.to_path_buf());
            }
            None => {
                self.controller.set_file(None::<PathBuf>).map_err(|error| {
                    format!("Failed to unload MIDI file from playback: {error}")
                })?;
                self.current_file = None;
            }
        }

        Ok(())
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.controller.is_playing()
    }

    pub(crate) fn play(&self) -> bool {
        self.controller.play()
    }

    pub(crate) fn pause(&mut self) {
        self.controller.stop();
    }

    pub(crate) fn jump_to_tick(&mut self, tick: u64) {
        self.controller.note_off_all();
        self.controller.set_position_ticks(tick);
    }

    pub(crate) fn position_ticks(&self) -> u64 {
        self.position_observer.ticks()
    }

    pub(crate) fn total_ticks(&self) -> u64 {
        self.position_observer.total_ticks()
    }

    pub(crate) fn track_count(&self) -> usize {
        self.controller.track_count()
    }

    pub(crate) fn set_track_muted(&mut self, track_index: usize, muted: bool) -> bool {
        self.controller.set_track_muted(track_index, muted)
    }

    pub(crate) fn set_track_solo(&mut self, track_index: usize, soloed: bool) -> bool {
        self.controller.set_track_solo(track_index, soloed)
    }
}

fn build_f32_stream(
    output_device: cpal::Device,
    output_config: StreamConfig,
    settings: Settings,
    player: Player,
) -> Result<Stream, String> {
    let channels = usize::from(output_config.channels.max(1));
    let buffer_capacity = match output_config.buffer_size {
        BufferSize::Fixed(size) => size as usize,
        BufferSize::Default => settings.audio_buffer_size as usize,
    }
    .max(1);
    let player = Arc::new(Mutex::new(player));
    let callback_player = Arc::clone(&player);
    let mut left = vec![0.0f32; buffer_capacity];
    let mut right = vec![0.0f32; buffer_capacity];

    output_device
        .build_output_stream(
            &output_config,
            move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                let sample_count = (data.len() / channels).max(1);

                if left.len() < sample_count {
                    left.resize(sample_count, 0.0);
                    right.resize(sample_count, 0.0);
                }

                if let Ok(mut player) = callback_player.lock() {
                    player.render(&mut left[..sample_count], &mut right[..sample_count]);
                } else {
                    for sample in &mut data[..] {
                        *sample = 0.0;
                    }
                    return;
                }

                for index in 0..sample_count {
                    data[channels * index] = left[index];

                    if channels >= 2 {
                        data[channels * index + 1] = right[index];
                    }

                    if channels > 2 {
                        let mono = (left[index] + right[index]) * 0.5;
                        for channel in 2..channels {
                            data[channels * index + channel] = mono;
                        }
                    }
                }
            },
            move |error| {
                eprintln!("Audio output stream error: {error}");
            },
            None,
        )
        .map_err(|error| format!("Failed to build audio output stream: {error}"))
}
