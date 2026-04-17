use iced::widget::svg;

const ARROW_LEFT: &[u8] = include_bytes!("../assets/icons/arrow-left.svg");
const ARROW_RIGHT: &[u8] = include_bytes!("../assets/icons/arrow-right.svg");
const BRUSH_CLEANING: &[u8] = include_bytes!("../assets/icons/brush-cleaning.svg");
const CHEVRON_RIGHT: &[u8] = include_bytes!("../assets/icons/chevron-right.svg");
const CHEVRON_DOWN: &[u8] = include_bytes!("../assets/icons/chevron-down.svg");
const CIRCLE_ALERT: &[u8] = include_bytes!("../assets/icons/circle-alert.svg");
const CIRCLE_X: &[u8] = include_bytes!("../assets/icons/circle-x.svg");
const ELLIPSIS_VERTICAL: &[u8] = include_bytes!("../assets/icons/ellipsis-vertical.svg");
const MUSIC_4: &[u8] = include_bytes!("../assets/icons/music-4.svg");
const METRONOME: &[u8] = include_bytes!("../assets/icons/metronome.svg");
const PIANO: &[u8] = include_bytes!("../assets/icons/piano.svg");
const PLAY: &[u8] = include_bytes!("../assets/icons/play.svg");
const PLUS: &[u8] = include_bytes!("../assets/icons/plus.svg");
const PAUSE: &[u8] = include_bytes!("../assets/icons/pause.svg");
const FILE: &[u8] = include_bytes!("../assets/icons/file.svg");
const FILE_PLUS: &[u8] = include_bytes!("../assets/icons/file-plus.svg");
const FILE_PEN: &[u8] = include_bytes!("../assets/icons/file-pen.svg");
const FOLDER: &[u8] = include_bytes!("../assets/icons/folder.svg");
const FOLDER_OPEN: &[u8] = include_bytes!("../assets/icons/folder-open.svg");
const FOLDER_PLUS: &[u8] = include_bytes!("../assets/icons/folder-plus.svg");
const FOLDER_TREE: &[u8] = include_bytes!("../assets/icons/folder-tree.svg");
const HAT_GLASSES: &[u8] = include_bytes!("../assets/icons/hat-glasses.svg");
const INFO: &[u8] = include_bytes!("../assets/icons/info.svg");
const LIST_MUSIC: &[u8] = include_bytes!("../assets/icons/list-music.svg");
const PENCIL: &[u8] = include_bytes!("../assets/icons/pencil.svg");
const SCROLL_TEXT: &[u8] = include_bytes!("../assets/icons/scroll-text.svg");
const SKIP_BACK: &[u8] = include_bytes!("../assets/icons/skip-back.svg");
const SLIDERS_VERTICAL: &[u8] = include_bytes!("../assets/icons/sliders-vertical.svg");
const SUN_DIM: &[u8] = include_bytes!("../assets/icons/sun-dim.svg");
const SUN: &[u8] = include_bytes!("../assets/icons/sun.svg");
const TRASH_2: &[u8] = include_bytes!("../assets/icons/trash-2.svg");
const ZOOM_IN: &[u8] = include_bytes!("../assets/icons/zoom-in.svg");
const ZOOM_OUT: &[u8] = include_bytes!("../assets/icons/zoom-out.svg");
const X: &[u8] = include_bytes!("../assets/icons/x.svg");

pub(crate) fn arrow_left() -> svg::Handle {
    svg::Handle::from_memory(ARROW_LEFT)
}

pub(crate) fn arrow_right() -> svg::Handle {
    svg::Handle::from_memory(ARROW_RIGHT)
}

pub(crate) fn brush_cleaning() -> svg::Handle {
    svg::Handle::from_memory(BRUSH_CLEANING)
}

pub(crate) fn chevron_right() -> svg::Handle {
    svg::Handle::from_memory(CHEVRON_RIGHT)
}

pub(crate) fn chevron_down() -> svg::Handle {
    svg::Handle::from_memory(CHEVRON_DOWN)
}

pub(crate) fn circle_alert() -> svg::Handle {
    svg::Handle::from_memory(CIRCLE_ALERT)
}

pub(crate) fn circle_x() -> svg::Handle {
    svg::Handle::from_memory(CIRCLE_X)
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

pub(crate) fn plus() -> svg::Handle {
    svg::Handle::from_memory(PLUS)
}

pub(crate) fn pause() -> svg::Handle {
    svg::Handle::from_memory(PAUSE)
}

pub(crate) fn file() -> svg::Handle {
    svg::Handle::from_memory(FILE)
}

pub(crate) fn file_plus() -> svg::Handle {
    svg::Handle::from_memory(FILE_PLUS)
}

pub(crate) fn file_pen() -> svg::Handle {
    svg::Handle::from_memory(FILE_PEN)
}

pub(crate) fn folder() -> svg::Handle {
    svg::Handle::from_memory(FOLDER)
}

pub(crate) fn folder_open() -> svg::Handle {
    svg::Handle::from_memory(FOLDER_OPEN)
}

pub(crate) fn folder_plus() -> svg::Handle {
    svg::Handle::from_memory(FOLDER_PLUS)
}

pub(crate) fn folder_tree() -> svg::Handle {
    svg::Handle::from_memory(FOLDER_TREE)
}

pub(crate) fn hat_glasses() -> svg::Handle {
    svg::Handle::from_memory(HAT_GLASSES)
}

pub(crate) fn info() -> svg::Handle {
    svg::Handle::from_memory(INFO)
}

pub(crate) fn list_music() -> svg::Handle {
    svg::Handle::from_memory(LIST_MUSIC)
}

pub(crate) fn pencil() -> svg::Handle {
    svg::Handle::from_memory(PENCIL)
}

pub(crate) fn scroll_text() -> svg::Handle {
    svg::Handle::from_memory(SCROLL_TEXT)
}

pub(crate) fn sliders_vertical() -> svg::Handle {
    svg::Handle::from_memory(SLIDERS_VERTICAL)
}

pub(crate) fn skip_back() -> svg::Handle {
    svg::Handle::from_memory(SKIP_BACK)
}

pub(crate) fn sun_dim() -> svg::Handle {
    svg::Handle::from_memory(SUN_DIM)
}

pub(crate) fn sun() -> svg::Handle {
    svg::Handle::from_memory(SUN)
}

pub(crate) fn trash_2() -> svg::Handle {
    svg::Handle::from_memory(TRASH_2)
}

pub(crate) fn zoom_in() -> svg::Handle {
    svg::Handle::from_memory(ZOOM_IN)
}

pub(crate) fn zoom_out() -> svg::Handle {
    svg::Handle::from_memory(ZOOM_OUT)
}

pub(crate) fn x() -> svg::Handle {
    svg::Handle::from_memory(X)
}
