//! Survival building blocks.
//!
//! A [`BuildingBlock`] is an inventory item that represents exactly one
//! placeable voxel. Breaking terrain in survival build mode yields one of these
//! items and placing one consumes it. Every variant has a single canonical
//! [`Block`] (kind + colour), which is what gets written into the world, so a
//! placed block always breaks back into the same item.
//!
//! Natural terrain has per-voxel colour noise, so it is *quantised* into the
//! closest material by [`BlockKind`] instead of being matched exactly. This
//! keeps inventories stackable (one "Dirt Block" stack rather than hundreds of
//! slightly different browns).

use crate::{
    comp::tool::ToolKind,
    terrain::{Block, BlockKind, SpriteKind},
};
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};
use vek::Rgb;

/// Maximum distance (in blocks) from a character's eyes to the centre of the
/// block they are editing in survival build mode. The client limits its
/// targeting to this range, the server adds [`SURVIVAL_BUILD_REACH_TOLERANCE`]
/// on top to absorb latency and rounding.
pub const SURVIVAL_BUILD_REACH: f32 = 6.0;
/// Extra reach the server tolerates before rejecting a survival block edit.
pub const SURVIVAL_BUILD_REACH_TOLERANCE: f32 = 1.5;

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, EnumIter,
)]
pub enum BuildingBlock {
    // Gathered from the world.
    Dirt,
    Turf,
    Sand,
    Snow,
    Stone,
    Timber,
    Leaves,
    Ice,
    // Crafted.
    Planks,
    StoneBricks,
    ClayBricks,
}

impl BuildingBlock {
    /// Every building block, in declaration order.
    pub fn all() -> impl Iterator<Item = Self> { Self::iter() }

    /// The voxel written into the world when this block is placed.
    pub const fn block(self) -> Block {
        let (kind, [r, g, b]) = match self {
            BuildingBlock::Dirt => (BlockKind::Earth, [120, 80, 50]),
            BuildingBlock::Turf => (BlockKind::Grass, [76, 136, 52]),
            BuildingBlock::Sand => (BlockKind::Sand, [218, 190, 132]),
            BuildingBlock::Snow => (BlockKind::Snow, [232, 238, 244]),
            BuildingBlock::Stone => (BlockKind::Rock, [124, 124, 120]),
            BuildingBlock::Timber => (BlockKind::Wood, [102, 70, 44]),
            BuildingBlock::Leaves => (BlockKind::Leaves, [58, 112, 42]),
            BuildingBlock::Ice => (BlockKind::Ice, [160, 204, 236]),
            BuildingBlock::Planks => (BlockKind::Wood, [176, 128, 80]),
            BuildingBlock::StoneBricks => (BlockKind::Rock, [156, 150, 140]),
            BuildingBlock::ClayBricks => (BlockKind::Rock, [164, 78, 58]),
        };
        Block::new(kind, Rgb::new(r, g, b))
    }

    /// Asset specifier of the item definition for this block.
    pub const fn item_definition_id(self) -> &'static str {
        match self {
            BuildingBlock::Dirt => "common.items.blocks.dirt",
            BuildingBlock::Turf => "common.items.blocks.turf",
            BuildingBlock::Sand => "common.items.blocks.sand",
            BuildingBlock::Snow => "common.items.blocks.snow",
            BuildingBlock::Stone => "common.items.blocks.stone",
            BuildingBlock::Timber => "common.items.blocks.timber",
            BuildingBlock::Leaves => "common.items.blocks.leaves",
            BuildingBlock::Ice => "common.items.blocks.ice",
            BuildingBlock::Planks => "common.items.blocks.planks",
            BuildingBlock::StoneBricks => "common.items.blocks.stone_bricks",
            BuildingBlock::ClayBricks => "common.items.blocks.clay_bricks",
        }
    }

    /// Find the building block whose canonical voxel is exactly `block`.
    ///
    /// Used when placing: the client sends the voxel it wants to place and the
    /// server resolves it back to the item that has to be consumed.
    pub fn from_canonical(block: Block) -> Option<Self> {
        Self::iter().find(|b| b.block() == block)
    }

    /// The item gained by breaking `block` in survival build mode, if any.
    ///
    /// Blocks previously placed by a player (exact canonical match) return the
    /// same item. Natural terrain is quantised by kind. Fluids, sprites,
    /// unknown `Misc` voxels (props and structure details) and glowing
    /// mushrooms yield nothing and therefore cannot be broken this way.
    pub fn from_terrain(block: Block) -> Option<Self> {
        if let Some(exact) = Self::from_canonical(block) {
            return Some(exact);
        }
        Some(match block.kind() {
            BlockKind::Earth => BuildingBlock::Dirt,
            BlockKind::Grass => BuildingBlock::Turf,
            BlockKind::Sand => BuildingBlock::Sand,
            BlockKind::Snow | BlockKind::ArtSnow => BuildingBlock::Snow,
            BlockKind::Rock
            | BlockKind::WeakRock
            | BlockKind::GlowingRock
            | BlockKind::GlowingWeakRock => BuildingBlock::Stone,
            BlockKind::Wood => BuildingBlock::Timber,
            BlockKind::Leaves | BlockKind::ArtLeaves => BuildingBlock::Leaves,
            BlockKind::Ice => BuildingBlock::Ice,
            BlockKind::Air
            | BlockKind::Water
            | BlockKind::Lava
            | BlockKind::GlowingMushroom
            | BlockKind::Misc => return None,
        })
    }

    /// The tool that must be equipped to break `block` in survival build
    /// mode. Soft materials can be dug by hand; rock and ice need a pickaxe.
    pub fn required_tool(block: Block) -> Option<ToolKind> {
        match block.kind() {
            BlockKind::Rock
            | BlockKind::WeakRock
            | BlockKind::GlowingRock
            | BlockKind::GlowingWeakRock
            | BlockKind::Ice => Some(ToolKind::Pick),
            _ => None,
        }
    }
}

/// Whether a survival block edit may destroy the sprite in `block` (if any) as
/// a side effect, either by placing into its voxel or by removing the block it
/// rests on. Only plain decoration such as grass may go: anything with
/// collision, contents, an interaction or a yield (collectibles, ores,
/// containers, doors, seats and beds, crafting stations, ...) has to be dealt
/// with first.
pub fn is_disposable_sprite(block: Block) -> bool {
    match block.get_sprite() {
        None | Some(SpriteKind::Empty) => true,
        Some(sprite) => {
            sprite.solid_height().is_none()
                && block.explode_power().is_some()
                && !block.is_collectible()
                && sprite.mine_tool().is_none()
                && !sprite.is_defined_as_container()
                && !sprite.is_door()
                && !sprite.is_mountable()
                && !sprite.is_controller()
        },
    }
}

/// Why the server refused a survival block edit. Sent back to the editing
/// client so the HUD can explain what happened.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildFailedReason {
    /// The block is part of a settlement, dungeon or other generated site.
    Protected,
    /// The block is further away than [`SURVIVAL_BUILD_REACH`].
    OutOfReach,
    /// Breaking this block needs a tool that is not equipped.
    RequiresTool(ToolKind),
    /// This block cannot be gathered in survival mode.
    Unbreakable,
    /// The target position is already occupied by a solid block, a creature
    /// or an object that must not be overwritten.
    Obstructed,
    /// The inventory holds no matching building block.
    MissingBlock,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_decoration_is_disposable() {
        for disposable in [
            Block::empty(),
            Block::air(SpriteKind::Empty),
            Block::water(SpriteKind::Empty),
            Block::air(SpriteKind::LongGrass),
            Block::air(SpriteKind::ShortGrass),
            // Not collectible (no loot).
            Block::air(SpriteKind::BlueFlower),
            BuildingBlock::Dirt.block(),
        ] {
            assert!(is_disposable_sprite(disposable), "{disposable:?}");
        }
        for kept in [
            SpriteKind::RedFlower,
            SpriteKind::Iron,
            SpriteKind::Velorite,
            SpriteKind::Chest,
            SpriteKind::Door,
            SpriteKind::CraftingBench,
            SpriteKind::Bedroll,
        ] {
            assert!(!is_disposable_sprite(Block::air(kept)), "{kept:?}");
        }
    }

    #[test]
    fn canonical_blocks_are_unique_and_solid() {
        let blocks: Vec<_> = BuildingBlock::iter().map(BuildingBlock::block).collect();
        for (i, a) in blocks.iter().enumerate() {
            assert!(a.is_filled(), "{a:?} must be a filled block");
            for b in &blocks[i + 1..] {
                assert_ne!(a, b, "two building blocks share the same voxel");
            }
        }
    }

    #[test]
    fn placed_blocks_break_into_the_same_item() {
        for b in BuildingBlock::iter() {
            assert_eq!(BuildingBlock::from_canonical(b.block()), Some(b));
            assert_eq!(BuildingBlock::from_terrain(b.block()), Some(b));
        }
    }

    #[test]
    fn natural_terrain_is_quantised_by_kind() {
        let noisy = |kind| Block::new(kind, Rgb::new(1, 2, 3));
        assert_eq!(
            BuildingBlock::from_terrain(noisy(BlockKind::Earth)),
            Some(BuildingBlock::Dirt)
        );
        assert_eq!(
            BuildingBlock::from_terrain(noisy(BlockKind::WeakRock)),
            Some(BuildingBlock::Stone)
        );
        assert_eq!(
            BuildingBlock::from_terrain(noisy(BlockKind::Wood)),
            Some(BuildingBlock::Timber)
        );
        assert_eq!(
            BuildingBlock::from_terrain(noisy(BlockKind::ArtSnow)),
            Some(BuildingBlock::Snow)
        );
    }

    #[test]
    fn props_fluids_and_sprites_cannot_be_gathered() {
        assert_eq!(BuildingBlock::from_terrain(Block::empty()), None);
        assert_eq!(
            BuildingBlock::from_terrain(Block::water(SpriteKind::Empty)),
            None
        );
        assert_eq!(
            BuildingBlock::from_terrain(Block::air(SpriteKind::LongGrass)),
            None
        );
        assert_eq!(
            BuildingBlock::from_terrain(Block::new(BlockKind::Misc, Rgb::new(9, 9, 9))),
            None
        );
        assert_eq!(
            BuildingBlock::from_terrain(Block::new(BlockKind::Lava, Rgb::new(255, 80, 0))),
            None
        );
    }

    #[test]
    fn hard_materials_need_a_pick() {
        assert_eq!(
            BuildingBlock::required_tool(BuildingBlock::Stone.block()),
            Some(ToolKind::Pick)
        );
        assert_eq!(
            BuildingBlock::required_tool(BuildingBlock::Ice.block()),
            Some(ToolKind::Pick)
        );
        assert_eq!(
            BuildingBlock::required_tool(BuildingBlock::Dirt.block()),
            None
        );
        assert_eq!(
            BuildingBlock::required_tool(BuildingBlock::Planks.block()),
            None
        );
    }

    #[test]
    fn every_block_has_a_matching_item_asset() {
        use crate::comp::item::{Item, ItemKind};
        for b in BuildingBlock::iter() {
            let item = Item::new_from_asset_expect(b.item_definition_id());
            assert!(
                matches!(&*item.kind(), ItemKind::Block(kind) if *kind == b),
                "{} must have `kind: Block({b:?})`",
                b.item_definition_id()
            );
            assert!(item.is_stackable(), "{b:?} items must stack");
        }
    }
}
