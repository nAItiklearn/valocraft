//! Survival building: gathering terrain into building block items and placing
//! them back into the world.
//!
//! Clients send the same `BreakBlock`/`PlaceBlock` messages as in creative
//! build mode. Edits outside of a creative build area are turned into a
//! [`SurvivalBuildEvent`] by `sys::msg::in_game` (when survival building is
//! enabled) and validated here, so the server stays authoritative over reach,
//! protected areas, required tools and the items gained or consumed.

use std::{borrow::Cow, f32::consts::PI};

#[cfg(feature = "persistent_world")]
use crate::TerrainPersistence;
use common::{
    CachedSpatialGrid,
    comp::{
        self, Body, CharacterState, Collider, Health, Inventory, InventoryUpdateBuffer,
        InventoryUpdateEvent, Pos, Scale,
        inventory::slot::EquipSlot,
        item::{
            BuildFailedReason, BuildingBlock, Item, ItemDefinitionId, ItemDesc, ItemKind,
            MaterialStatManifest,
            building::{
                SURVIVAL_BUILD_REACH, SURVIVAL_BUILD_REACH_TOLERANCE, is_disposable_sprite,
            },
        },
        loot_owner::{LootOwnerKind, ONWERSHIP_TIMEOUT_FAST},
        tool::{AbilityMap, ToolKind},
    },
    event::{CreateItemDropEvent, EventBus, SurvivalBuildAction, SurvivalBuildEvent},
    outcome::Outcome,
    resources::ProgramTime,
    terrain::{Block, SpriteKind, TerrainGrid},
    uid::Uid,
    util::{
        Dir,
        find_dist::{Cube, Cylinder, FindDist},
    },
    vol::ReadVol,
};
use common_state::{AreasContainer, BlockChange, BuildArea};
#[cfg(feature = "persistent_world")]
use specs::Write;
use specs::{
    DispatcherBuilder, Read, ReadExpect, ReadStorage, SystemData, WriteExpect, WriteStorage, shred,
};
use tracing::{debug, error};
use vek::*;

use super::{ServerEvent, event_dispatch};

pub(super) fn register_event_systems(builder: &mut DispatcherBuilder) {
    event_dispatch::<SurvivalBuildEvent>(builder, &[]);
}

/// Hand slots that are checked for a tool that is able to break a block.
const TOOL_SLOTS: [EquipSlot; 4] = [
    EquipSlot::ActiveMainhand,
    EquipSlot::ActiveOffhand,
    EquipSlot::InactiveMainhand,
    EquipSlot::InactiveOffhand,
];

#[derive(SystemData)]
pub struct SurvivalBuildData<'a> {
    block_change: WriteExpect<'a, BlockChange>,
    terrain: ReadExpect<'a, TerrainGrid>,
    build_areas: Read<'a, AreasContainer<BuildArea>>,
    #[cfg(feature = "persistent_world")]
    terrain_persistence: Option<Write<'a, TerrainPersistence>>,
    #[cfg(feature = "worldgen")]
    world: ReadExpect<'a, std::sync::Arc<world::World>>,
    #[cfg(feature = "worldgen")]
    index: ReadExpect<'a, world::IndexOwned>,
    spatial_grid: ReadExpect<'a, CachedSpatialGrid>,
    ability_map: ReadExpect<'a, AbilityMap>,
    msm: ReadExpect<'a, MaterialStatManifest>,
    program_time: ReadExpect<'a, ProgramTime>,
    create_item_drop_events: ReadExpect<'a, EventBus<CreateItemDropEvent>>,
    outcomes: ReadExpect<'a, EventBus<Outcome>>,
    inventories: WriteStorage<'a, Inventory>,
    inventory_update_buffers: WriteStorage<'a, InventoryUpdateBuffer>,
    positions: ReadStorage<'a, Pos>,
    bodies: ReadStorage<'a, Body>,
    scales: ReadStorage<'a, Scale>,
    colliders: ReadStorage<'a, Collider>,
    character_states: ReadStorage<'a, CharacterState>,
    healths: ReadStorage<'a, Health>,
    uids: ReadStorage<'a, Uid>,
}

impl SurvivalBuildData<'_> {
    /// Whether survival edits at `pos` are forbidden: creative build areas
    /// belong to whoever was permitted to build there, and generated sites
    /// keep their structures.
    fn is_protected(&self, pos: Vec3<i32>) -> bool { self.in_build_area(pos) || self.in_site(pos) }

    fn in_build_area(&self, pos: Vec3<i32>) -> bool {
        self.build_areas
            .area_metas()
            .iter()
            // The reserved "world" area covers the whole map, so that admins
            // can be permitted to build anywhere. It doesn't claim any land.
            .filter(|(name, _)| name.as_str() != WORLD_BUILD_AREA)
            .filter_map(|(_, id)| self.build_areas.areas().get(*id))
            // Matches the inclusive check of creative building in
            // `sys::msg::in_game`.
            .any(|aabb| aabb.contains_point(pos))
    }

    /// Whether `pos` belongs to a generated site (settlement, dungeon, ...).
    ///
    /// Every tile the site built on is protected, plus a margin of one tile so
    /// that walls and building edges that slightly overhang their tile are
    /// covered as well. Positions outside of the world are always protected.
    #[cfg(feature = "worldgen")]
    fn in_site(&self, pos: Vec3<i32>) -> bool {
        let wpos = pos.xy();
        let Some(chunk) = self.world.sim().get_wpos(wpos) else {
            return true;
        };
        chunk.sites.iter().any(|site_id| {
            let site = &self.index.sites[*site_id];
            let tpos = site.wpos_tile_pos(wpos);
            (-1..=1).any(|x| {
                (-1..=1).any(|y| is_built_tile(&site.tiles.get(tpos + Vec2::new(x, y)).kind))
            })
        })
    }

    #[cfg(not(feature = "worldgen"))]
    fn in_site(&self, _pos: Vec3<i32>) -> bool { false }

    /// Whether the editor's eyes are close enough to the centre of `pos`.
    fn in_reach(&self, entity: specs::Entity, pos: Vec3<i32>) -> bool {
        let Some(editor_pos) = self.positions.get(entity) else {
            return false;
        };
        let scale = self.scales.get(entity).map_or(1.0, |s| s.0);
        let eye_height = self.bodies.get(entity).map_or(0.0, |b| b.eye_height(scale));
        let eye = editor_pos.0 + Vec3::unit_z() * eye_height;
        let max = SURVIVAL_BUILD_REACH + SURVIVAL_BUILD_REACH_TOLERANCE;
        eye.distance_squared(pos.as_::<f32>() + 0.5) <= max * max
    }

    /// Whether any living creature (including the editor) overlaps the voxel
    /// at `pos`, so that placing a block there would trap it.
    fn is_occupied(&self, pos: Vec3<i32>) -> bool {
        // Slightly shrink the voxel so that standing right next to it, or on
        // top of it, does not count as overlapping.
        const INSET: f32 = 0.1;
        let cube = Cube {
            min: pos.as_::<f32>() + INSET,
            side_length: 1.0 - 2.0 * INSET,
        };
        let centre = pos.xy().as_::<f32>() + 0.5;
        // Generous radius: the grid indexes entities by position, and large
        // creatures can overlap the voxel from further away.
        self.spatial_grid
            .0
            .in_circle_aabr(centre, 4.0)
            .filter(|e| self.healths.get(*e).is_some_and(|h| !h.is_dead))
            .filter_map(|e| Some((e, self.positions.get(e)?)))
            .any(|(e, p)| {
                let cylinder = Cylinder::from_components(
                    p.0,
                    self.scales.get(e).copied(),
                    self.colliders.get(e),
                    self.character_states.get(e),
                );
                cube.min_distance(cylinder) <= 0.0
            })
    }

    /// Whether the editor has a tool of `kind` in either hand of either
    /// weapon set.
    fn has_tool(&self, entity: specs::Entity, kind: ToolKind) -> bool {
        self.inventories.get(entity).is_some_and(|inv| {
            TOOL_SLOTS
                .iter()
                .filter_map(|slot| inv.equipped(*slot))
                .any(|item| item.tool_info() == Some(kind))
        })
    }

    fn set_block(&mut self, pos: Vec3<i32>, block: Block) -> bool {
        let was_set = self.block_change.try_set(pos, block).is_some();
        #[cfg(feature = "persistent_world")]
        if was_set && let Some(terrain_persistence) = self.terrain_persistence.as_mut() {
            terrain_persistence.set_block(pos, block);
        }
        was_set
    }

    fn fail(&mut self, entity: specs::Entity, pos: Vec3<i32>, reason: BuildFailedReason) {
        debug!(?entity, ?pos, ?reason, "Survival block edit rejected");
        if let Some(buf) = self.inventory_update_buffers.get_mut(entity) {
            buf.push(InventoryUpdateEvent::BuildFailed { pos, reason });
        }
    }

    fn break_block(&mut self, entity: specs::Entity, pos: Vec3<i32>) {
        let Some(block) = self.terrain.get(pos).ok().copied() else {
            return;
        };
        let Some(gained) = BuildingBlock::from_terrain(block) else {
            // Air, fluids, sprites and props: nothing to gather. Air is not
            // worth a message, the client simply raced a block update.
            if block.is_filled() {
                self.fail(entity, pos, BuildFailedReason::Unbreakable);
            }
            return;
        };
        let required_tool = BuildingBlock::required_tool(block);
        if let Some(tool) = required_tool
            && !self.has_tool(entity, tool)
        {
            return self.fail(entity, pos, BuildFailedReason::RequiresTool(tool));
        }

        // A sprite resting on top of this block loses its support. Decoration
        // (grass, ...) is removed with it, anything else has to be dealt with
        // first. The removal is written here rather than left to the generic
        // unsupported sprite cleanup so that it is persisted as well.
        let above_pos = pos + Vec3::unit_z();
        let supported_sprite = self
            .terrain
            .get(above_pos)
            .ok()
            .copied()
            .filter(|b| b.is_fluid() && b.get_sprite().is_some_and(|s| s != SpriteKind::Empty));
        if let Some(above) = supported_sprite
            && !is_disposable_sprite(above)
        {
            return self.fail(entity, pos, BuildFailedReason::Obstructed);
        }

        let item = match Item::new_from_item_definition_id(
            ItemDefinitionId::Simple(Cow::Borrowed(gained.item_definition_id())),
            &self.ability_map,
            &self.msm,
        ) {
            Ok(item) => item,
            Err(err) => {
                error!(?err, ?gained, "Missing item definition for building block");
                return;
            },
        };

        if !self.set_block(pos, block.into_vacant()) {
            // Another edit already touched this voxel during this tick.
            return;
        }
        if let Some(above) = supported_sprite {
            self.set_block(above_pos, above.into_vacant());
        }

        let frontend_item = item.frontend_item(&self.ability_map, &self.msm);
        let leftover = match self.inventories.get_mut(entity) {
            Some(mut inv) => inv.push(item).err().map(|(item, _)| item),
            None => Some(item),
        };
        match leftover {
            None => {
                if let Some(buf) = self.inventory_update_buffers.get_mut(entity) {
                    buf.push(InventoryUpdateEvent::Collected(frontend_item));
                }
            },
            Some(item) => {
                // No room in the inventory: pop the block out into the world
                // instead of destroying it, owned by the editor for a moment.
                use rand::RngExt;
                let mut rng = rand::rng();
                let loot_owner = self.uids.get(entity).map(|uid| {
                    comp::LootOwner::new(LootOwnerKind::Player(*uid), false, ONWERSHIP_TIMEOUT_FAST)
                });
                self.create_item_drop_events.emit_now(CreateItemDropEvent {
                    pos: Pos(pos.as_::<f32>() + 0.5),
                    vel: comp::Vel(
                        (Vec2::unit_x().rotated_z(rng.random::<f32>() * PI * 2.0) * 2.0)
                            .with_z(rng.random_range(3.0..5.0)),
                    ),
                    ori: comp::Ori::from(Dir::random_2d(&mut rng)),
                    item: comp::PickupItem::new(item, *self.program_time, false),
                    loot_owner,
                });
            },
        }

        self.outcomes.emit_now(Outcome::BreakBlock {
            pos,
            tool: required_tool,
            color: block.get_color(),
        });
    }

    fn place_block(&mut self, entity: specs::Entity, pos: Vec3<i32>, new_block: Block) {
        let Some(building_block) = BuildingBlock::from_canonical(new_block) else {
            // Only the canonical voxels of building blocks can be placed in
            // survival mode; anything else is a confused or modified client.
            return self.fail(entity, pos, BuildFailedReason::MissingBlock);
        };
        let Some(old_block) = self.terrain.get(pos).ok().copied() else {
            return;
        };
        if !old_block.is_fluid() || !is_disposable_sprite(old_block) || self.is_occupied(pos) {
            return self.fail(entity, pos, BuildFailedReason::Obstructed);
        }

        let Some(inventory) = self.inventories.get(entity) else {
            return;
        };
        let Some(slot) = inventory
            .slots_with_id()
            .find(|(_, item)| {
                item.as_ref().is_some_and(
                    |item| matches!(&*item.kind(), ItemKind::Block(b) if *b == building_block),
                )
            })
            .map(|(slot, _)| slot)
        else {
            return self.fail(entity, pos, BuildFailedReason::MissingBlock);
        };

        if !self.set_block(pos, new_block) {
            return;
        }
        let consumed = self
            .inventories
            .get_mut(entity)
            .and_then(|mut inv| inv.take(slot, &self.ability_map, &self.msm));
        if consumed.is_none() {
            error!(
                ?entity,
                ?slot,
                "Failed to consume building block after placing it"
            );
        }
        if let Some(buf) = self.inventory_update_buffers.get_mut(entity) {
            buf.push(InventoryUpdateEvent::Used);
        }
        self.outcomes.emit_now(Outcome::PlaceBlock {
            pos,
            color: new_block.get_color(),
        });
    }
}

/// Name of the build area that `Server::new` creates over the whole world.
pub(crate) const WORLD_BUILD_AREA: &str = "world";

/// Whether a site tile holds something the site built. Sites also mark the
/// natural terrain around them, up to a few hundred blocks away, as hazards
/// (steep slopes, water) and overland paths so that they don't build there;
/// that terrain stays free for survival building.
#[cfg(feature = "worldgen")]
fn is_built_tile(kind: &world::site::TileKind) -> bool {
    use world::site::TileKind;
    !matches!(
        kind,
        TileKind::Empty | TileKind::Hazard(_) | TileKind::Path { .. }
    )
}

impl ServerEvent for SurvivalBuildEvent {
    type SystemData<'a> = SurvivalBuildData<'a>;

    fn handle(events: impl ExactSizeIterator<Item = Self>, mut data: Self::SystemData<'_>) {
        for SurvivalBuildEvent {
            entity,
            pos,
            action,
        } in events
        {
            if data.healths.get(entity).is_none_or(|h| h.is_dead) {
                continue;
            }
            if !data.in_reach(entity, pos) {
                data.fail(entity, pos, BuildFailedReason::OutOfReach);
                continue;
            }
            if data.is_protected(pos) {
                data.fail(entity, pos, BuildFailedReason::Protected);
                continue;
            }
            match action {
                SurvivalBuildAction::Break => data.break_block(entity, pos),
                SurvivalBuildAction::Place(block) => data.place_block(entity, pos, block),
            }
        }
    }
}
