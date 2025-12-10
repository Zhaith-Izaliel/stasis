pub mod actions;
pub mod app_inhibit;
pub mod lid;
pub mod profile;
pub mod stasis;

pub use actions::*;
pub use app_inhibit::*;
pub use lid::*;
pub use profile::*;
pub use stasis::*;

#[derive(Debug, Clone, PartialEq)]
pub enum LockDetectionType {
    Process,
    Logind,
}
