# Valocraft — Voxel Building & Adventure Mechanics Plan

This document maps the existing Veloren terrain/build/crafting machinery and gives a
concrete, staged implementation plan for Minecraft-style block destruction, block
placement, block items, and workbench recipes.

The important discovery is that **the hard part already exists**: Veloren has a
server-authoritative block-edit pipeline, a client-side build raycast, a `CanBuild`
component with build areas, and network messages for both breaking and placing blocks.
The work is therefore mostly about:

1. Making block items first-class items.
2. Granting build mode automatically in local singleplayer.
3. Validating/consuming/dropping items on the server.
4. Exposing blocks through the existing hotbar/crafting UI.
5. Adding survival/adventure rules (tools, hardness, drops, progression).

## Implementation status (branch `feature/survival-building`)

Phases 1, 3, 4 and 5 are implemented. The design changed in a few places while
building it, and the record of what was actually done lives in
`docs/TECHNICAL_DECISIONS.md` (TD-001). The main differences from this plan:

- **Survival rules instead of granting `CanBuild` (phase 2 dropped).** Players
  don't get creative build rights. Edits outside creative build areas go
  through a separate server handler (`server/src/events/building.rs`) when
  `gameplay.survival_building` is on and terrain persistence is enabled.
  Creative areas still work as before and take precedence.
- **One canonical voxel per block item.** `BuildingBlock`
  (`common/src/comp/inventory/item/building.rs`) is the only lookup table, and
  `ItemKind::Block(BuildingBlock)` replaces the `BlockItem` struct sketched in
  1.1. Natural terrain is quantised by `BlockKind`.
- **Reach** is 6 blocks from the eye to the block centre, plus 1.5 blocks of
  server tolerance.
- **Build mode** is toggled with `Z` (`GameInput::ToggleBuildMode`), or by using
  a block item. It isn't tied to the active hotbar item.
- **No rate limit yet** (3.4). It is listed as a known limitation.
- **Phase 6** (adventure mechanics) hasn't been started. The next candidate is
  room recognition, see `docs/GAME_DESIGN.md`.

---

## 0. Existing engine pieces (do not reinvent)

| Concern | Existing code |
|---|---|
| Block model (`Block`, `BlockKind`, color) | `common/src/terrain/block.rs` |
| Chunk storage / subchunks | `common/src/terrain/chonk.rs`, `common/src/terrain/map.rs` |
| Edit queue | `common/state/src/state.rs` — `BlockChange`, `TerrainChanges`, `State::apply_terrain_changes` |
| Client→server messages | `common/net/src/msg/client.rs` — `ClientGeneral::BreakBlock(Vec3<i32>)`, `ClientGeneral::PlaceBlock(Vec3<i32>, Block)` |
| Client senders | `client/src/lib.rs` — `Client::remove_block` (line ~2166), `Client::place_block` (line ~2161) |
| Client build targeting | `voxygen/src/session/target.rs` — `Target<Build>`, `targets_under_cursor` |
| Client build input | `voxygen/src/session/mod.rs` — `can_build`, `build_target`, `GameInput::Primary` (break), `GameInput::Secondary` (place), `GameInput::Roll` (pick) |
| Server edit validation | `server/src/sys/msg/in_game.rs` — `BreakBlock` / `PlaceBlock` arms (around lines 161–215) |
| Permission component | `common/src/comp/inputs.rs` — `CanBuild { enabled, build_areas }` |
| Build areas | `common/state/src/special_areas.rs` — `BuildArea`; `server/src/cmd.rs` — `/area_add`, `/permit_build`, `/build` |
| Terrain changes → clients | `server/src/sys/terrain_sync.rs` — `ServerGeneral::TerrainBlockUpdates` |
| Mining/drops/crafting | `server/src/events/interaction.rs`, `server/src/events/inventory_manip.rs` |
| Recipe definitions | `assets/common/recipe_book_manifest.ron` |
| Recipe groups | `assets/common/items/recipes/*.ron` |
| Starting recipe book | `server/src/character_creator.rs` — `push_recipe_group("common.items.recipes.default")` |
| Crafting UI | `voxygen/src/hud/crafting.rs` — `CraftingTab::satisfies` |

### Current admin-only build flow

```text
/area_add spawn build -64 64 -64 64 -64 64   # create a build area AABB
/permit_build spawn                         # add it to your CanBuild (enabled = false)
/build                                      # toggle build mode on
```

Once `CanBuild.enabled` is true, the existing client behavior is already close to
Minecraft creative mode:

- `Primary` → `Client::remove_block(pos)` → server sets the block vacant.
- `Secondary` → `Client::place_block(pos, selected_block)` → server places the block.
- `Roll` (middle mouse, by default) → picks the targeted block as `selected_block`.

The default `selected_block` is `Block::new(BlockKind::Misc, Rgb::broadcast(255))`
(magenta debug block), so a block palette is needed.

---

## 1. Phase 1 — Block items as first-class items

**Goal:** every placeable block can exist in an inventory, be dropped, and be crafted.

### 1.1 Add a block item kind

File: `common/src/comp/inventory/item/mod.rs`

```rust
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockItem {
    /// Terrain material family (Rock, Earth, Wood, ...).
    pub kind: BlockKind,
    /// RGB tint stored in every placed block.
    pub color: [u8; 3],
}

impl BlockItem {
    pub fn to_block(self) -> Block {
        Block::new(self.kind, Rgb::new(self.color[0], self.color[1], self.color[2]))
    }
}

pub enum ItemKind {
    // ... existing variants ...
    Block(BlockItem),
}
```

Add a helper next to the other `ItemKind` helpers:

```rust
impl ItemKind {
    pub fn as_block(&self) -> Option<Block> {
        match self {
            ItemKind::Block(block) => Some(block.to_block()),
            _ => None,
        }
    }
}
```

Update the compiler-flagged exhaustive matches. The main ones are:

- `get_itemkind_string()` → `format!("Block: {:?}", block.kind)`
- `has_durability()` → `false`
- `is_equippable()` → `false`
- Any UI match that classifies items (search `ItemKind::` — ~183 references, but only
  the exhaustive matches need changes).
- Salvage/trade filters (`common/src/comp/inventory/trade_pricing.rs`).

### 1.2 Block item RON

Add `assets/common/items/blocks/`:

```ron
// assets/common/items/blocks/stone.ron
ItemDef(
    legacy_name: "Stone",
    legacy_description: "A sturdy block of stone.",
    kind: Block((
        kind: Rock,
        color: [128, 128, 128],
    )),
    quality: Common,
    tags: [Utility],
)
```

Suggested starter palette (all `ItemTag::Utility` so they appear in the existing
Utility crafting tab without a new tab):

```text
stone       Rock     [128,128,128]
dirt        Earth    [110, 80, 50]
grass       Grass    [ 80,140, 60]
sand        Sand     [210,190,140]
wood        Wood     [110, 80, 45]
leaves      Leaves   [ 60,120, 50]
snow        Snow     [235,240,245]
glowstone   GlowingRock [230,200,120]
```

Block item IDs become `common.items.blocks.stone`, etc.

### 1.3 Block ↔ item lookup

Add a small resolver (new file `common/src/block_item.rs` or inside `terrain/block.rs`)
that maps natural terrain to a default drop item:

```rust
pub fn default_drop_for(block: &Block) -> Option<&'static str> {
    match block.kind() {
        BlockKind::Rock | BlockKind::WeakRock => Some("common.items.blocks.stone"),
        BlockKind::Earth => Some("common.items.blocks.dirt"),
        BlockKind::Grass => Some("common.items.blocks.grass"),
        BlockKind::Sand => Some("common.items.blocks.sand"),
        BlockKind::Wood => Some("common.items.blocks.wood"),
        BlockKind::Leaves => Some("common.items.blocks.leaves"),
        BlockKind::Snow | BlockKind::ArtSnow => Some("common.items.blocks.snow"),
        BlockKind::GlowingRock | BlockKind::GlowingWeakRock => Some("common.items.blocks.glowstone"),
        _ => None,
    }
}
```

For player-placed blocks, prefer an exact `(BlockKind, color)` lookup in the block-item
registry first, then fall back to the kind-based default. This preserves the chosen
block item when a player breaks a block they placed.

### 1.4 Icons

Item icons in Veloren are resolved from the item definition/asset pipeline. For the
first slice, reuse one generic block icon (or a per-material tinted cube) so the
inventory/hotbar can render blocks. Add proper per-block icons later; the crafting and
hotbar widgets only need a valid icon key.

---

## 2. Phase 2 — Auto-enable sandbox building in singleplayer

**Goal:** after Valocraft's auto-boot, the player can break/place immediately.

### 2.1 Add a server sandbox flag

File: `server/src/settings/mod.rs`

```rust
pub struct Settings {
    // ... existing fields ...
    /// Valocraft: allow block editing without admin build areas.
    #[serde(default)]
    pub sandbox_building: bool,
}
```

- `Settings::default()` → `sandbox_building: false`
- `Settings::singleplayer()` → `sandbox_building: true`

This keeps dedicated servers safe and gives the integrated server creative-building
behavior by default.

### 2.2 Grant `CanBuild` on character initialization

File: `server/src/events/entity_creation.rs` — `handle_initialize_character`.

After `server.state.initialize_character_data(...)`:

```rust
if server.settings().sandbox_building {
    // Build areas use integer world coordinates.
    let spawn: Vec3<i32> = server.state.ecs().read_resource::<SpawnPoint>().0.map(|e| e as i32);
    let area_id = {
        let mut areas = server
            .state
            .mut_resource::<AreasContainer<BuildArea>>();
        let name = "valocraft_sandbox";
        if let Some(id) = areas.area_metas().get(name).copied() {
            id
        } else {
            areas
                .insert(
                    name.to_string(),
                    Aabb {
                        min: spawn - Vec3::new(128, 128, 64),
                        max: spawn + Vec3::new(128, 128, 64),
                    },
                )
                .expect("fresh sandbox area name")
        }
    };

    server.state.write_component_ignore_entity_dead(
        ev.entity,
        comp::CanBuild {
            enabled: true,
            build_areas: [area_id].into_iter().collect(),
        },
    );
}
```

Notes:

- The build-area AABB upper bound is inclusive (there is an existing TODO about this in
  `server/src/sys/msg/in_game.rs`).
- `spawn` is available as the `SpawnPoint` resource (`server/src/state_ext.rs` already
  reads it).
- If you prefer not to create an AABB, the alternative is to bypass the `CanBuild`
  check in the `BreakBlock`/`PlaceBlock` arms when
  `server.settings().sandbox_building` is true, and enforce a reach limit instead. The
  build-area approach is safer and keeps the existing permission model.

### 2.3 Optional build-mode toggle

Keep `/build` as a way to turn building off while exploring. The auto-granted
`CanBuild` starts enabled; `/build` flips `enabled` without removing the area.

---

## 3. Phase 3 — Server-authoritative break/place rules

**Goal:** blocks are consumed/dropped correctly and cannot be edited at arbitrary range.

File: `server/src/sys/msg/in_game.rs`, arms `ClientGeneral::BreakBlock` and
`ClientGeneral::PlaceBlock`.

### 3.1 Reach validation

Add a shared helper:

```rust
fn within_build_reach(player_pos: Vec3<f32>, block_pos: Vec3<i32>) -> bool {
    // Eye-to-block-center distance; tune to taste.
    player_pos.distance_squared(block_pos.map(|e| e as f32) + Vec3::broadcast(0.5))
        <= 8.0 * 8.0
}
```

Use `comp::Pos` of the player (already available in the system data). Reject edits
outside the radius. Also reject edits to the block the player is standing inside of if
desired.

### 3.2 Place: require and consume a block item

In the `PlaceBlock` arm:

1. Verify `new_block.kind().is_filled()` (reject air/fluid placement).
2. Verify the target is air/replaceable
   (`terrain.get(pos).ok().is_some_and(|b| !b.is_filled())`).
3. Read the player's active inventory item (`Inventory`, `EquipSlot::ActiveMainhand`,
   or the selected hotbar slot).
4. Require `item.kind().as_block() == Some(new_block)` (or compare kind+color).
5. In survival mode, consume one item (`Inventory::take` / `take_amount`, see
   `common/src/comp/inventory/mod.rs` lines ~604–660). In creative/sandbox mode,
   leave the stack untouched.
6. Only then `block_changes.try_set(pos, new_block)`.

The in-game message system does not currently read inventories. Add
`ReadStorage<'a, Inventory>` (or `WriteStorage` if consuming) to `Sys::SystemData` in
`server/src/sys/msg/in_game.rs`, pass it into `handle_client_in_game_msg`, and look up
the component with `inventories.get(entity)`.

If the item check fails, send a `ServerGeneral` chat/info message or simply ignore the
edit. Ignoring is cheapest for the prototype.

### 3.3 Break: drop the block item

In the `BreakBlock` arm, before/after setting the block vacant:

1. Read the old block (`terrain.get(pos)`).
2. Reject `!old_block.is_filled()`.
3. Compute the drop item via `default_drop_for(&old_block)` or the exact block-item
   registry.
4. Emit a `CreateItemDropEvent` (the same event used by
   `server/src/events/interaction.rs` for mining loot), or insert the item directly
   into the player's inventory for the first slice. Emitting a drop is more
   Minecraft-like and reuses the existing pickup/ownership code.
5. Set `block_changes.try_set(pos, old_block.into_vacant())`.

To emit the drop from this system, register a new emitter at the top of
`server/src/sys/msg/in_game.rs`:

```rust
event_emitters! {
    struct Events[Emitters] {
        // ... existing emitters ...
        create_item_drop: event::CreateItemDropEvent,
    }
}
```

Then:

```rust
// `ProgramTime` is a resource; add `Read<ProgramTime>` to `Sys::SystemData`
// (and pass `*program_time` into this helper) if it is not already available.
if let Some(item_id) = default_drop_for(&old_block) {
    let item = Item::new_from_asset_expect(item_id);
    emitters.create_item_drop.emit(CreateItemDropEvent {
        pos: comp::Pos(pos.map(|e| e as f32) + Vec3::broadcast(0.5)),
        vel: comp::Vel(Vec3::unit_z() * 5.0),
        ori: comp::Ori::from(Dir::random_2d(&mut rand::rng())),
        item: comp::PickupItem::new(item, *program_time, false),
        loot_owner: None, // or Some(...) to restrict pickup to the breaker
    });
}
```

Persistence is already handled: the existing code calls `terrain_persistence.set_block`
when `persistent_world` is enabled.

### 3.4 Rate limiting / anti-grief

The current handler allows one change per position per tick (`try_set`). Add:

- A per-player edits-per-tick cap.
- A whitelist of placeable `BlockKind`s (reject `Air`, `Water`, `Lava`, and sprite-only
  blocks in survival mode).
- Optional “block ownership” for protected areas (reuse `BuildArea`).
- The existing build-area check remains the outer gate.

---

## 4. Phase 4 — Client interaction, hotbar, and preview

### 4.1 Use the active item as the placement block

File: `voxygen/src/session/mod.rs`, around the `GameInput::Secondary` handler (lines
~809–820).

Today:

```rust
GameInput::Secondary => {
    self.walking_speed = false;
    let mut client = self.client.borrow_mut();
    if let Some(build_target) = build_target.filter(|_| state) {
        let selected_pos = build_target.kind.0.map(|p| p.floor() as i32);
        client.place_block(selected_pos, self.selected_block);
    } else {
        client.handle_input(/* ... normal Secondary ... */);
    }
}
```

Change to resolve the block from the active inventory item first:

```rust
GameInput::Secondary => {
    self.walking_speed = false;

    // `Client::current::<T>()` clones the player's ECS component.
    let active_block = self
        .client
        .borrow()
        .current::<comp::Inventory>()
        .and_then(|inv| {
            inv.equipped(EquipSlot::ActiveMainhand)
                .and_then(|item| item.kind().as_block())
        });

    let mut client = self.client.borrow_mut();
    if let Some(build_target) = build_target.filter(|_| state) {
        let selected_pos = build_target.kind.0.map(|p| p.floor() as i32);
        client.place_block(
            selected_pos,
            active_block.unwrap_or(self.selected_block),
        );
    } else {
        client.handle_input(/* ... normal Secondary ... */);
    }
}
```

Keep `self.selected_block` as a creative-palette fallback. The server is authoritative
and will reject placements that don't match the held item.

### 4.2 Block palette / hotbar

- Survival: the hotbar is the inventory. Put stackable block items into the active
  mainhand/offhand slots; selecting a slot selects that block.
- Creative/sandbox: add a simple palette UI (a grid of block items, scroll wheel to
  cycle). The existing `voxygen/src/hud/hotbar.rs` and slot-grid widgets can be reused;
  a minimal first version can bind `1..9`/scroll to a small array of block items.
- The existing `selected_block` + `scene.set_select_pos(...)` already provides a
  highlighted target; add a translucent “ghost block” preview at `build_target` for
  placement feedback.

### 4.3 Break behavior

The existing `Primary` handler in build mode calls `client.remove_block(...)`, which is
instant creative-style breaking. For survival:

- Keep `GameInput::Primary` as the mining/tool attack input
  (`BlockInteraction::Mine` in `voxygen/src/session/interactable.rs`) so tool tiers and
  block hardness still apply.
- Add a “sandbox/creative” toggle that switches between instant break and tool-based
  mining. The server setting `sandbox_building` can drive the default.

### 4.4 Block pick (`Roll`)

The existing `GameInput::Roll` picks the targeted block into `self.selected_block`.
Extend it to also select/find the matching block item in the inventory, so creative
pick and survival hotbar selection converge.

---

## 5. Phase 5 — Recipes, workbench, and UI hookup

### 5.1 Add recipe entries

File: `assets/common/recipe_book_manifest.ron`

```ron
"stone_bricks": (
    output: ("common.items.blocks.stone_bricks", 4),
    inputs: [
        (Item("common.items.blocks.stone"), 4, false),
    ],
    // None = hand-craftable; Some(CraftingBench) = workbench required.
    craft_sprite: Some(CraftingBench),
),
"dirt_path": (
    output: ("common.items.blocks.dirt_path", 4),
    inputs: [
        (Item("common.items.blocks.dirt"), 4, false),
    ],
    craft_sprite: None,
),
```

`craft_sprite` is already enforced server-side in
`server/src/events/inventory_manip.rs` (the crafting handler checks the nearby sprite).

### 5.2 Make recipes visible in the crafting UI

Two options:

**Fast path (recommended first):**

- Tag block items with `ItemTag::Utility`.
- Add the recipe keys to `assets/common/items/recipes/default.ron`’s `recipes` list.
  New characters already receive `common.items.recipes.default` in
  `server/src/character_creator.rs`, so the recipes appear immediately in the existing
  Utility/All tabs.

**Cleaner path (second pass):**

- Create `assets/common/items/recipes/building.ron`:

  ```ron
  ItemDef(
      legacy_name: "",
      legacy_description: "",
      kind: RecipeGroup(
          recipes: ["stone_bricks", "dirt_path", "wood_planks", ...],
      ),
      quality: Common,
      tags: [],
  )
  ```

- Grant it to new characters in `server/src/character_creator.rs` with
  `inventory.push_recipe_group(Item::new_from_asset_expect("common.items.recipes.building"))`.
- Add `CraftingTab::Building` to `voxygen/src/hud/crafting.rs`:
  - `name_key()` → `"hud-crafting-tabs-building"` (add the `.ftl` string).
  - `icon()` → a cube icon.
  - `satisfies()` → `matches!(&*item.kind(), ItemKind::Block(_))`.
- Add the tab to the tab list in the crafting UI’s tab bar.

### 5.3 Workbench progression

- Basic blocks: `craft_sprite: None` (hand crafting).
- Refined blocks (bricks, planks, glass, polished stone):
  `craft_sprite: Some(CraftingBench)`.
- Later, add a `CraftingBench`-like sprite for a “workbench” or reuse the existing
  `CraftingBench` sprite (already defined in `common/src/terrain/sprite/mod.rs`, line
  ~143). This avoids new sprite assets for the first iteration.

### 5.4 Localization and icons

- Add `.ftl` strings under `assets/voxygen/i18n/en/item/blocks.ftl` (and optionally the
  other locales) for each block item name/description.
- Add crafting-tab strings under `assets/voxygen/i18n/en/hud/crafting.ftl`.
- Reuse/produce icons for the block palette.

---

## 6. Phase 6 — Adventure mechanics

Once block items and edits work, layer the survival loop:

1. **Tool tiers and mining speed**
   - `Block::mine_tool()` already maps blocks to required tool kinds.
   - `server/src/events/interaction.rs` already applies block damage based on the tool
     and emits `Outcome::DamagedBlock` / `Outcome::BreakBlock`.
   - Add per-`BlockKind` hardness (a tolerance/damage table) and use it to gate
     progression (wood → stone → iron pickaxes).

2. **Drops and inventory**
   - Use `default_drop_for` + `CreateItemDropEvent` for terrain blocks.
   - Add fortune/silk-touch-like modifiers later via `MaterialStatManifest`.

3. **Workbench and crafting progression**
   - Gate refined blocks behind `CraftingBench`.
   - Add a “building” recipe group and grant it on character creation.
   - Optionally add a dedicated building tab with icons.

4. **Structural rules (optional)**
   - Gravity for sand/gravel, support requirements for floating blocks, and
     explosion behavior. The engine already supports `SpriteAdjecencyRequirement` and
     `BonkEvent` for sprite blocks; terrain blocks currently do not simulate gravity.
     This is a larger subsystem and should be a separate milestone.

5. **Persistence and multiplayer**
   - `persistent_world` already persists block edits via
     `terrain_persistence.set_block`.
   - `server/src/sys/terrain_sync.rs` broadcasts `TerrainBlockUpdates` to all clients;
     no new network message is required for basic edits.

---

## 7. First implementation slice (recommended order)

This is the smallest sequence that produces a playable “break block → get item →
craft block → place block” loop:

1. Add `BlockItem` + `ItemKind::Block` and fix compiler-flagged matches.
2. Add 4–8 block item RONs (`stone`, `dirt`, `grass`, `sand`, `wood`, `leaves`, ...).
3. Add `sandbox_building` to server settings and auto-grant `CanBuild` on character
   initialization.
4. Add server reach checks and item consumption/drop for `BreakBlock`/`PlaceBlock`.
5. Change the client’s `Secondary` placement to use the active block item.
6. Add 3–5 recipes to `recipe_book_manifest.ron` and list them in `default.ron`.
7. Grant a starter stack of block items on character creation (or add a creative
   palette) so the loop is testable immediately.
8. Build, launch, and verify:
   - Break a block with `Primary` → item drops/pickup.
   - Place it with `Secondary` → block appears for all clients.
   - Craft a recipe at/without a workbench.

### Starter inventory grant (prototype)

In `server/src/character_creator.rs`, alongside the existing potion/cheese/recipe
grants:

```rust
for item in [
    "common.items.blocks.stone",
    "common.items.blocks.dirt",
    "common.items.blocks.wood",
] {
    let mut stack = Item::new_from_asset_expect(item);
    stack.set_amount(32).expect("valid stack amount");
    inventory.push(stack).expect("inventory has space");
}
```

(Check the `Item` amount API in `common/src/comp/inventory/item/mod.rs`; use the
existing helper for stack size rather than hard-coding if available.)

---

## 8. Testing / verification

- Build: `cargo build --release --bin veloren-voxygen --no-default-features --features default-publish,hot-reloading,egui-ui,simd`
- Server: `cargo build --release --bin veloren-server-cli`
- Auto-boot log expectations:
  - integrated server starts,
  - client connects over the in-process channel,
  - character selection creates/selects `Player`,
  - session state is entered without login/server-selection UI.
- In-game checks:
  1. `Primary` on a terrain block → block disappears, drop appears.
  2. Pick up the drop → stack appears in inventory.
  3. Select the block item → `Secondary` places it.
  4. Recipe appears in the crafting UI and consumes the correct inputs.
  5. Reload the world → placed/broken blocks persist (with `persistent_world`).
- Edge cases:
  - Place inside the player/another entity (reject or allow).
  - Place at max reach (reject beyond range).
  - Break bedrock/air/fluid (reject).
  - Break a block another player is standing on (allowed; physics handles it).
  - Concurrent edits at the same position (first write wins via `try_set`).

---

## 9. Summary of key design decisions

- **Reuse the existing `CanBuild` + `BuildArea` system** rather than adding a new edit
  protocol. The network messages and terrain pipeline already work.
- **Server authority first.** The client may predict/visualize, but the server checks
  reach, permission, item ownership, and block validity.
- **Items are the source of truth for placement.** This makes crafting, drops, and
  inventory integration natural.
- **Creative/sandbox vs survival is a server setting.** Local singleplayer starts in
  sandbox mode for immediate fun; dedicated servers keep the existing admin-gated build
  areas until survival rules are ready.
- **Quick UI hookup first, dedicated Building tab later.** Reusing `ItemTag::Utility`
  and `common.items.recipes.default` gets recipes visible with almost no UI work.
