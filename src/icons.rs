use iced::widget::svg;

const ARROW_UP_TO_LINE: &[u8] = include_bytes!("../assets/icons/arrow-up-to-line.svg");
const MUSIC_4: &[u8] = include_bytes!("../assets/icons/music-4.svg");
const PIANO: &[u8] = include_bytes!("../assets/icons/piano.svg");
const FILE_PEN: &[u8] = include_bytes!("../assets/icons/file-pen.svg");

pub(crate) fn arrow_up_to_line() -> svg::Handle {
    svg::Handle::from_memory(ARROW_UP_TO_LINE)
}

pub(crate) fn music_4() -> svg::Handle {
    svg::Handle::from_memory(MUSIC_4)
}

pub(crate) fn piano() -> svg::Handle {
    svg::Handle::from_memory(PIANO)
}

pub(crate) fn file_pen() -> svg::Handle {
    svg::Handle::from_memory(FILE_PEN)
}
