use serde::{Deserialize, Serialize};

/// Per-server constant data (configs) that stays the same for the server's
/// life.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConstants {
    /// How many times faster the in-game day/night cycle should be compared to
    /// real time.
    pub day_cycle_coefficient: f64,
    /// Whether players may gather and place blocks in survival build mode
    /// (see [`crate::comp::item::BuildingBlock`]).
    pub survival_building: bool,
}
