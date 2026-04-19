# Lilypalooza

**Lilypalooza** is a desktop LilyPond IDE.

It is built for editing, previewing, and auditioning LilyPond projects in one
place.

![Screenshot 1](assets/screenshots/screenshot_1.png)
![Screenshot 2](assets/screenshots/screenshot_2.png)

## Major Features

- multi-tab editor for LilyPond projects
- tree-sitter-based syntax highlighting
- rendered score preview with point-and-click back to source
- MIDI playback with transport, mixer, and SoundFont-based instruments
- piano roll for MIDI inspection
- dockable multi-pane workspace
- project persistence and workspace restore
- integrated compile status and logs

## Run

```bash
cargo run --release
```

## Workflow

Typical workflow:

1. Open or create a LilyPond file.
2. Edit it in the built-in editor.
3. Save to trigger recompilation.
4. Inspect the rendered score and piano roll.
5. Click the score to jump back to the exact source location.

If the current file belongs to a saved project, Lilypalooza restores project
state from `.lilypalooza/project.ron`. Otherwise it uses global state.

## Settings

App settings are stored in the platform config directory under:

- `lilypalooza/settings.toml`

That file contains:

- editor view settings
- editor theme tuning
- shortcut overrides
- recent-file limits

Some editor behavior is configurable both from the UI and from the settings
file.

## CLI Arguments

CLI arguments mostly exist for development and quick startup.

Preload a SoundFont:

```bash
cargo run -- --soundfont assets/soundfonts/FluidR3_GM.sf2
```

Or:

```bash
LILYPALOOZA_SOUNDFONT=assets/soundfonts/FluidR3_GM.sf2 cargo run
```

Preload a score file:

```bash
cargo run -- --score path/to/score.ly
```

Or:

```bash
LILYPALOOZA_SCORE=path/to/score.ly cargo run
```

## Tests

```bash
cargo test
```

There are also manual startup error-path checks under:

```text
scripts/lilypond-error-tests
```
