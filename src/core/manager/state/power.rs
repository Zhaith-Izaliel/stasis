use crate::{
    config::model::IdleActionBlock,
    core::utils::{ChassisKind, detect_chassis},
    sinfo,
};

#[derive(Debug)]
pub struct PowerState {
    pub chassis: ChassisType,
    pub default_actions: Vec<IdleActionBlock>,
    pub ac_actions: Vec<IdleActionBlock>,
    pub battery_actions: Vec<IdleActionBlock>,
    pub current_block: String,
}

impl PowerState {
    pub fn new_from_config(actions: &[IdleActionBlock]) -> Self {
        let default_actions: Vec<_> = actions
            .iter()
            .filter(|a| !a.name.starts_with("ac.") && !a.name.starts_with("battery."))
            .cloned()
            .collect();

        let ac_actions: Vec<_> = actions
            .iter()
            .filter(|a| a.name.starts_with("ac."))
            .cloned()
            .collect();

        let battery_actions: Vec<_> = actions
            .iter()
            .filter(|a| a.name.starts_with("battery."))
            .cloned()
            .collect();

        let chassis = match detect_chassis() {
            ChassisKind::Laptop => ChassisType::Laptop(LaptopState { on_battery: false }),
            ChassisKind::Desktop => ChassisType::Desktop(DesktopState),
        };

        let current_block = match chassis {
            ChassisType::Desktop(_) => "default".to_string(),
            ChassisType::Laptop(_) => "ac".to_string(), // corrected later when power is read
        };

        Self {
            chassis,
            default_actions,
            ac_actions,
            battery_actions,
            current_block,
        }
    }

    /// Called after a config reload
    pub fn reload_actions(&mut self, actions: &[IdleActionBlock]) {
        self.default_actions = actions
            .iter()
            .filter(|a| !a.name.starts_with("ac.") && !a.name.starts_with("battery."))
            .cloned()
            .collect();

        self.ac_actions = actions
            .iter()
            .filter(|a| a.name.starts_with("ac."))
            .cloned()
            .collect();

        self.battery_actions = actions
            .iter()
            .filter(|a| a.name.starts_with("battery."))
            .cloned()
            .collect();

        // switching logic stays centralized
        self.update_current_block();
    }

    pub fn is_laptop(&self) -> bool {
        matches!(self.chassis, ChassisType::Laptop(_))
    }

    pub fn on_battery(&self) -> Option<bool> {
        match &self.chassis {
            ChassisType::Laptop(l) => Some(l.on_battery),
            ChassisType::Desktop(_) => None,
        }
    }

    /// Return true = block changed (ManagerState should reset its action index)
    pub fn set_on_battery(&mut self, value: bool) -> bool {
        if let ChassisType::Laptop(l) = &mut self.chassis {
            l.on_battery = value;
        }
        self.update_current_block()
    }

    /// Core block switching logic
    pub fn update_current_block(&mut self) -> bool {
        let new_block = match &self.chassis {
            ChassisType::Desktop(_) => "default".to_string(),
            ChassisType::Laptop(state) => {
                if state.on_battery {
                    if !self.battery_actions.is_empty() { "battery" } else { "default" }
                } else {
                    if !self.ac_actions.is_empty() { "ac" } else { "default" }
                }.to_string()
            }
        };

        if new_block != self.current_block {
            let old = std::mem::replace(&mut self.current_block, new_block.clone());
            sinfo!("Stasis", "Switched block: {} → {}", old, new_block);
            return true;
        }

        false
    }

    pub fn active_actions(&self) -> &[IdleActionBlock] {
        match self.current_block.as_str() {
            "ac" => &self.ac_actions,
            "battery" => &self.battery_actions,
            _ => &self.default_actions,
        }
    }

    pub fn active_actions_mut(&mut self) -> &mut Vec<IdleActionBlock> {
        match self.current_block.as_str() {
            "ac" => &mut self.ac_actions,
            "battery" => &mut self.battery_actions,
            _ => &mut self.default_actions,
        }
    }

    pub fn active_instant_actions(&self) -> Vec<IdleActionBlock> {
        self.active_actions()
            .iter()
            .filter(|a| a.is_instant())
            .cloned()
            .collect()
    }
}

//
// Types
//

#[derive(Debug)]
pub enum ChassisType {
    Laptop(LaptopState),
    Desktop(DesktopState),
}

#[derive(Debug)]
pub struct LaptopState {
    pub on_battery: bool,
}

#[derive(Debug)]
pub struct DesktopState;
