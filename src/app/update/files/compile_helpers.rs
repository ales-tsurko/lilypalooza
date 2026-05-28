use super::*;

pub(super) fn lilypond_compile_args(output_prefix: &Path) -> Vec<String> {
    vec![
        "--svg".to_string(),
        "-dmidi-extension=midi".to_string(),
        "-dinclude-settings=event-listener.ly".to_string(),
        "-dpoint-and-click=note-event".to_string(),
        "-o".to_string(),
        output_prefix.to_string_lossy().to_string(),
    ]
}

pub(super) fn collect_affected_editor_tabs(
    affected_tabs: &mut Vec<u64>,
    watched_tabs: &[(u64, PathBuf)],
    event: &notify::Event,
) {
    for (tab_id, path) in watched_tabs {
        if is_relevant_editor_file_change(event, path) && !affected_tabs.contains(tab_id) {
            affected_tabs.push(*tab_id);
        }
    }
}

pub(super) fn drain_compile_logs(session: &lilypond::CompileSession) -> CompileLogDrain {
    let mut drain = CompileLogDrain {
        keep_session: true,
        ..CompileLogDrain::default()
    };

    loop {
        match session.try_recv() {
            Ok(event) => apply_compile_event(&mut drain, event),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                drain.keep_session = false;
                break;
            }
        }
    }

    drain
}

pub(super) fn apply_compile_event(drain: &mut CompileLogDrain, event: lilypond::CompileEvent) {
    match event {
        lilypond::CompileEvent::Log { stream, line } => {
            drain
                .lines
                .push(format!("[{}] {line}", compile_log_prefix(stream)));
        }
        lilypond::CompileEvent::ProcessError(message) => {
            drain
                .lines
                .push(format!("[lilypond:process-error] {message}"));
        }
        lilypond::CompileEvent::Finished { success, exit_code } => {
            drain
                .lines
                .push(finished_compile_log_line(success, exit_code));
            drain.finished_successfully = success;
            drain.keep_session = false;
        }
    }
}

pub(super) fn compile_log_prefix(stream: lilypond::LogStream) -> &'static str {
    match stream {
        lilypond::LogStream::Stdout => "lilypond:stdout",
        lilypond::LogStream::Stderr => "lilypond:stderr",
    }
}

pub(super) fn finished_compile_log_line(success: bool, exit_code: Option<i32>) -> String {
    if success {
        return format!(
            "LilyPond compile finished successfully (exit code {})",
            exit_code.unwrap_or(0)
        );
    }

    format!("LilyPond compile failed (exit code {exit_code:?})")
}

pub(super) fn rendered_page_from_loaded(page: super::LoadedRenderedPage) -> RenderedPage {
    RenderedPage {
        handle: svg::Handle::from_memory(page.svg_bytes.clone()),
        svg_bytes: Bytes::from(page.svg_bytes),
        display_size: page.display_size,
        coord_size: page.coord_size,
        note_anchors: page.note_anchors,
        system_bands: page.system_bands,
    }
}

pub(super) fn load_compile_outputs(
    build_dir: PathBuf,
    score_path: PathBuf,
    selected_file_name: String,
) -> Result<super::LoadedCompileOutputs, String> {
    let score_stem = selected_score_stem(&selected_file_name)?;
    let rendered_pages = collect_loaded_rendered_pages(&build_dir, score_stem)?;
    if rendered_pages.is_empty() {
        return Err("LilyPond finished without SVG output".to_string());
    }

    let midi_files = midi::collect_midi_roll_files(&build_dir, score_stem)?;
    let point_and_click_disabled =
        score_cursor::score_disables_point_and_click(&score_path).unwrap_or_default();
    let score_has_repeats = score_cursor::score_contains_repeats(&score_path).unwrap_or_default();

    let all_anchors: Vec<_> = rendered_pages
        .iter()
        .flat_map(|page| page.note_anchors.iter().cloned())
        .collect();
    let score_cursor_maps =
        score_cursor::build_score_cursor_maps(&build_dir, score_stem, &all_anchors, &midi_files)
            .ok();

    Ok(super::LoadedCompileOutputs {
        score_path,
        rendered_pages,
        midi_files,
        score_cursor_maps,
        point_and_click_disabled,
        score_has_repeats,
    })
}

pub(super) fn collect_loaded_rendered_pages(
    build_dir: &Path,
    score_stem: &str,
) -> Result<Vec<super::LoadedRenderedPage>, String> {
    let mut pages = collect_rendered_page_metadata(build_dir, score_stem)?;
    pages.sort_by(|left, right| {
        left.page_number
            .cmp(&right.page_number)
            .then_with(|| left.path.cmp(&right.path))
    });
    pages.into_iter().map(load_rendered_page).collect()
}

pub(super) struct RenderedPageMetadata {
    page_number: u32,
    path: PathBuf,
    display_size: SvgSize,
    coord_size: SvgSize,
}

pub(super) fn collect_rendered_page_metadata(
    build_dir: &Path,
    score_stem: &str,
) -> Result<Vec<RenderedPageMetadata>, String> {
    let entries = read_build_dir_entries(build_dir)?;
    let mut pages = Vec::new();

    for entry in entries {
        push_rendered_page_metadata(entry, score_stem, &mut pages)?;
    }

    Ok(pages)
}

pub(super) fn read_build_dir_entries(build_dir: &Path) -> Result<fs::ReadDir, String> {
    let entries = fs::read_dir(build_dir).map_err(|error| {
        format!(
            "Failed to read build directory {}: {error}",
            build_dir.display()
        )
    })?;
    Ok(entries)
}

pub(super) fn push_rendered_page_metadata(
    entry: std::io::Result<fs::DirEntry>,
    score_stem: &str,
    pages: &mut Vec<RenderedPageMetadata>,
) -> Result<(), String> {
    let entry = entry.map_err(|error| format!("Failed to read build artifact entry: {error}"))?;
    if let Some(page) = rendered_page_metadata_for_path(entry.path(), score_stem)? {
        pages.push(page);
    }
    Ok(())
}

pub(super) fn rendered_page_metadata_for_path(
    path: PathBuf,
    score_stem: &str,
) -> Result<Option<RenderedPageMetadata>, String> {
    if !is_svg_file(&path) {
        return Ok(None);
    }
    let Some(file_stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return Ok(None);
    };
    let Some(page_number) = svg_page_index(file_stem, score_stem) else {
        return Ok(None);
    };

    let display_size = read_svg_size(&path).unwrap_or(SvgSize {
        width: 1200.0,
        height: 1700.0,
    });
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read SVG {}: {error}", path.display()))?;
    let coord_size = super::parse_svg_coordinate_size_from_source(&source).unwrap_or(display_size);

    Ok(Some(RenderedPageMetadata {
        page_number,
        path,
        display_size,
        coord_size,
    }))
}

pub(super) fn load_rendered_page(
    page: RenderedPageMetadata,
) -> Result<super::LoadedRenderedPage, String> {
    let bytes = fs::read(&page.path)
        .map_err(|error| format!("Failed to read SVG {}: {error}", page.path.display()))?;
    let source = String::from_utf8_lossy(&bytes);
    let page_index = page.page_number.saturating_sub(1) as usize;
    let note_anchors = score_cursor::parse_svg_note_anchors(&source, page_index);
    let system_bands = score_cursor::parse_svg_system_bands(&source);

    Ok(super::LoadedRenderedPage {
        svg_bytes: bytes,
        display_size: page.display_size,
        coord_size: page.coord_size,
        note_anchors,
        system_bands,
    })
}
