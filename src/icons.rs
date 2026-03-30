use iced::widget::svg;

const ARROW_LEFT: &[u8] = include_bytes!("../assets/icons/arrow-left.svg");
const ARROW_RIGHT: &[u8] = include_bytes!("../assets/icons/arrow-right.svg");
const BRUSH_CLEANING: &[u8] = include_bytes!("../assets/icons/brush-cleaning.svg");
const ELLIPSIS_VERTICAL: &[u8] = include_bytes!("../assets/icons/ellipsis-vertical.svg");
const MUSIC_4: &[u8] = include_bytes!("../assets/icons/music-4.svg");
const METRONOME: &[u8] = include_bytes!("../assets/icons/metronome.svg");
const PIANO: &[u8] = include_bytes!("../assets/icons/piano.svg");
const PLAY: &[u8] = include_bytes!("../assets/icons/play.svg");
const PAUSE: &[u8] = include_bytes!("../assets/icons/pause.svg");
const FILE_PEN: &[u8] = include_bytes!("../assets/icons/file-pen.svg");
const SCROLL_TEXT: &[u8] = include_bytes!("../assets/icons/scroll-text.svg");
const SKIP_BACK: &[u8] = include_bytes!("../assets/icons/skip-back.svg");

pub(crate) fn arrow_left() -> svg::Handle {
    svg::Handle::from_memory(ARROW_LEFT)
}

pub(crate) fn arrow_right() -> svg::Handle {
    svg::Handle::from_memory(ARROW_RIGHT)
}

pub(crate) fn brush_cleaning() -> svg::Handle {
    svg::Handle::from_memory(BRUSH_CLEANING)
}

pub(crate) fn music_4() -> svg::Handle {
    svg::Handle::from_memory(MUSIC_4)
}

pub(crate) fn ellipsis_vertical() -> svg::Handle {
    svg::Handle::from_memory(ELLIPSIS_VERTICAL)
}

pub(crate) fn metronome() -> svg::Handle {
    svg::Handle::from_memory(METRONOME)
}

pub(crate) fn piano() -> svg::Handle {
    svg::Handle::from_memory(PIANO)
}

pub(crate) fn play() -> svg::Handle {
    svg::Handle::from_memory(PLAY)
}

pub(crate) fn pause() -> svg::Handle {
    svg::Handle::from_memory(PAUSE)
}

pub(crate) fn file_pen() -> svg::Handle {
    svg::Handle::from_memory(FILE_PEN)
}

pub(crate) fn scroll_text() -> svg::Handle {
    svg::Handle::from_memory(SCROLL_TEXT)
}

pub(crate) fn skip_back() -> svg::Handle {
    svg::Handle::from_memory(SKIP_BACK)
}
