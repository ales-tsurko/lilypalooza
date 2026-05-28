use super::*;
use crate::app::editor::EditorTabFileState;

async fn save_plugin_scan_cache(
    cache: lilypalooza_plugin_scan::PluginScanCache,
    path: PathBuf,
) -> Result<(), String> {
    cache.save_to(&path)
}

mod compile_helpers;
mod compile_outputs;
mod projects;
mod watcher_helpers;
mod watchers;

use compile_helpers::*;
use watcher_helpers::*;
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::lilypond_compile_args;

    #[test]
    fn lilypond_compile_args_keep_interactive_svg_output() {
        let args = lilypond_compile_args(Path::new("/tmp/out"));

        assert_eq!(args[0], "--svg");
        assert!(!args.iter().any(|arg| arg == "-dbackend=cairo"));
        assert!(args.iter().any(|arg| arg == "-dmidi-extension=midi"));
        assert!(args.iter().any(|arg| arg == "-dpoint-and-click=note-event"));
        assert_eq!(args[args.len() - 2], "-o");
        assert_eq!(args[args.len() - 1], "/tmp/out");
    }
}
