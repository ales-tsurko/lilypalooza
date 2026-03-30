use std::time::Duration;

use iced::widget::{button, container, row, slider, svg, text};
use iced::{ContentFit, Element, Fill, Font, Length, alignment};

use super::{FileMessage, LilyView, Message, PianoRollMessage, SoundfontStatus};
use crate::icons;
use crate::midi::{MidiRollData, TimeSignatureChange};
use crate::ui_style;

pub(super) const HEIGHT: f32 = 34.0;
const ICON_BUTTON_WIDTH: f32 = 34.0;
const ICON_BUTTON_HEIGHT: f32 = 22.0;
const ICON_SIZE: f32 = 14.0;

pub(super) fn view(app: &LilyView) -> Element<'_, Message> {
    let is_playing = app.piano_roll.playback_is_playing();

    let (
        total_ticks,
        normalized_position,
        time_label,
        musical_clock_label,
        tempo_label,
        meter_label,
    ) = if let Some(file) = app.piano_roll.current_file() {
        let total_ticks = file.data.total_ticks;
        let tick = app.piano_roll.playback_tick().min(total_ticks);
        let normalized = app.piano_roll.playback_position_normalized();
        let current_time = format_duration(ticks_to_duration(&file.data, tick));
        let total_time = format_duration(ticks_to_duration(&file.data, total_ticks));
        let musical_clock = musical_clock_short(&file.data, tick);
        let current_bpm = tempo_bpm_at_tick(&file.data, tick);
        let time_signature = time_signature_at_tick(&file.data, tick);

        (
            total_ticks,
            normalized,
            format!("{current_time} / {total_time}"),
            musical_clock,
            format!("{current_bpm:.1}"),
            format!(
                "{}/{}",
                time_signature.numerator, time_signature.denominator
            ),
        )
    } else {
        (
            0,
            0.0,
            "00:00.000 / 00:00.000".to_string(),
            "--:--".to_string(),
            "--.-".to_string(),
            "--/--".to_string(),
        )
    };

    let can_transport = app.playback.is_some() && total_ticks > 0;

    let play_pause_button = button(
        container(transport_icon(if is_playing {
            icons::pause()
        } else {
            icons::play()
        }))
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill),
    )
    .style(if is_playing {
        ui_style::button_active
    } else {
        ui_style::button_neutral
    })
    .padding(0)
    .width(Length::Fixed(ICON_BUTTON_WIDTH))
    .height(Length::Fixed(ICON_BUTTON_HEIGHT));
    let play_pause_button = if can_transport {
        play_pause_button.on_press(Message::PianoRoll(PianoRollMessage::TransportPlayPause))
    } else {
        play_pause_button
    };

    let rewind_button = button(
        container(transport_icon(icons::skip_back()))
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
    )
    .style(ui_style::button_neutral)
    .padding(0)
    .width(Length::Fixed(ICON_BUTTON_WIDTH))
    .height(Length::Fixed(ICON_BUTTON_HEIGHT));
    let rewind_button = if can_transport {
        rewind_button.on_press(Message::PianoRoll(PianoRollMessage::TransportRewind))
    } else {
        rewind_button
    };

    let seek_slider = slider(0.0..=1.0, normalized_position, |value| {
        Message::PianoRoll(PianoRollMessage::TransportSeekNormalized(value))
    })
    .step(0.001)
    .width(Fill);

    let soundfont_label = match (&app.soundfont_status, app.playback.as_ref()) {
        (_, Some(playback)) => playback
            .soundfont_path()
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| "Soundfont".to_string()),
        (SoundfontStatus::NotSelected, None) => "Set soundfont".to_string(),
        (SoundfontStatus::Ready(path), None) => path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| "Soundfont".to_string()),
        (SoundfontStatus::Error(error), None) => format!("Soundfont error: {error}"),
    };

    let soundfont_button = button(
        row![
            text("♬").size(ui_style::FONT_SIZE_UI_SM),
            text(soundfont_label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(Font::MONOSPACE),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center),
    )
    .style(ui_style::button_neutral)
    .padding([
        ui_style::PADDING_BUTTON_COMPACT_V,
        ui_style::PADDING_BUTTON_COMPACT_H,
    ])
    .on_press(Message::File(FileMessage::RequestSoundfont));

    container(
        row![
            row![play_pause_button, rewind_button]
                .spacing(ui_style::SPACE_XS)
                .align_y(alignment::Vertical::Center),
            text(time_label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(Font::MONOSPACE),
            text(musical_clock_label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(Font::MONOSPACE),
            seek_slider,
            row![
                transport_icon(icons::metronome()),
                text(tempo_label)
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(Font::MONOSPACE),
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
            text(meter_label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(Font::MONOSPACE),
            soundfont_button,
        ]
        .width(Fill)
        .align_y(alignment::Vertical::Center)
        .spacing(ui_style::SPACE_SM),
    )
    .width(Fill)
    .height(Length::Fixed(HEIGHT))
    .padding([
        ui_style::PADDING_STATUS_BAR_V,
        ui_style::PADDING_STATUS_BAR_H,
    ])
    .style(ui_style::workspace_toolbar_surface)
    .into()
}

fn transport_icon(icon: svg::Handle) -> Element<'static, Message> {
    svg(icon)
        .width(Length::Fixed(ICON_SIZE))
        .height(Length::Fixed(ICON_SIZE))
        .content_fit(ContentFit::Contain)
        .style(ui_style::svg_window_control)
        .into()
}

fn tempo_bpm_at_tick(data: &MidiRollData, tick: u64) -> f32 {
    let micros_per_quarter = data
        .tempo_changes
        .iter()
        .take_while(|tempo| tempo.tick <= tick)
        .last()
        .map(|tempo| tempo.micros_per_quarter)
        .unwrap_or(500_000);

    if micros_per_quarter == 0 {
        0.0
    } else {
        60_000_000.0 / micros_per_quarter as f32
    }
}

fn time_signature_at_tick(data: &MidiRollData, tick: u64) -> TimeSignatureChange {
    data.time_signatures
        .iter()
        .take_while(|signature| signature.tick <= tick)
        .last()
        .copied()
        .unwrap_or(TimeSignatureChange {
            tick: 0,
            numerator: 4,
            denominator: 4,
        })
}

fn ticks_to_duration(data: &MidiRollData, tick: u64) -> Duration {
    let clamped_tick = tick.min(data.total_ticks);
    let ppq = u64::from(data.ppq.max(1));
    let mut total_micros = 0_u128;
    let mut segment_start_tick = 0_u64;
    let mut micros_per_quarter = 500_000_u64;
    let mut tempo_iter = data.tempo_changes.iter().peekable();

    while segment_start_tick < clamped_tick {
        if let Some(next_tempo) = tempo_iter.peek()
            && next_tempo.tick <= segment_start_tick
        {
            micros_per_quarter = u64::from(next_tempo.micros_per_quarter.max(1));
            let _ = tempo_iter.next();
            continue;
        }

        let next_tempo_tick = tempo_iter
            .peek()
            .map(|tempo| tempo.tick)
            .unwrap_or(clamped_tick);
        let segment_end_tick = next_tempo_tick.min(clamped_tick);
        let segment_ticks = segment_end_tick.saturating_sub(segment_start_tick);

        total_micros = total_micros.saturating_add(
            (u128::from(segment_ticks) * u128::from(micros_per_quarter)) / u128::from(ppq),
        );
        segment_start_tick = segment_end_tick;
    }

    Duration::from_micros(total_micros.min(u128::from(u64::MAX)) as u64)
}

fn format_duration(duration: Duration) -> String {
    let total_millis = duration.as_millis() as u64;
    let minutes = total_millis / 60_000;
    let seconds = (total_millis % 60_000) / 1_000;
    let millis = total_millis % 1_000;

    format!("{minutes:02}:{seconds:02}.{millis:03}")
}

fn musical_clock_short(data: &MidiRollData, tick: u64) -> String {
    let tick = tick.min(data.total_ticks);
    let bar_start_tick = data
        .bar_lines
        .iter()
        .copied()
        .take_while(|bar_tick| *bar_tick <= tick)
        .last()
        .unwrap_or(0);
    let bar_index = data
        .bar_lines
        .iter()
        .take_while(|bar_tick| **bar_tick <= tick)
        .count()
        .max(1);
    let signature = time_signature_at_tick(data, tick);
    let beat_ticks = beat_step_ticks(data.ppq, signature).max(1);
    let beat = ((tick.saturating_sub(bar_start_tick) / beat_ticks) + 1)
        .clamp(1, u64::from(signature.numerator.max(1)));

    format!("{bar_index}:{beat}")
}

fn beat_step_ticks(ppq: u16, signature: TimeSignatureChange) -> u64 {
    let quarter = u64::from(ppq.max(1));
    let denominator = u64::from(signature.denominator.max(1));

    quarter.saturating_mul(4) / denominator
}
