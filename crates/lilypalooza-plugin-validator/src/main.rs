//! Isolated helper process for plugin validation.

fn main() {
    std::process::exit(lilypalooza_plugin_validator::run_cli(
        std::env::args().skip(1).collect(),
    ));
}
