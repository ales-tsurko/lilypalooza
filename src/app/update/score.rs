use super::*;

impl LilyView {
    pub(in crate::app) fn handle_viewer_message(
        &mut self,
        message: ViewerMessage,
    ) -> Task<Message> {
        match message {
            ViewerMessage::ScrollUp
            | ViewerMessage::ScrollDown
            | ViewerMessage::PrevPage
            | ViewerMessage::NextPage
            | ViewerMessage::ZoomIn
            | ViewerMessage::ZoomOut
            | ViewerMessage::SmoothZoom(_)
            | ViewerMessage::DecreasePageBrightness
            | ViewerMessage::IncreasePageBrightness
            | ViewerMessage::ResetZoom
            | ViewerMessage::ResetPageBrightness => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Score);
            }
            ViewerMessage::ScrollPositionChanged { .. }
            | ViewerMessage::ViewportCursorMoved(_)
            | ViewerMessage::ViewportCursorLeft => {}
        }

        match message {
            ViewerMessage::ScrollUp => {
                return iced::widget::operation::scroll_by(
                    SCORE_SCROLLABLE_ID,
                    iced::widget::operation::AbsoluteOffset {
                        x: 0.0,
                        y: -KEYBOARD_SCROLL_STEP,
                    },
                );
            }
            ViewerMessage::ScrollDown => {
                return iced::widget::operation::scroll_by(
                    SCORE_SCROLLABLE_ID,
                    iced::widget::operation::AbsoluteOffset {
                        x: 0.0,
                        y: KEYBOARD_SCROLL_STEP,
                    },
                );
            }
            ViewerMessage::ScrollPositionChanged { x, y } => {
                self.svg_scroll_x = x.max(0.0);
                self.svg_scroll_y = y.max(0.0);
            }
            ViewerMessage::ViewportCursorMoved(position) => {
                self.score_viewport_cursor = Some(position);
            }
            ViewerMessage::ViewportCursorLeft => {
                self.score_viewport_cursor = None;
            }
            ViewerMessage::PrevPage => {
                if let Some(rendered_score) = self.rendered_score.as_mut()
                    && rendered_score.current_page > 0
                {
                    rendered_score.current_page -= 1;
                    self.score_zoom_preview = None;
                    self.score_zoom_preview_pending = None;

                    if let Some(task) = self.request_score_zoom_preview(self.svg_zoom) {
                        return task;
                    }
                }
            }
            ViewerMessage::NextPage => {
                if let Some(rendered_score) = self.rendered_score.as_mut()
                    && rendered_score.current_page + 1 < rendered_score.pages.len()
                {
                    rendered_score.current_page += 1;
                    self.score_zoom_preview = None;
                    self.score_zoom_preview_pending = None;

                    if let Some(task) = self.request_score_zoom_preview(self.svg_zoom) {
                        return task;
                    }
                }
            }
            ViewerMessage::ZoomIn => {
                self.svg_zoom = next_zoom_step_up(self.svg_zoom, SVG_ZOOM_STEP, MAX_SVG_ZOOM);
                self.score_zoom_persist_pending = false;
                self.persist_settings();
            }
            ViewerMessage::ZoomOut => {
                self.svg_zoom = next_zoom_step_down(self.svg_zoom, SVG_ZOOM_STEP, MIN_SVG_ZOOM);
                self.score_zoom_persist_pending = false;
                self.persist_settings();
            }
            ViewerMessage::SmoothZoom(delta) => {
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

                if let Some(task) = self.request_score_zoom_preview(next_zoom) {
                    return task;
                }
            }
            ViewerMessage::DecreasePageBrightness => {
                self.svg_page_brightness = self
                    .svg_page_brightness
                    .saturating_sub(SVG_PAGE_BRIGHTNESS_STEP);
                self.persist_settings();
            }
            ViewerMessage::IncreasePageBrightness => {
                self.svg_page_brightness = self
                    .svg_page_brightness
                    .saturating_add(SVG_PAGE_BRIGHTNESS_STEP)
                    .min(MAX_SVG_PAGE_BRIGHTNESS);
                self.persist_settings();
            }
            ViewerMessage::ResetZoom => {
                self.svg_zoom = self.default_global_state.score_view.zoom;
                self.score_zoom_persist_pending = false;
                self.persist_settings();
            }
            ViewerMessage::ResetPageBrightness => {
                self.svg_page_brightness = self.default_global_state.score_view.page_brightness;
                self.persist_settings();
            }
        }

        Task::none()
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
            Ok(preview)
                if preview.page_index == pending.page_index
                    && (preview.zoom - pending.zoom).abs() <= 1e-4
                    && preview.tier == pending.tier =>
            {
                self.score_zoom_preview = Some(ScoreZoomPreview {
                    page_index: preview.page_index,
                    tier: preview.tier,
                    handle: preview.handle,
                });

                if preview.tier == ScoreZoomPreviewTier::Fallback
                    && let Some(task) = self.request_score_zoom_preview(self.svg_zoom)
                {
                    return task;
                }
            }
            Ok(_) => {}
            Err(error) => {
                self.logger.push(format!("[score-preview] {error}"));
            }
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

        let request = match self.score_zoom_preview.as_ref() {
            Some(preview)
                if preview.page_index == page_index
                    && preview.tier == ScoreZoomPreviewTier::Primary =>
            {
                return None;
            }
            Some(preview) if preview.page_index == page_index => ScoreZoomPreviewRequest {
                page_index,
                zoom: score_preview_target_zoom(zoom, ScoreZoomPreviewTier::Primary),
                tier: ScoreZoomPreviewTier::Primary,
            },
            _ => ScoreZoomPreviewRequest {
                page_index,
                zoom: score_preview_target_zoom(zoom, ScoreZoomPreviewTier::Fallback),
                tier: ScoreZoomPreviewTier::Fallback,
            },
        };

        let svg_bytes = page.svg_bytes.clone();
        let page_size = page.size;
        self.score_zoom_preview_pending = Some(request);

        Some(Task::perform(
            async move { render_score_zoom_preview(svg_bytes, page_size, request) },
            Message::ScorePreviewReady,
        ))
    }
}
