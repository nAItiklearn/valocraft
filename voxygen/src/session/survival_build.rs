//! Client side of survival building: the build mode toggle, the block that is
//! placed and the feedback for edits the server rejected.
//!
//! The server stays authoritative; everything here only decides which
//! `BreakBlock`/`PlaceBlock` requests are sent and what the HUD shows.

use std::time::{Duration, Instant};

use common::comp::{
    Inventory,
    item::{BuildFailedReason, BuildingBlock, Item, ItemKind},
};

use crate::hud::SurvivalBuildInfo;

/// How long a rejected edit stays visible in the build HUD.
const FAILURE_DISPLAY_TIME: Duration = Duration::from_millis(2500);

#[derive(Default)]
pub struct SurvivalBuild {
    /// Whether the player switched build mode on. It is only *active* while
    /// the server allows survival building and the player controls a
    /// character, see `SessionState::tick`.
    pub enabled: bool,
    selected: Option<BuildingBlock>,
    /// An item of the selected block, so the HUD can show its name and icon
    /// even when none are left in the inventory.
    selected_item: Option<Item>,
    failure: Option<(BuildFailedReason, Instant)>,
}

impl SurvivalBuild {
    pub fn selected(&self) -> Option<BuildingBlock> { self.selected }

    pub fn select(&mut self, block: BuildingBlock) {
        if self.selected != Some(block) {
            self.selected = Some(block);
            self.selected_item = Item::new_from_asset(block.item_definition_id()).ok();
        }
    }

    /// Select a block that is actually in the inventory, unless the current
    /// selection still has some left.
    pub fn ensure_selection(&mut self, inventory: Option<&Inventory>) {
        if self
            .selected
            .is_none_or(|block| count(inventory, block) == 0)
            && let Some(block) = first_available(inventory)
        {
            self.select(block);
        }
    }

    pub fn fail(&mut self, reason: BuildFailedReason) {
        self.failure = Some((reason, Instant::now()));
    }

    fn recent_failure(&self) -> Option<BuildFailedReason> {
        self.failure
            .filter(|(_, time)| time.elapsed() < FAILURE_DISPLAY_TIME)
            .map(|(reason, _)| reason)
    }

    pub fn hud_info(&self, inventory: Option<&Inventory>) -> SurvivalBuildInfo<'_> {
        SurvivalBuildInfo {
            block: self.selected,
            item: self.selected_item.as_ref(),
            count: self.selected.map_or(0, |block| count(inventory, block)),
            failure: self.recent_failure(),
        }
    }
}

fn block_of(item: &Item) -> Option<BuildingBlock> {
    match &*item.kind() {
        ItemKind::Block(block) => Some(*block),
        _ => None,
    }
}

/// How many of `block` the inventory holds, across all stacks.
pub fn count(inventory: Option<&Inventory>, block: BuildingBlock) -> u32 {
    inventory.map_or(0, |inventory| {
        inventory
            .slots()
            .flatten()
            .filter(|item| block_of(item) == Some(block))
            .map(|item| item.amount())
            .sum()
    })
}

/// The first building block found in the inventory, in slot order.
fn first_available(inventory: Option<&Inventory>) -> Option<BuildingBlock> {
    inventory?.slots().flatten().find_map(block_of)
}

/// The building block in an inventory slot, if any.
pub fn block_in_slot(
    inventory: Option<&Inventory>,
    slot: common::comp::inventory::slot::InvSlotId,
) -> Option<BuildingBlock> {
    inventory?.get(slot).and_then(block_of)
}
