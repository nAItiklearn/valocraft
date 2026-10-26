# Veloren Workspace Index

Local checkout: `/home/naitik/Projects/valocraft`
Upstream: <https://gitlab.com/veloren/veloren.git>
Workspace version: `0.18.0` · Rust edition `2024` · resolver `2`
Pinned toolchain: `nightly-2026-06-13` (from `rust-toolchain`)

This document maps the multi-crate workspace so new gameplay features can be
added without re-discovering the layout each time.

---

## 1. Workspace crate map

| Path | Package | Role |
|---|---|---|
| `client/` | `veloren-client` | Client-side ECS, prediction, networking glue |
| `client/i18n/` | `veloren-client-i18n` | Client translations |
| `common/` | `veloren-common` | Shared gameplay data: items, stats, entities, terrain types |
| `common/assets/` | `veloren-common-assets` | Asset loading/validation (`AssetExt`, `AssetHandle`, RON loaders) |
| `common/base/` | `veloren-common-base` | Low-level helpers (profiling spans, tracy, utilities) |
| `common/dynlib/` | `veloren-common-dynlib` | Hot-reload dynamic library support |
| `common/ecs/` | `veloren-common-ecs` | ECS plumbing over `specs` (`System`, `dispatch`, `run_now`) |
| `common/frontend/` | `veloren-common-frontend` | Shared frontend/UI-facing types |
| `common/i18n/` | `veloren-common-i18n` | Shared localization (`Content`) |
| `common/net/` | `veloren-common-net` | Shared network message types |
| `common/query_server/` | `veloren-query-server` | Server query (status) endpoint |
| `common/state/` | `veloren-common-state` | Shared ECS `State`, component/resource registration |
| `common/systems/` | `veloren-common-systems` | Reusable ECS systems (combat, physics, buffs, …) |
| `network/` | `veloren-network` | Transport layer (QUIC/TCP/UDP, channels, participants) |
| `network/protocol/` | `veloren-network-protocol` | Wire protocol (frames, handshake, messages, priority) |
| `rtsim/` | `veloren-rtsim` | Real-time simulation of NPCs/settlements/economy |
| `server/` | `veloren-server` | Authoritative game server: ECS loop, persistence, chunk gen |
| `server/agent/` | `veloren-server-agent` | Server-side agent/NPC helpers |
| `server-cli/` | `veloren-server-cli` | Dedicated server binary + TUI/admin CLI |
| `voxygen/` | `veloren-voxygen` | Client binary: graphics, UI, audio, menus |
| `voxygen/anim/` | `veloren-voxygen-anim` | Skeletal animation (optional hot-reload) |
| `voxygen/credentialstore/` | `veloren-voxygen-credentialstore` | OS credential storage |
| `voxygen/egui/` | `veloren-voxygen-egui` | egui debug/dev UI integration |
| `voxygen/i18n-helpers/` | `veloren-voxygen-i18n-helpers` | UI translation helpers |
| `world/` | `veloren-world` | Procedural worldgen: terrain, biomes, rivers, sites, civs |

Build profiles (`Cargo.toml`):
- `dev`: `opt-level = 2`, overflow checks, `panic = "abort"`, no debuginfo.
- `release`: `opt-level = 3`, LTO, `panic = "abort"`, line tables for Veloren crates.
- `no_overflow`: dev-like but faster for worldgen math.
- `release-thinlto`: release with thin LTO.

---

## 2. `veloren-voxygen` — graphics / UI / WGPU pipeline

Root: `voxygen/` · binary: `veloren-voxygen` · entry: `voxygen/src/main.rs`

Key modules:

| Path | Responsibility |
|---|---|
| `voxygen/src/main.rs` | Process entry; CLI parsing, wgpu `Instance`/backend selection, audio, window, `GlobalState` |
| `voxygen/src/run.rs` | `winit` event loop; drives the `PlayState` stack |
| `voxygen/src/render/mod.rs` | Render module re-exports (meshes, models, pipelines, globals) |
| `voxygen/src/render/renderer/` | WGPU renderer core: `compiler.rs`, `pipeline_creation.rs`, `drawer.rs`, `shaders.rs`, `shadow_map.rs`, `screenshot.rs` |
| `voxygen/src/render/pipelines/` | Per-feature pipelines: `terrain.rs`, `figure.rs`, `sprite.rs`, `ui.rs`, `shadow.rs`, `skybox.rs`, `clouds.rs`, `bloom.rs`, `postprocess.rs`, `particle.rs`, `fluid.rs`, `lod_terrain.rs`, `lod_object.rs`, `trail.rs`, `rope.rs`, `rain_occlusion.rs`, `debug.rs`, `blit.rs` |
| `voxygen/src/render/mesh.rs`, `model.rs`, `texture.rs` | Mesh/model/texture abstractions |
| `voxygen/src/scene/` | Scene graph + terrain/figure rendering helpers (`scene/terrain/`, `scene/figure/`) |
| `voxygen/src/hud/` | In-game HUD (chat, inventory, settings, …) |
| `voxygen/src/ui/` | Immediate/graphical UI toolkit (`ui/graphic/`, `ui/ice/`, `ui/widgets/`) |
| `voxygen/src/menu/` | Main menu + character selection (`menu/main/`, `menu/char_selection/`) |
| `voxygen/src/singleplayer/` | Integrated-server client mode (`mod.rs`, `singleplayer_world.rs`) |
| `voxygen/src/audio/` | Audio frontend (kira/cpal) |
| `voxygen/src/settings/` | User settings + persistence |
| `voxygen/src/session/` | Session/connection handling |
| `voxygen/src/ecs/` | Client-side ECS systems (`ecs/sys/`) |

Rendering notes:
- Backend: **WGPU 27** (`wgpu = { version = "27", features = ["spirv", "glsl"] }`).
- Shaders live in `assets/voxygen/shaders/` as **GLSL** (`.glsl`, `.vert`, `.frag`),
  compiled to SPIR-V at runtime via **shaderc**. `shaderc-from-source` is a feature;
  system `libshaderc` can be used instead when `cmake` is unavailable.
- Shader names map to pipelines in `voxygen/src/render/pipelines/` + `renderer/shaders.rs`.
- `voxygen/src/singleplayer/mod.rs` starts an in-process `veloren-server` on a
  background thread at `TPS = 30`, using bounded/unbounded crossbeam channels.

Where to add client features:
- New visual effect → `voxygen/src/render/pipelines/` + shader in `assets/voxygen/shaders/`, register in `renderer/pipeline_creation.rs`.
- New HUD widget → `voxygen/src/hud/`.
- New menu/screen → `voxygen/src/menu/` and a `PlayState` implementation used by `run.rs`.
- New client-side ECS behavior → `voxygen/src/ecs/sys/` and register in `voxygen/src/ecs/`.

---

## 3. `veloren-world` — procedural voxel terrain & noise

Root: `world/` · crate: `veloren-world` · entry: `world/src/lib.rs`

Key modules:

| Path | Responsibility |
|---|---|
| `world/src/all.rs` | Overworld definition: biomes/`Environment`, `ForestKind`, trees |
| `world/src/block.rs` | `BlockGen` — block-level generation |
| `world/src/canvas.rs` | `Canvas`/`CanvasInfo` — per-chunk generation scratch space |
| `world/src/column.rs` | `ColumnGen`/`ColumnSample` — vertical column data (biome, temp, humidity, …) |
| `world/src/land.rs` | Land/continent shaping |
| `world/src/layer/` | Surface features: `cave.rs`, `rock.rs`, `scatter.rs`, `shrub.rs`, `spot.rs`, `tree.rs`, `wildlife.rs` |
| `world/src/sim/` | Macro simulation: `erosion.rs`, `diffusion.rs`, `map.rs`, `location.rs`, `way.rs` (rivers/lakes/orogens) |
| `world/src/sim2.rs` | Secondary simulation pass (economy) |
| `world/src/civ/` | Civilizations: `econ.rs`, `airship_route_map.rs`, `airship_travel.rs` |
| `world/src/site/` | Sites/settlements/dungeons: `generation.rs`, `genstat.rs`, `namegen.rs`, `tile.rs`, `economy/`, `plot/` |
| `world/src/util/` | Math/noise/structures: `fast_noise.rs` (`FastNoise`, `FastNoise2d`), `sampler.rs`, `random.rs`, `structure.rs`, `seed_expan.rs`, `wgrid.rs` |
| `world/src/config.rs` | `CONFIG` — sea level, mountain scale, temperatures, humidity, rainfall, … |
| `world/src/index.rs` | `Index`/`IndexOwned` — spatial index over the generated world |

Worldgen pipeline (high level):
1. `WorldSim` (`sim/mod.rs`) runs macro passes: uplift/erosion, river/lake drainage, biome assignment.
2. `Index` stores the coarse world map and site/civ placements.
3. On chunk request, `ChunkGenerator` (server) samples `World`/`ColumnGen`/`Canvas` and
   applies `layer::apply_*` passes (trees, rocks, caves, scatter, wildlife).
4. Results are `TerrainChunk` + `ChunkSupplement` sent to clients.

Where to add world features:
- New biome/terrain shape → `world/src/all.rs`, `world/src/column.rs`, `world/src/land.rs`.
- New surface decoration → `world/src/layer/` (add module + call from `layer/mod.rs`).
- New macro feature (river/lake/mountain) → `world/src/sim/`.
- New structure/settlement → `world/src/site/`.
- Tunables → `assets/world/` RON configs loaded through `world/src/config.rs`.

---

## 4. `veloren-server` — ECS game loop & networking

Root: `server/` · crate: `veloren-server` · entry: `server/src/lib.rs`

Key modules:

| Path | Responsibility |
|---|---|
| `server/src/lib.rs` | `Server` struct, `Server::new`, `Server::tick` (the authoritative game loop), `ServerInitStage` |
| `server/src/sys/` | Server ECS systems: `agent/`, `terrain.rs`, `terrain_sync.rs`, `entity_sync.rs`, `chunk_send.rs`, `chunk_serialize.rs`, `item.rs`, `loot.rs`, `object.rs`, `persistence.rs`, `pets.rs`, `sentinel.rs`, `subscription.rs`, `waypoint.rs`, `teleporter.rs`, `invite_timeout.rs`, `msg/`, `wiring.rs` |
| `server/src/sys/mod.rs` | `add_server_systems()` + `run_sync_systems()` — system registration order |
| `server/src/events/` | Server event handling: player, inventory, entity create/manipulate, group, trade, interact, mount, invite |
| `server/src/persistence/` | SQLite persistence, migrations, character load/save |
| `server/src/rtsim/` | Server-side RTSim integration |
| `server/src/chunk_generator.rs` | Async/parallel chunk generation (`ChunkGenerator`) |
| `server/src/connection_handler.rs` | Client connections + network message routing |
| `server/src/state_ext.rs`, `wiring.rs` | ECS `State` extensions and wiring |
| `server/src/metrics.rs` | Prometheus metrics |
| `server/src/settings/` | Server settings (`Settings`, `EditableSettings`, protocol/query config) |
| `server/src/weather.rs` | Weather simulation (worldgen feature) |
| `server/src/terrain_persistence.rs` | Persistent terrain (feature `persistent_world`) |

Loop architecture:
- ECS: **`specs` 0.20** (patched git rev) with custom `common-ecs` dispatch.
- Tick order (documented in `Server::tick`): collect input → timer events → client
  network messages → `LocalState` tick → terrain update queue → send state updates.
- Fixed `TPS = 30` (also used by singleplayer).
- Networking: `veloren-network` transport (QUIC/TCP/UDP) + `veloren-network-protocol`.
- Persistence: SQLite via `rusqlite` (bundled), migrations in `server/src/migrations/`.

Where to add server features:
- New per-tick gameplay logic → `server/src/sys/` (implement `common_ecs::System`) and
  register in `server/src/sys/mod.rs::add_server_systems`.
- New client/server event → `server/src/events/` + `common/src/event.rs` / `common/net`.
- New persistence field → `server/src/persistence/models.rs` + a migration.
- New network message → `network/protocol/src/message.rs` / `common/net/`.

---

## 5. `veloren-common` — items / stats / entities

Root: `common/` · crate: `veloren-common` · entry: `common/src/lib.rs`

Key areas:

| Path | Responsibility |
|---|---|
| `common/src/comp/mod.rs` | Component re-exports; central entity component surface |
| `common/src/comp/inventory/item/mod.rs` | **Items**: `Item`, `ItemKind` (`Tool`, `Armor`, `Consumable`, `ModularComponent`, `Lantern`, `Glider`, `Utility`, `Ingredient`, …), `ItemDefinition` |
| `common/src/comp/inventory/` | `slot.rs`, `loadout.rs`, `loadout_builder.rs`, `recipe_book.rs`, `trade_pricing.rs` |
| `common/src/comp/stats.rs` | **Stats**: `Stats`, `StatsModifier`, `StatsSplit`, damage/defense model |
| `common/src/comp/health.rs` | Health/damage components |
| `common/src/comp/ability.rs` | **Abilities**: `Ability`, `CharacterAbility`, `Stance`, active ability input |
| `common/src/comp/skillset/` | Skill trees / skill sets |
| `common/src/comp/body/` | Body plans (humanoid, biped, quadruped, bird, fish, dragon, golem, …) |
| `common/src/comp/agent.rs` | NPC AI (`Agent`, `Behavior`, `Alignment`, `PidController`) |
| `common/src/comp/buff.rs`, `aura.rs`, `beam.rs`, `arcing.rs`, `projectile.rs`, `shockwave.rs`, `melee.rs`, `pool.rs`, `poise.rs` | Combat/effect components |
| `common/src/comp/phys.rs` | Physics body/collider components |
| `common/src/comp/inventory/…` | Inventory + item definitions |
| `common/src/combat.rs` | `AttackEffect`, `StatEffect`, combat requirements |
| `common/src/event.rs`, `outcome.rs` | Gameplay events/outcomes |
| `common/src/resources.rs` | ECS resources (time, map, server config, …) |
| `common/src/recipe.rs`, `depot.rs`, `trade.rs` | Crafting/economy |
| `common/systems/` | Reusable systems: `character_behavior`, `controller`, `melee`, `projectile`, `phys`, `buff`, `aura`, `beam`, `arcing`, `shockwave`, `pool`, `stats`, `mount`, `tether`, `interpolation` |
| `common/src/terrain/` | Block/terrain types shared with `veloren-world` |
| `common/src/consts.rs` | Global constants (tick rate, physics, …) |

Where to add gameplay features:
- **New item** → `common/src/comp/inventory/item/mod.rs` (`ItemKind` variant) + RON in
  `assets/common/items/` (definitions) + i18n entries.
- **New stat/effect** → `common/src/comp/stats.rs` + `common/src/combat.rs` (`StatEffect`).
- **New ability** → `common/src/comp/ability.rs` + system in `common/systems/`.
- **New entity/component** → `common/src/comp/` + register in `common/state/` pools.
- **New combat behavior** → `common/systems/` and/or `server/src/sys/`.
- **New recipe/economy** → `common/src/recipe.rs`, `assets/common/recipes/`, `rtsim/`.

---

## 6. Assets layout (`assets/`)

Counts from this checkout:

| Category | Count |
|---|---|
| `.vox` voxel models | **4,752** |
| `.ron` configs/data | **2,855** |
| `.glsl` shaders (+ `.vert`/`.frag`) | **67+** |
| `.png` textures | 898 |
| `.ogg` audio | 639 |
| `.ftl` translations | 1,342 |

Top-level layout:
- `assets/common/` — items, voxel models (`voxel/`), recipes, figures, sprites, i18n.
- `assets/voxygen/` — shaders (`shaders/`), UI textures, fonts, sounds, icons.
- `assets/world/` — biome/site RON configs and world data.
- `assets/server/` — server-side data.
- `assets/test/` — test fixtures.

Assets are loaded via `veloren-common-assets` (`AssetExt`, `AssetHandle`) and are
hot-reloadable where the feature is enabled.

---

## 7. Build & run

Toolchain is pinned by `rust-toolchain` → `nightly-2026-06-13`.

```bash
cd /home/naitik/Projects/valocraft
export PATH="$HOME/.cargo/bin:$PATH"

# Client (default feature set; requires cmake + mold, or use the local workaround below)
cargo build --release --bin veloren-voxygen

# Local workaround when cmake/mold are unavailable:
cargo build --release --bin veloren-voxygen \
  --no-default-features \
  --features default-publish,hot-reloading,egui-ui,simd

# Dedicated server
cargo build --release --bin veloren-server-cli
```

Launch:
```bash
cd /home/naitik/Projects/valocraft

# Valocraft client: auto-boots into the integrated singleplayer server as the
# offline profile "Player". On first run it creates a world and character
# automatically (no login, server selection, or character-screen clicks).
./target/release/veloren-voxygen

# Dedicated server (separate terminal / host):
./target/release/veloren-server-cli
```

### Valocraft customizations

- Window title, crash dialog, version labels, and English UI copy rebranded to
  **Valocraft** (`voxygen/src/window.rs`, `voxygen/src/panic_handler.rs`,
  `assets/voxygen/i18n/en/`, HUD/char-select/main-menu version labels).
- Client auto-boot flow:
  - `voxygen/src/lib.rs` — `AUTO_BOOT_SINGLEPLAYER` switch.
  - `voxygen/src/menu/main/mod.rs` — `MainMenuState::enter` starts the integrated
    `SingleplayerState` immediately and switches the UI to the connecting screen.
  - `voxygen/src/menu/char_selection/mod.rs` — auto-creates/selects the offline
    character named `Player`.
  - `common/src/consts.rs` — `OFFLINE_PROFILE_NAME = "Player"`, shared with the
    server's admin UUID derivation so the local player keeps admin rights.
- No remote authentication is used for local play: the integrated server sets
  `auth_server_address: None` in `server::Settings::singleplayer`, and the client's
  singleplayer credential provider returns the offline username directly.
- Block-building roadmap: see [`VOXEL_BUILDING_PLAN.md`](VOXEL_BUILDING_PLAN.md).

Build & runtime status (verified 2026-10-26):
- **Assets:** the checkout originally contained 6,375 Git LFS pointer files (the canary
  asset correctly aborted the client). The full content was restored from the local
  Airshipper profile at
  `/home/naitik/.local/share/airshipper/profiles/default/assets`; every file was
  SHA-256-verified against its LFS pointer OID before copying. `assets/` is now 443 MB
  with **0 remaining pointers** and a valid `VELOREN_CANARY_MAGIC` canary.
- `target/release/veloren-voxygen` — 259 MB, built 20:06, `ldd` clean, `--help` OK.
  Built with the system-shaderc workaround feature set:
  `--no-default-features --features default-publish,hot-reloading,egui-ui,simd`.
- `target/release/veloren-server-cli` — 176 MB, rebuilt 20:19 after the offline-auth
  changes, `ldd` clean, `--help` OK.
- **Auto-boot runtime smoke test** (isolated profile
  `VELOREN_USERDATA=/tmp/opencode/valocraft-userdata`):
  - `New User username="Player"` — offline login, no credential/auth prompt.
  - `Pushed state 'Character Selection'.` → `Switching to state 'Session'` ~0.5 s later.
  - Character DB contains `Player`
    (`21d6bf65-d13c-64bf-6edb-1be01a5d15c0`).
  - No login screen, server-selection screen, or character-screen click was required.

Local notes / deviations:
- The repo's `.cargo/config.toml` forces `-fuse-ld=mold`; `mold` is not installed on
  this machine, so the local config now uses `lld` (`/usr/bin/ld.lld`).
- `cmake` is **not installed** system-wide; `voxygen`'s default
  `shaderc-from-source` feature needs it. The local build uses the installed system
  `shaderc` instead. A user-space CMake 4.4.4 is available at
  `~/.local/cmake-4.4.4-linux-x86_64/bin/cmake` for the stock default build:
  ```bash
  PATH="$HOME/.cargo/bin:$HOME/.local/cmake-4.4.4-linux-x86_64/bin:$PATH" \
    cargo build --release --bin veloren-voxygen
  ```
  Install `cmake` (`sudo pacman -S cmake`) and `mold` (`sudo pacman -S mold`) to
  restore the stock toolchain end-to-end.
