//! End-to-end check of survival building against a running server.
//!
//! This drives a headless client through the whole break/place loop and checks
//! what the server actually does: terrain edits, items gained and consumed,
//! every rejection reason and, in a second run after a server restart, that
//! placed blocks were persisted.
//!
//! It needs a local no-auth server with terrain persistence enabled
//! (`experimental_terrain_persistence: true`) and the check's user as an admin,
//! because it uses `/goto`, `/give_item` and `/kill_npcs` to set up each
//! scenario. `scripts/survival_build_e2e.sh` does all of this, or by hand:
//!
//! ```sh
//! veloren-server-cli admin add buildcheck admin
//! veloren-server-cli --no-auth --non-interactive &
//! cargo run --release -p veloren-client --features bin_bot \
//!     --example survival_build_check -- run <port> /path/to/state
//! # restart the server, then
//! cargo run ... -- verify <port> /path/to/state
//! ```
//!
//! The process exits with a non-zero code if any check fails.

#![deny(unsafe_code)]

use common::{
    clock::Clock,
    comp::{
        self, InventoryUpdateEvent,
        inventory::slot::{EquipSlot, Slot},
        item::{
            BuildFailedReason, BuildingBlock, ItemDefinitionId, ItemDesc, ItemKind,
            building::SURVIVAL_BUILD_REACH, tool::ToolKind,
        },
    },
    map::MarkerKind,
    outcome::Outcome,
    terrain::{Block, SpriteKind},
    vol::ReadVol,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;
use vek::*;
use veloren_client::{
    Client, ClientType, Credentials, Event, StaticCredentialProvider, addr::ConnectionArgs,
};

const USERNAME: &str = "buildcheck";
const TPS: f64 = 30.0;
/// High enough to be above any terrain while waiting for chunks to load.
const HOVER_ALT: f32 = 3000.0;
/// How far around the test spot NPCs are cleared, see `Harness::secure`.
const SECURE_RADIUS: f32 = 64.0;
/// The slots the server checks for a tool.
const HAND_SLOTS: [EquipSlot; 4] = [
    EquipSlot::ActiveMainhand,
    EquipSlot::ActiveOffhand,
    EquipSlot::InactiveMainhand,
    EquipSlot::InactiveOffhand,
];

#[derive(Debug, PartialEq)]
enum EditResult {
    Changed(Block),
    Failed(BuildFailedReason),
    NoResponse,
}

struct Harness {
    client: Client,
    clock: Clock,
    inventory_events: Vec<InventoryUpdateEvent>,
    failures: u32,
}

impl Harness {
    fn connect(port: u16, runtime: Arc<Runtime>) -> Self {
        let provider = StaticCredentialProvider {
            credentials_by_auth: [(None, Credentials::Username(USERNAME.to_owned()))].into(),
        };
        let client = runtime
            .block_on(Client::new(
                ConnectionArgs::Tcp {
                    prefer_ipv6: false,
                    hostname: format!("127.0.0.1:{port}"),
                },
                Arc::clone(&runtime),
                None,
                &provider,
                &|_| {},
                |_| {},
                std::env::temp_dir(),
                ClientType::Game,
            ))
            .expect("Failed to connect to the server");
        Self {
            client,
            clock: Clock::new(Duration::from_secs_f64(1.0 / TPS)),
            inventory_events: Vec::new(),
            failures: 0,
        }
    }

    fn step(&mut self) {
        let events = self
            .client
            .tick(comp::ControllerInputs::default(), self.clock.game_dt())
            .expect("Client tick failed");
        for event in events {
            match event {
                Event::InventoryUpdated(events) => self.inventory_events.extend(events),
                Event::Chat(msg) => println!("    chat: {:?}", msg.content()),
                // Anything that hurts or moves the character can spoil the
                // reach checks, so report it.
                Event::Outcome(Outcome::HealthChange { info, .. })
                    if Some(info.target) == self.client.uid() =>
                {
                    println!(
                        "    health change {:.1} by {:?} ({:?}), {}",
                        info.amount,
                        info.by,
                        info.cause,
                        self.status()
                    )
                },
                Event::Outcome(Outcome::Death { pos }) if pos.distance(self.pos()) < 1.0 => {
                    println!("    died at {pos}")
                },
                Event::CharacterError(err) => panic!("Character error: {err}"),
                Event::Disconnect => panic!("Disconnected from the server"),
                _ => {},
            }
        }
        self.client.cleanup();
        self.clock.tick();
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.step();
        }
    }

    /// Tick until `done` holds, returning whether it did before `timeout`.
    fn wait(&mut self, timeout: Duration, mut done: impl FnMut(&mut Self) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            self.step();
            if done(self) {
                return true;
            }
        }
        false
    }

    fn check(&mut self, name: &str, ok: bool) {
        println!("[{}] {name}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            println!("    {}", self.status());
            self.failures += 1;
        }
    }

    /// Where the character is and how it is doing, for diagnosing failures.
    fn status(&self) -> String {
        format!(
            "at {:.2}, vel {:.2?}, health {:?}, state {:?}",
            self.pos(),
            self.client.current::<comp::Vel>().map(|v| v.0),
            self.client.current::<comp::Health>().map(|h| h.current()),
            self.client
                .current::<comp::CharacterState>()
                .map(|c| c.to_string()),
        )
    }

    fn command(&mut self, name: &str, args: &[String]) {
        self.client.send_command(name.to_owned(), args.to_vec());
    }

    fn block(&self, pos: Vec3<i32>) -> Option<Block> {
        self.client.state().terrain().get(pos).ok().copied()
    }

    fn count(&self, block: BuildingBlock) -> u32 {
        let inventories = self.client.inventories();
        inventories.get(self.client.entity()).map_or(0, |inv| {
            inv.slots()
                .flatten()
                .filter(|item| matches!(&*item.kind(), ItemKind::Block(b) if *b == block))
                .map(|item| item.amount())
                .sum()
        })
    }

    fn equipped_tool(&self, slot: EquipSlot) -> Option<ToolKind> {
        self.client
            .inventories()
            .get(self.client.entity())
            .and_then(|inv| inv.equipped(slot))
            .and_then(|item| item.tool_info())
    }

    fn pos(&self) -> Vec3<f32> {
        self.client
            .current::<comp::Pos>()
            .map(|p| p.0)
            .expect("Player has no position")
    }

    fn eye(&self) -> Vec3<f32> {
        let scale = self.client.current::<comp::Scale>().map_or(1.0, |s| s.0);
        let eye_height = self
            .client
            .current::<comp::Body>()
            .map_or(0.0, |b| b.eye_height(scale));
        self.pos() + Vec3::unit_z() * eye_height
    }

    fn reach_to(&self, pos: Vec3<i32>) -> f32 { self.eye().distance(pos.as_::<f32>() + 0.5) }

    /// The highest filled block of a loaded terrain column.
    fn ground(&self, xy: Vec2<i32>) -> Option<i32> {
        let terrain = self.client.state().terrain();
        (-200..HOVER_ALT as i32 - 100)
            .rev()
            .find(|z| terrain.get(xy.with_z(*z)).is_ok_and(|b| b.is_filled()))
    }

    fn column_loaded(&self, xy: Vec2<i32>) -> bool {
        self.client.state().terrain().get(xy.with_z(0)).is_ok()
    }

    /// Teleport above `xy`, wait for its terrain, then stand on the ground.
    /// Returns the altitude of the ground block.
    fn teleport(&mut self, xy: Vec2<i32>) -> Option<i32> {
        let entity = self.client.entity();
        let hover = xy.as_::<f32>().with_z(HOVER_ALT) + Vec3::new(0.5, 0.5, 0.0);
        self.command("goto", &[
            hover.x.to_string(),
            hover.y.to_string(),
            hover.z.to_string(),
        ]);
        // Wait for the server to move us, then hold the character in place
        // (physics is client side) instead of letting it fall to its death
        // while the chunks load.
        if !self.wait(Duration::from_secs(10), |h| {
            h.pos().xy().distance(hover.xy()) < 2.0
        }) {
            println!("    /goto {xy} was not applied");
            return None;
        }
        let loaded = self.wait(Duration::from_secs(60), |h| {
            let state = h.client.state_mut();
            state.write_component_ignore_entity_dead(entity, comp::Pos(hover));
            state.write_component_ignore_entity_dead(entity, comp::Vel(Vec3::zero()));
            [Vec2::zero(), Vec2::unit_x() * 16, Vec2::unit_y() * 16]
                .iter()
                .all(|offset| h.column_loaded(xy + offset) && h.column_loaded(xy - offset))
        });
        if !loaded {
            println!("    terrain around {xy} did not load");
            return None;
        }
        let ground = self.ground(xy)?;
        self.stand_on(xy, ground).then_some(ground)
    }

    /// Move onto the ground block at `xy`, `ground`, returning whether the
    /// character landed there.
    fn stand_on(&mut self, xy: Vec2<i32>, ground: i32) -> bool {
        let stand = Self::stand_pos(xy, ground);
        self.command("goto", &[
            stand.x.to_string(),
            stand.y.to_string(),
            stand.z.to_string(),
        ]);
        let landed = self.wait(Duration::from_secs(10), |h| {
            h.pos().distance(stand) < 1.5
                && h.client
                    .current::<comp::PhysicsState>()
                    .is_some_and(|p| p.on_ground.is_some())
        });
        self.ticks(10);
        landed
    }

    fn stand_pos(xy: Vec2<i32>, ground: i32) -> Vec3<f32> {
        xy.as_::<f32>().with_z(ground as f32 + 1.0) + Vec3::new(0.5, 0.5, 0.0)
    }

    /// Clear the NPCs around the test spot and get back onto it. Wildlife
    /// attacks would otherwise knock the character out of reach of its
    /// targets, or kill it, and make the checks after that fail at random.
    fn secure(&mut self, xy: Vec2<i32>, ground: i32) {
        self.command("kill_npcs", &[SECURE_RADIUS.to_string()]);
        self.ticks(10);
        if self.pos().distance(Self::stand_pos(xy, ground)) > 0.25 {
            println!(
                "    moved off the test spot ({}), going back",
                self.status()
            );
            if !self.stand_on(xy, ground) {
                println!("    could not get back onto the test spot");
            }
        }
    }

    /// Send a terrain edit and wait for the server to apply or reject it.
    fn edit(&mut self, pos: Vec3<i32>, place: Option<Block>) -> EditResult {
        let before = self.block(pos);
        self.inventory_events.clear();
        match place {
            Some(block) => self.client.place_block(pos, block),
            None => self.client.remove_block(pos),
        }
        let mut result = EditResult::NoResponse;
        self.wait(Duration::from_secs(5), |h| {
            if let Some(reason) = h.inventory_events.iter().find_map(|e| match e {
                InventoryUpdateEvent::BuildFailed { pos: p, reason } if *p == pos => Some(*reason),
                _ => None,
            }) {
                result = EditResult::Failed(reason);
                true
            } else if let Some(now) = h.block(pos).filter(|b| Some(*b) != before) {
                result = EditResult::Changed(now);
                true
            } else {
                false
            }
        });
        // Give the matching inventory update a moment to arrive as well.
        self.ticks(5);
        result
    }

    fn give(&mut self, item: &str, amount: u32) {
        self.command("give_item", &[item.to_owned(), amount.to_string()]);
    }

    fn enter_game(&mut self) {
        self.client.load_character_list();
        assert!(
            self.wait(Duration::from_secs(30), |h| !h
                .client
                .character_list()
                .loading),
            "Character list did not load"
        );
        if self.client.character_list().characters.is_empty() {
            self.client.create_character(
                "BuildCheck".to_owned(),
                Some("common.items.weapons.sword.starter".to_owned()),
                None,
                comp::body::humanoid::Body::random().into(),
                false,
                None,
            );
            self.client.load_character_list();
            assert!(
                self.wait(Duration::from_secs(30), |h| {
                    let list = h.client.character_list();
                    !list.loading && !list.characters.is_empty()
                }),
                "Failed to create a character"
            );
        }
        let id = self.client.character_list().characters[0]
            .character
            .id
            .expect("Listed character has no id");
        self.client.request_character(id, common::ViewDistances {
            terrain: 3,
            entity: 3,
        });
        assert!(
            self.wait(Duration::from_secs(120), |h| {
                h.client.current::<comp::Pos>().is_some_and(|p| {
                    h.column_loaded(p.0.xy().as_()) && h.client.current::<comp::Body>().is_some()
                })
            }),
            "Did not spawn into the world"
        );
        assert!(
            self.wait(Duration::from_secs(10), |h| h.client.is_moderator()),
            "{USERNAME} must be an admin: `veloren-server-cli admin add {USERNAME} admin`"
        );
        self.ticks(30);
    }
}

/// A spot away from any site, where the ground can be gathered without a
/// pick: `(standing column, ground altitude, target block)`.
fn find_wilderness(h: &mut Harness, around: Vec2<i32>) -> Option<(Vec2<i32>, i32, Vec3<i32>)> {
    let world_size = h.client.world_data().chunk_size().as_::<i32>() * 32;
    for radius in [400, 700, 1000, 1500] {
        for dir in 0..8 {
            let angle = dir as f32 * std::f32::consts::FRAC_PI_4;
            let xy = around + (Vec2::new(angle.cos(), angle.sin()) * radius as f32).as_::<i32>();
            if xy.x < 64 || xy.y < 64 || xy.x > world_size.x - 64 || xy.y > world_size.y - 64 {
                continue;
            }
            let Some(ground) = h.teleport(xy) else {
                continue;
            };
            let target_xy = xy + Vec2::unit_x() * 2;
            let Some(target_z) = h.ground(target_xy) else {
                continue;
            };
            let target = target_xy.with_z(target_z);
            let gatherable = h.block(target).is_some_and(|b| {
                BuildingBlock::from_terrain(b).is_some()
                    && BuildingBlock::required_tool(b).is_none()
            });
            let dry = h
                .block(target + Vec3::unit_z())
                .is_some_and(|b| !b.is_liquid());
            let reach = h.reach_to(target);
            if !gatherable || !dry || reach > SURVIVAL_BUILD_REACH - 0.5 {
                println!(
                    "    {xy}: unsuitable ground ({:?}, gatherable without tools: {gatherable}, \
                     dry: {dry}, reach {reach:.1}), trying elsewhere",
                    h.block(target).map(|b| b.kind())
                );
                continue;
            }
            // Probe for site protection with a harmless break.
            match h.edit(target, None) {
                EditResult::Failed(BuildFailedReason::Protected) => {
                    println!("    {xy}: protected, trying elsewhere");
                    continue;
                },
                EditResult::Changed(_) => {
                    // `run` continues from this gathered block.
                    let gathered = h
                        .inventory_events
                        .iter()
                        .any(|e| matches!(e, InventoryUpdateEvent::Collected(_)));
                    println!("    {xy}: probe break ok (collected: {gathered})");
                    return Some((xy, ground, target));
                },
                other => println!("    {xy}: probe break gave {other:?}"),
            }
        }
    }
    None
}

fn run(h: &mut Harness, state_file: &PathBuf) {
    h.check(
        "server advertises survival building",
        h.client.server_constants().survival_building,
    );

    let spawn = h.pos().xy().as_::<i32>();

    // --- Protected sites -------------------------------------------------
    let town = h
        .client
        .sites()
        .values()
        .filter(|s| s.marker.kind == MarkerKind::Town)
        .map(|s| s.marker.wpos.as_::<i32>())
        .min_by_key(|wpos| wpos.distance_squared(spawn));
    match town.and_then(|town| Some((town, h.teleport(town)?))) {
        Some((town, ground)) => {
            // The block we stand on is always within reach.
            let target = town.with_z(ground);
            let before = h.block(target);
            let result = h.edit(target, None);
            h.check(
                &format!("breaking terrain in a town is rejected as protected ({result:?})"),
                result == EditResult::Failed(BuildFailedReason::Protected),
            );
            h.check("protected terrain is unchanged", h.block(target) == before);
        },
        None => h.check("found a town to test protection", false),
    }

    // --- Gathering and placing -------------------------------------------
    let Some((stand, ground, target)) = find_wilderness(h, spawn) else {
        h.check("found unprotected, gatherable ground", false);
        return;
    };
    println!(
        "    testing at {stand} (ground {ground}), target {target}, reach {:.2}, {}",
        h.reach_to(target),
        h.status()
    );
    // The probe in `find_wilderness` gathered the target block.
    let gathered_block = h
        .inventory_events
        .iter()
        .find_map(|e| match e {
            InventoryUpdateEvent::Collected(item) => BuildingBlock::all().find(|b| {
                matches!(item.item_definition_id(), ItemDefinitionId::Simple(id)
                    if id == b.item_definition_id())
            }),
            _ => None,
        })
        .expect("Probe break did not report a building block");
    h.check(
        &format!("breaking ground removes it and yields {gathered_block:?}"),
        h.block(target).is_some_and(|b| !b.is_filled()) && h.count(gathered_block) >= 1,
    );
    let above = target + Vec3::unit_z();
    h.check(
        "nothing is left floating above a gathered block",
        h.block(above).is_some_and(|b| {
            !b.is_filled() && b.get_sprite().is_none_or(|s| s == SpriteKind::Empty)
        }),
    );

    h.secure(stand, ground);

    // Placing without the item.
    let missing = BuildingBlock::all()
        .find(|b| h.count(*b) == 0)
        .expect("Inventory holds every building block");
    let result = h.edit(target, Some(missing.block()));
    h.check(
        &format!("placing {missing:?} without having any is rejected ({result:?})"),
        result == EditResult::Failed(BuildFailedReason::MissingBlock),
    );

    // Placing into the player.
    let feet = h.pos().map(|e| e.floor() as i32);
    let count = h.count(gathered_block);
    let result = h.edit(feet, Some(gathered_block.block()));
    h.check(
        &format!("placing a block inside the player is rejected ({result:?})"),
        result == EditResult::Failed(BuildFailedReason::Obstructed),
    );
    h.check(
        "a rejected placement keeps the item",
        h.count(gathered_block) == count,
    );

    // Placing and gathering again round-trips.
    let result = h.edit(target, Some(gathered_block.block()));
    h.check(
        &format!("placing {gathered_block:?} puts its canonical block ({result:?})"),
        result == EditResult::Changed(gathered_block.block()),
    );
    let ok = h.wait(Duration::from_secs(3), |h| {
        h.count(gathered_block) == count - 1
    });
    h.check("placing consumes one block", ok);
    let result = h.edit(target, None);
    h.check(
        &format!("a placed block can be gathered again ({result:?})"),
        matches!(result, EditResult::Changed(b) if !b.is_filled()),
    );
    let ok = h.wait(Duration::from_secs(3), |h| h.count(gathered_block) == count);
    h.check("and yields the same item", ok);

    // Creative build areas: protected from survival edits, and creative
    // building keeps working inside them (and only there).
    h.secure(stand, ground);
    const AREA: &str = "survival_build_check";
    let area_cmd = |name: &str| {
        [AREA, "build"]
            .into_iter()
            .map(str::to_owned)
            .chain(
                [target.x, target.x, target.y, target.y, target.z, target.z]
                    .iter()
                    .map(|e| e.to_string())
                    .filter(|_| name == "area_add"),
            )
            .collect::<Vec<_>>()
    };
    // Left over if an earlier run on this server stopped half way.
    h.command("area_remove", &area_cmd("area_remove"));
    h.command("area_add", &area_cmd("area_add"));
    // Commands and edits are handled by different systems.
    h.ticks(30);
    let result = h.edit(target, Some(gathered_block.block()));
    h.check(
        &format!("placing in a build area without permission is rejected ({result:?})"),
        result == EditResult::Failed(BuildFailedReason::Protected),
    );
    h.command("permit_build", &[AREA.to_owned()]);
    h.ticks(30);
    h.command("build", &[]);
    let creative = h.wait(Duration::from_secs(5), |h| {
        h.client
            .current::<comp::CanBuild>()
            .is_some_and(|b| b.enabled)
    });
    h.check("creative build mode is on", creative);
    if creative {
        let outside = target + Vec3::unit_z();
        let result = h.edit(outside, Some(gathered_block.block()));
        h.check(
            &format!("creative edits outside a permitted area are ignored ({result:?})"),
            result == EditResult::NoResponse,
        );
        let result = h.edit(target, Some(gathered_block.block()));
        h.check(
            &format!("creative placement works in a permitted area ({result:?})"),
            result == EditResult::Changed(gathered_block.block()),
        );
        let result = h.edit(target, None);
        h.check(
            &format!("creative breaking works in a permitted area ({result:?})"),
            matches!(result, EditResult::Changed(b) if !b.is_filled()),
        );
        h.check(
            "creative building neither consumes nor gives items",
            h.count(gathered_block) == count,
        );
        h.command("build", &[]);
        let ok = h.wait(Duration::from_secs(5), |h| {
            h.client
                .current::<comp::CanBuild>()
                .is_none_or(|b| !b.enabled)
        });
        h.check("creative build mode is off again", ok);
    }
    h.command("area_remove", &area_cmd("area_remove"));
    h.ticks(30);
    h.secure(stand, ground);

    // Reach.
    let far_xy = stand + Vec2::unit_x() * 12;
    if let Some(far_z) = h.ground(far_xy) {
        let far = far_xy.with_z(far_z);
        let before = h.block(far);
        let result = h.edit(far, None);
        h.check(
            &format!(
                "breaking a block {:.1} blocks away is rejected ({result:?})",
                h.reach_to(far)
            ),
            result == EditResult::Failed(BuildFailedReason::OutOfReach),
        );
        h.check("out of reach terrain is unchanged", h.block(far) == before);
    }

    h.secure(stand, ground);

    // Tools. Counts are relative so that the check can run again with the
    // same character.
    let stone = h.count(BuildingBlock::Stone) + 3;
    h.give(BuildingBlock::Stone.item_definition_id(), 3);
    let ok = h.wait(Duration::from_secs(5), |h| {
        h.count(BuildingBlock::Stone) == stone
    });
    h.check("received stone blocks", ok);
    for slot in HAND_SLOTS {
        if h.equipped_tool(slot) == Some(ToolKind::Pick) {
            h.client.use_slot(Slot::Equip(slot));
        }
    }
    let ok = h.wait(Duration::from_secs(5), |h| {
        HAND_SLOTS
            .iter()
            .all(|slot| h.equipped_tool(*slot) != Some(ToolKind::Pick))
    });
    h.check("no pickaxe is equipped", ok);
    let result = h.edit(target, Some(BuildingBlock::Stone.block()));
    h.check(
        &format!("placing stone works ({result:?})"),
        result == EditResult::Changed(BuildingBlock::Stone.block()),
    );
    let result = h.edit(target, None);
    h.check(
        &format!("breaking stone without a pickaxe is rejected ({result:?})"),
        result == EditResult::Failed(BuildFailedReason::RequiresTool(ToolKind::Pick)),
    );
    h.check(
        "stone stays in place",
        h.block(target) == Some(BuildingBlock::Stone.block()),
    );

    h.give("common.items.tool.pickaxe_stone", 1);
    let has_pick = |h: &Harness| {
        let inventories = h.client.inventories();
        inventories.get(h.client.entity()).and_then(|inv| {
            inv.slots_with_id()
                .find(|(_, item)| {
                    item.as_ref()
                        .is_some_and(|i| i.tool_info() == Some(ToolKind::Pick))
                })
                .map(|(slot, _)| slot)
        })
    };
    if h.wait(Duration::from_secs(5), |h| has_pick(h).is_some())
        && let Some(slot) = has_pick(h)
    {
        h.client.use_slot(Slot::Inventory(slot));
        // Equipping prefers an empty hand slot, and a tool counts in any
        // of them.
        let equipped = h.wait(Duration::from_secs(5), |h| {
            HAND_SLOTS
                .iter()
                .any(|slot| h.equipped_tool(*slot) == Some(ToolKind::Pick))
        });
        h.check("equipped a pickaxe", equipped);
    } else {
        h.check("received a pickaxe", false);
    }
    let result = h.edit(target, None);
    h.check(
        &format!("breaking stone with a pickaxe works ({result:?})"),
        matches!(result, EditResult::Changed(b) if !b.is_filled()),
    );
    let ok = h.wait(Duration::from_secs(3), |h| {
        h.count(BuildingBlock::Stone) == stone
    });
    h.check("and gives the stone back", ok);

    // Persistence: leave a block behind for `verify`.
    let result = h.edit(target, Some(BuildingBlock::Stone.block()));
    h.check(
        &format!("placed a stone block to check persistence ({result:?})"),
        result == EditResult::Changed(BuildingBlock::Stone.block()),
    );
    std::fs::write(
        state_file,
        format!("{} {} {}\n", target.x, target.y, target.z),
    )
    .expect("Failed to write the state file");
    println!("    wrote {}", state_file.display());
}

fn verify(h: &mut Harness, state_file: &PathBuf) {
    let state = std::fs::read_to_string(state_file).expect("Failed to read the state file");
    let coords: Vec<i32> = state
        .split_whitespace()
        .map(|s| s.parse().expect("Invalid state file"))
        .collect();
    let target = Vec3::new(coords[0], coords[1], coords[2]);
    let loaded = h.teleport(target.xy() - Vec2::unit_x() * 2).is_some();
    h.check("terrain at the saved position loaded", loaded);
    h.check(
        &format!(
            "the stone block placed before the restart is still there ({:?})",
            h.block(target)
        ),
        h.block(target) == Some(BuildingBlock::Stone.block()),
    );
}

fn main() {
    common_frontend::init_stdout(None);
    let mut args = std::env::args().skip(1);
    let (mode, port, state_file) = match (args.next(), args.next(), args.next()) {
        (Some(mode), Some(port), Some(state)) => (
            mode,
            port.parse::<u16>().expect("Invalid port"),
            PathBuf::from(state),
        ),
        _ => {
            eprintln!("usage: survival_build_check <run|verify> <port> <state file>");
            std::process::exit(2);
        },
    };

    let runtime = Arc::new(Runtime::new().expect("Failed to start the async runtime"));
    let mut h = Harness::connect(port, runtime);
    h.enter_game();
    match mode.as_str() {
        "run" => run(&mut h, &state_file),
        "verify" => verify(&mut h, &state_file),
        _ => panic!("Unknown mode {mode}"),
    }
    // Let pending messages go out before disconnecting.
    h.ticks(15);

    if h.failures == 0 {
        println!("All survival building checks passed.");
    } else {
        println!("{} survival building check(s) failed.", h.failures);
        std::process::exit(1);
    }
}
