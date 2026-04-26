#![allow(missing_docs)]

use std::env;
use std::path::{Path, PathBuf};

use auxiliary_window::{HostOptions, WindowSnapshot, install_editor_host};
use iced::widget::{container, text};
use iced::{Element, Length, Size, Subscription, Task, window};
use lilypalooza_audio::{
    AudioEngine, AudioEngineOptions, BUILTIN_SOUNDFONT_ID, EditorDescriptor, EditorParent,
    EditorSession, MixerState, SlotAddress, SlotState, SoundfontResource, TrackId,
};
use lilypalooza_builtins::soundfont_synth;

const SOUNDFONT_ID: &str = "debug";

fn main() -> iced::Result {
    let options = match Options::from_args(env::args_os().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            print_usage();
            return Ok(());
        }
    };

    if let Err(error) = auxiliary_window::prepare_process() {
        eprintln!("failed to prepare editor host process: {error}");
        return Ok(());
    }

    iced::application(
        move || SoundfontEditorApp::new(options.clone()),
        update,
        view,
    )
    .subscription(subscription)
    .title(title)
    .window(window_settings())
    .run()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    path: PathBuf,
    bank: u16,
    program: u8,
}

impl Options {
    fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = std::ffi::OsString>,
    {
        let mut path = None;
        let mut bank = 0;
        let mut program = 0;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.to_string_lossy().as_ref() {
                "--soundfont" => path = args.next().map(PathBuf::from),
                "--bank" => bank = parse_next(&mut args, "--bank")?,
                "--program" => program = parse_next(&mut args, "--program")?,
                value if value.starts_with('-') => return Err(format!("unknown option `{value}`")),
                _ => path = Some(PathBuf::from(arg)),
            }
        }

        Ok(Self {
            path: path.ok_or("missing <soundfont.sf2>")?,
            bank,
            program,
        })
    }
}

fn parse_next<T, I>(args: &mut I, option: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    I: Iterator<Item = std::ffi::OsString>,
{
    args.next()
        .ok_or_else(|| format!("missing value for {option}"))?
        .to_string_lossy()
        .parse()
        .map_err(|_| format!("invalid value for {option}"))
}

struct SoundfontRuntime {
    _engine: AudioEngine,
    descriptor: EditorDescriptor,
    session: Option<Box<dyn EditorSession>>,
    title: String,
}

impl SoundfontRuntime {
    fn new(options: &Options) -> Result<Self, String> {
        lilypalooza_builtins::register_all();

        let mut engine = AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
            .map_err(|error| error.to_string())?;
        let resource = SoundfontResource {
            id: SOUNDFONT_ID.to_string(),
            name: soundfont_name(&options.path),
            path: options.path.clone(),
        };

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(resource)
                .map_err(|error| error.to_string())?;
            mixer
                .set_track_instrument(
                    TrackId(0),
                    SlotState::built_in(
                        BUILTIN_SOUNDFONT_ID,
                        soundfont_synth::state(SOUNDFONT_ID, options.bank, options.program),
                    ),
                )
                .map_err(|error| error.to_string())?;
        }

        let controller = engine
            .controller(SlotAddress {
                strip_index: 1,
                slot_index: 0,
            })
            .map_err(|error| error.to_string())?
            .ok_or("SF-01 controller is unavailable")?;
        let descriptor = controller
            .descriptor()
            .editor
            .ok_or("SF-01 builtin has no editor")?;
        let session = controller
            .create_editor_session()
            .map_err(|error| error.to_string())?
            .ok_or("SF-01 editor session is unavailable")?;

        Ok(Self {
            _engine: engine,
            descriptor,
            session: Some(session),
            title: format!("SF-01 - {}", soundfont_name(&options.path)),
        })
    }
}

struct SoundfontEditorApp {
    runtime: Result<SoundfontRuntime, String>,
    host_window_id: Option<window::Id>,
    attached: bool,
    status: String,
}

impl SoundfontEditorApp {
    fn new(options: Options) -> Self {
        Self {
            runtime: SoundfontRuntime::new(&options),
            host_window_id: None,
            attached: false,
            status: "opening editor".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    HostReady {
        window_id: window::Id,
        host: Result<WindowSnapshot, String>,
        parent: Result<WindowSnapshot, String>,
    },
}

fn update(app: &mut SoundfontEditorApp, message: Message) -> Task<Message> {
    match message {
        Message::WindowOpened(window_id) => {
            app.host_window_id = Some(window_id);
            let (title, resizable) = match &app.runtime {
                Ok(runtime) => (runtime.title.clone(), runtime.descriptor.resizable),
                Err(_) => return Task::none(),
            };
            window::run(window_id, move |window| {
                let host = window
                    .window_handle()
                    .map_err(|error| error.to_string())
                    .and_then(|handle| {
                        WindowSnapshot::capture(
                            handle.as_raw(),
                            window.display_handle().ok().map(|display| display.as_raw()),
                        )
                        .map_err(|error| error.to_string())
                    });
                let parent = host.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |host| {
                        install_editor_host(
                            host,
                            &HostOptions::new(title).with_resizable(resizable),
                        )
                        .map(|installed| installed.content)
                        .map_err(|error| error.to_string())
                    },
                );

                Message::HostReady {
                    window_id,
                    host,
                    parent,
                }
            })
        }
        Message::WindowClosed(window_id) => {
            if Some(window_id) == app.host_window_id {
                if let Ok(runtime) = &mut app.runtime
                    && let Some(mut session) = runtime.session.take()
                {
                    let _ = session.detach();
                }
                return iced::exit();
            }
            Task::none()
        }
        Message::HostReady {
            window_id,
            host,
            parent,
        } => {
            if Some(window_id) != app.host_window_id || app.attached {
                return Task::none();
            }
            let Ok(runtime) = &mut app.runtime else {
                return Task::none();
            };
            let Some(session) = runtime.session.as_mut() else {
                return Task::none();
            };
            let parent = match host.and(parent).and_then(snapshot_into_editor_parent) {
                Ok(parent) => parent,
                Err(error) => {
                    app.status = error;
                    return Task::none();
                }
            };
            match session.attach(parent) {
                Ok(()) => {
                    app.attached = true;
                    app.status = "editor attached".to_string();
                }
                Err(error) => {
                    app.status = error.to_string();
                }
            }
            Task::none()
        }
    }
}

fn view(app: &SoundfontEditorApp) -> Element<'_, Message> {
    let status = match &app.runtime {
        Ok(_) => app.status.as_str(),
        Err(error) => error.as_str(),
    };
    container(text(status))
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into()
}

fn subscription(_app: &SoundfontEditorApp) -> Subscription<Message> {
    Subscription::batch([
        window::open_events().map(Message::WindowOpened),
        window::close_events().map(Message::WindowClosed),
    ])
}

fn title(app: &SoundfontEditorApp) -> String {
    app.runtime
        .as_ref()
        .map(|runtime| runtime.title.clone())
        .unwrap_or_else(|_| "SF-01".to_string())
}

fn print_usage() {
    eprintln!(
        "Usage: cargo run -p lilypalooza-builtins --example open_soundfont_synth -- <soundfont.sf2> [--bank N] [--program N]"
    );
}

fn window_settings() -> window::Settings {
    window::Settings {
        size: Size::new(820.0, 456.0),
        min_size: Some(Size::new(820.0, 456.0)),
        resizable: false,
        decorations: false,
        exit_on_close_request: false,
        ..window::Settings::default()
    }
}

fn snapshot_into_editor_parent(snapshot: WindowSnapshot) -> Result<EditorParent, String> {
    let window = snapshot
        .raw_window_handle()
        .map_err(|error| error.to_string())?;
    let display = snapshot
        .raw_display_handle()
        .map_err(|error| error.to_string())?;
    Ok(EditorParent { window, display })
}

fn soundfont_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "SoundFont".to_string())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::Options;

    #[test]
    fn parses_positional_soundfont_with_defaults() {
        let options =
            Options::from_args([OsString::from("piano.sf2")]).expect("options should parse");

        assert_eq!(options.path, PathBuf::from("piano.sf2"));
        assert_eq!(options.bank, 0);
        assert_eq!(options.program, 0);
    }

    #[test]
    fn parses_program_selection() {
        let options = Options::from_args([
            OsString::from("--soundfont"),
            OsString::from("piano.sf2"),
            OsString::from("--bank"),
            OsString::from("128"),
            OsString::from("--program"),
            OsString::from("42"),
        ])
        .expect("options should parse");

        assert_eq!(options.path, PathBuf::from("piano.sf2"));
        assert_eq!(options.bank, 128);
        assert_eq!(options.program, 42);
    }
}
