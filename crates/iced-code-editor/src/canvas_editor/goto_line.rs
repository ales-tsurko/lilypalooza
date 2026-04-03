use iced::widget::Id;

#[derive(Debug, Clone)]
pub struct GotoLineState {
    pub query: String,
    pub is_open: bool,
    pub input_id: Id,
}

impl Default for GotoLineState {
    fn default() -> Self {
        Self {
            query: String::new(),
            is_open: false,
            input_id: Id::unique(),
        }
    }
}

impl GotoLineState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }
}
