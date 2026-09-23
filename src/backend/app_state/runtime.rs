use crate::backend::app_state::state::State;

#[derive(Debug, Clone)]
pub struct RuntimeState {
    pub ui_alpha: State<f32>,
}

impl RuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            ui_alpha: State::new(1.0),
        }
    }
}
