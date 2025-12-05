use crate::config::model::IdleActionBlock;

/// Manages action execution state
#[derive(Debug)]
pub struct ActionState {
    pub action_index: usize,
    pub instants_triggered: bool,
    pub resume_queue: Vec<IdleActionBlock>,
    pub resume_commands_fired: bool,
    pub pre_suspend_triggered: bool,
}

impl ActionState {
    pub fn reset(&mut self) {
        self.action_index = 0;
        self.instants_triggered = false;
    }

    pub fn advance(&mut self) {
        self.action_index += 1;
        self.instants_triggered = false;
    }
}

impl Default for ActionState {
    fn default() -> Self {
        Self {
            action_index: 0,
            instants_triggered: false,
            resume_queue: Vec::new(),
            resume_commands_fired: false,
            pre_suspend_triggered: false,
        }
    }
}
