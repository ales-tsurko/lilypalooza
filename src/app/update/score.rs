use super::*;

fn score_preview_matches_request(
    preview: &super::messages::ScorePreviewReady,
    request: ScoreZoomPreviewRequest,
) -> bool {
    preview.page_index == request.page_index
        && (preview.zoom - request.zoom).abs() <= 1e-4
        && preview.tier == request.tier
}

fn score_zoom_preview_request(
    page_index: usize,
    zoom: f32,
    tier: ScoreZoomPreviewTier,
) -> ScoreZoomPreviewRequest {
    ScoreZoomPreviewRequest {
        page_index,
        zoom: score_preview_target_zoom(zoom, tier),
        tier,
    }
}

fn existing_path(path: Option<&PathBuf>) -> Option<PathBuf> {
    path.filter(|candidate| candidate.exists()).cloned()
}

enum ScoreViewerRoute {
    ScrollBy(f32),
    ScrollPosition { x: f32, y: f32 },
    Cursor(Option<iced::Point>),
    PointAndClick,
    Page(ViewerMessage),
    Zoom(ViewerMessage),
    Brightness(ViewerMessage),
}

impl From<ViewerMessage> for ScoreViewerRoute {
    fn from(message: ViewerMessage) -> Self {
        match message {
            ViewerMessage::ScrollUp => Self::ScrollBy(-KEYBOARD_SCROLL_STEP),
            ViewerMessage::ScrollDown => Self::ScrollBy(KEYBOARD_SCROLL_STEP),
            ViewerMessage::ScrollPositionChanged { x, y } => Self::ScrollPosition { x, y },
            ViewerMessage::ViewportCursorMoved(position) => Self::Cursor(Some(position)),
            ViewerMessage::ViewportCursorLeft => Self::Cursor(None),
            ViewerMessage::OpenPointAndClick => Self::PointAndClick,
            ViewerMessage::PrevPage | ViewerMessage::NextPage => Self::Page(message),
            ViewerMessage::ZoomIn
            | ViewerMessage::ZoomOut
            | ViewerMessage::SmoothZoom(_)
            | ViewerMessage::ResetZoom => Self::Zoom(message),
            ViewerMessage::DecreasePageBrightness
            | ViewerMessage::IncreasePageBrightness
            | ViewerMessage::ResetPageBrightness => Self::Brightness(message),
        }
    }
}

impl Lilypalooza {
    pub(in crate::app) fn handle_viewer_message(
        &mut self,
        message: ViewerMessage,
    ) -> Task<Message> {
        match ScoreViewerRoute::from(message) {
            ScoreViewerRoute::ScrollBy(y) => self.scroll_score_viewer_by(y),
            ScoreViewerRoute::ScrollPosition { x, y } => {
                self.svg_scroll_x = x.max(0.0);
                self.svg_scroll_y = y.max(0.0);
                Task::none()
            }
            ScoreViewerRoute::Cursor(position) => {
                self.score_viewport_cursor = position;
                Task::none()
            }
            ScoreViewerRoute::PointAndClick => self.open_score_point_and_click_target(),
            ScoreViewerRoute::Page(message) => self.handle_score_page_message(message),
            ScoreViewerRoute::Zoom(message) => self.handle_score_zoom_message(message),
            ScoreViewerRoute::Brightness(message) => self.handle_score_brightness_message(message),
        }
    }

    fn scroll_score_viewer_by(&mut self, y: f32) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Score);
        iced::widget::operation::scroll_by(
            SCORE_SCROLLABLE_ID,
            iced::widget::operation::AbsoluteOffset { x: 0.0, y },
        )
    }

    fn handle_score_page_message(&mut self, message: ViewerMessage) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Score);
        let Some(rendered_score) = self.rendered_score.as_mut() else {
            return Task::none();
        };

        let changed = match message {
            ViewerMessage::PrevPage if rendered_score.current_page > 0 => {
                rendered_score.current_page -= 1;
                true
            }
            ViewerMessage::NextPage
                if rendered_score.current_page + 1 < rendered_score.pages.len() =>
            {
                rendered_score.current_page += 1;
                true
            }
            _ => false,
        };
        if !changed {
            return Task::none();
        }

        self.score_zoom_preview = None;
        self.score_zoom_preview_pending = None;
        self.request_score_zoom_preview(self.svg_zoom)
            .unwrap_or_else(Task::none)
    }

    fn handle_score_zoom_message(&mut self, message: ViewerMessage) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Score);
        if let ViewerMessage::SmoothZoom(delta) = message {
            return self.handle_smooth_score_zoom(delta);
        }
        if let Some(zoom) = score_step_zoom(
            self.svg_zoom,
            self.default_global_state.score_view.zoom,
            message,
        ) {
            self.svg_zoom = zoom;
            self.score_zoom_persist_pending = false;
            self.persist_settings();
        }
        Task::none()
    }

    fn handle_smooth_score_zoom(&mut self, delta: iced::mouse::ScrollDelta) -> Task<Message> {
        let previous_zoom = self.svg_zoom;
        let next_zoom = smooth_zoom(self.svg_zoom, delta, MIN_SVG_ZOOM, MAX_SVG_ZOOM);

        if (next_zoom - previous_zoom).abs() <= f32::EPSILON {
            return Task::none();
        }

        self.svg_zoom = next_zoom;
        self.score_zoom_last_interaction = Some(std::time::Instant::now());
        self.score_zoom_persist_pending = true;

        if let Some(cursor) = self.score_viewport_cursor {
            let scale = next_zoom / previous_zoom.max(f32::EPSILON);
            self.svg_scroll_x = anchored_scroll(self.svg_scroll_x, cursor.x, scale);
            self.svg_scroll_y = anchored_scroll(self.svg_scroll_y, cursor.y, scale);
            let mut tasks = vec![self.restore_score_scroll()];
            if let Some(task) = self.request_score_zoom_preview(next_zoom) {
                tasks.push(task);
            }
            return Task::batch(tasks);
        }

        self.request_score_zoom_preview(next_zoom)
            .unwrap_or_else(Task::none)
    }

    fn handle_score_brightness_message(&mut self, message: ViewerMessage) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Score);
        match message {
            ViewerMessage::DecreasePageBrightness => {
                self.svg_page_brightness = self
                    .svg_page_brightness
                    .saturating_sub(SVG_PAGE_BRIGHTNESS_STEP);
            }
            ViewerMessage::IncreasePageBrightness => {
                self.svg_page_brightness = self
                    .svg_page_brightness
                    .saturating_add(SVG_PAGE_BRIGHTNESS_STEP)
                    .min(MAX_SVG_PAGE_BRIGHTNESS);
            }
            ViewerMessage::ResetPageBrightness => {
                self.svg_page_brightness = self.default_global_state.score_view.page_brightness;
            }
            _ => {}
        }
        self.persist_settings();
        Task::none()
    }

    pub(in crate::app) fn score_point_and_click_target_at_cursor(
        &self,
    ) -> Option<(PathBuf, usize, usize)> {
        let pointer = self.score_viewport_cursor?;
        let rendered_score = self.rendered_score.as_ref()?;
        let page = rendered_score.current_page()?;

        let display_scale = crate::app::score_view::score_base_scale() * self.svg_zoom;
        let display_x = (self.svg_scroll_x + pointer.x - f32::from(crate::ui_style::PADDING_SM))
            / display_scale.max(f32::EPSILON);
        let display_y = (self.svg_scroll_y + pointer.y - f32::from(crate::ui_style::PADDING_SM))
            / display_scale.max(f32::EPSILON);
        let page_x = display_x * page.coord_size.width / page.display_size.width.max(f32::EPSILON);
        let page_y =
            display_y * page.coord_size.height / page.display_size.height.max(f32::EPSILON);

        if page_x.is_sign_negative()
            || page_y.is_sign_negative()
            || page_x > page.coord_size.width
            || page_y > page.coord_size.height
        {
            return None;
        }

        let target = crate::app::score_cursor::point_and_click_target_at(
            &page.note_anchors,
            page_x,
            page_y,
        )?;
        let path = self.resolve_point_and_click_path(target.path.as_deref())?;

        Some((path, target.line, target.column))
    }

    fn open_score_point_and_click_target(&mut self) -> Task<Message> {
        let Some((path, line, column)) = self.score_point_and_click_target_at_cursor() else {
            return Task::none();
        };

        let _pane_is_visible = self.ensure_workspace_pane_visible(WorkspacePaneKind::Editor);
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.open_editor_file_at_location(&path, line, column)
    }

    fn resolve_point_and_click_path(&self, raw_path: Option<&std::path::Path>) -> Option<PathBuf> {
        let score_path = self
            .current_score
            .as_ref()
            .map(|score| score.path.clone())?;
        let Some(path) = raw_path else {
            return Some(score_path);
        };
        if path.is_absolute() {
            return Some(path.to_path_buf());
        }
        self.resolve_relative_point_and_click_path(&score_path, path)
    }

    fn resolve_relative_point_and_click_path(
        &self,
        score_path: &Path,
        path: &Path,
    ) -> Option<PathBuf> {
        let score_relative = score_path.parent().map(|parent| parent.join(path));
        if let Some(candidate) = existing_path(score_relative.as_ref()) {
            return Some(candidate);
        }

        let project_relative = self.project_root.as_ref().map(|root| root.join(path));
        if let Some(candidate) = existing_path(project_relative.as_ref()) {
            return Some(candidate);
        }

        score_relative
            .or(project_relative)
            .or(Some(path.to_path_buf()))
    }

    pub(in crate::app) fn handle_score_preview_ready(
        &mut self,
        result: Result<super::messages::ScorePreviewReady, String>,
    ) -> Task<Message> {
        let Some(pending) = self.score_zoom_preview_pending else {
            return Task::none();
        };

        self.score_zoom_preview_pending = None;

        match result {
            Ok(preview) if score_preview_matches_request(&preview, pending) => {
                return self.accept_score_zoom_preview(preview);
            }
            Ok(_) => {}
            Err(error) => {
                self.logger.push(format!("[score-preview] {error}"));
            }
        }

        Task::none()
    }

    fn accept_score_zoom_preview(
        &mut self,
        preview: super::messages::ScorePreviewReady,
    ) -> Task<Message> {
        let tier = preview.tier;
        self.score_zoom_preview = Some(ScoreZoomPreview {
            page_index: preview.page_index,
            tier,
            handle: preview.handle,
        });
        if tier == ScoreZoomPreviewTier::Fallback {
            return self
                .request_score_zoom_preview(self.svg_zoom)
                .unwrap_or_else(Task::none);
        }
        Task::none()
    }

    pub(in crate::app) fn score_zoom_preview_active(&self) -> bool {
        self.score_zoom_last_interaction
            .is_some_and(|instant| instant.elapsed() < SCORE_ZOOM_PREVIEW_SETTLE_DELAY)
    }

    pub(in crate::app) fn request_score_zoom_preview(
        &mut self,
        zoom: f32,
    ) -> Option<Task<Message>> {
        let rendered_score = self.rendered_score.as_ref()?;
        let page = rendered_score.current_page()?;
        let page_index = rendered_score.current_page;

        if self.score_zoom_preview_pending.is_some() {
            return None;
        }

        let request = self.next_score_zoom_preview_request(page_index, zoom)?;

        let svg_bytes = page.svg_bytes.clone();
        let page_size = page.display_size;
        self.score_zoom_preview_pending = Some(request);

        Some(Task::perform(
            async move { render_score_zoom_preview(svg_bytes, page_size, request) },
            Message::ScorePreviewReady,
        ))
    }

    fn next_score_zoom_preview_request(
        &self,
        page_index: usize,
        zoom: f32,
    ) -> Option<ScoreZoomPreviewRequest> {
        match self.score_zoom_preview.as_ref() {
            Some(preview)
                if preview.page_index == page_index
                    && preview.tier == ScoreZoomPreviewTier::Primary =>
            {
                None
            }
            Some(preview) if preview.page_index == page_index => Some(score_zoom_preview_request(
                page_index,
                zoom,
                ScoreZoomPreviewTier::Primary,
            )),
            _ => Some(score_zoom_preview_request(
                page_index,
                zoom,
                ScoreZoomPreviewTier::Fallback,
            )),
        }
    }
}

fn score_step_zoom(current: f32, default_zoom: f32, message: ViewerMessage) -> Option<f32> {
    match message {
        ViewerMessage::ZoomIn => Some(next_zoom_step_up(current, SVG_ZOOM_STEP, MAX_SVG_ZOOM)),
        ViewerMessage::ZoomOut => Some(next_zoom_step_down(current, SVG_ZOOM_STEP, MIN_SVG_ZOOM)),
        ViewerMessage::ResetZoom => Some(default_zoom),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_score_route(message: ViewerMessage, expected: fn(ScoreViewerRoute) -> bool) {
        assert!(expected(ScoreViewerRoute::from(message)));
    }

    #[test]
    fn score_viewer_route_classifies_viewer_messages() {
        assert_score_route(ViewerMessage::ScrollUp, |route| {
            matches!(route, ScoreViewerRoute::ScrollBy(_))
        });
        assert_score_route(
            ViewerMessage::ScrollPositionChanged { x: 1.0, y: 2.0 },
            |route| matches!(route, ScoreViewerRoute::ScrollPosition { .. }),
        );
        assert_score_route(
            ViewerMessage::ViewportCursorMoved(iced::Point::ORIGIN),
            |route| matches!(route, ScoreViewerRoute::Cursor(Some(_))),
        );
        assert_score_route(ViewerMessage::ViewportCursorLeft, |route| {
            matches!(route, ScoreViewerRoute::Cursor(None))
        });
        assert_score_route(ViewerMessage::OpenPointAndClick, |route| {
            matches!(route, ScoreViewerRoute::PointAndClick)
        });
        assert_score_route(ViewerMessage::NextPage, |route| {
            matches!(route, ScoreViewerRoute::Page(_))
        });
        assert_score_route(ViewerMessage::ZoomIn, |route| {
            matches!(route, ScoreViewerRoute::Zoom(_))
        });
        assert_score_route(ViewerMessage::ResetPageBrightness, |route| {
            matches!(route, ScoreViewerRoute::Brightness(_))
        });
    }
}
