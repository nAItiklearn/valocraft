# Technical decisions

Short records of decisions that shape Valocraft's code. Newest first.

## TD-001: Survival building

**Context.** Veloren already has a server-authoritative block edit pipeline
(`ClientGeneral::BreakBlock` / `PlaceBlock`, `BlockChange`, terrain sync) but
only for admins inside creative build areas (`CanBuild`). Survival building
needs items, rules and feedback on top of it.

**Decisions.**

- **No new network messages.** The client sends the existing
  `BreakBlock`/`PlaceBlock`. `server/src/sys/msg/in_game.rs` routes edits in a
  permitted creative build area to the old creative path (which keeps
  precedence). Everything else becomes a `SurvivalBuildEvent` if the server
  enables survival building, the client controls a character and it is not in
  creative build mode (`/build`). Creative edits outside a permitted area are
  ignored, as they were before survival building.
- **One handler owns the rules:** `server/src/events/building.rs` checks, in
  order: the editor is alive → reach (eye to block centre ≤
  `SURVIVAL_BUILD_REACH` + `SURVIVAL_BUILD_REACH_TOLERANCE`) → site protection
  → per-action rules. Each rejection sends
  `InventoryUpdateEvent::BuildFailed { pos, reason }` to the editor only.
- **`BuildingBlock` is the single source of truth**
  (`common/src/comp/inventory/item/building.rs`): each variant has one
  canonical voxel (kind + colour) and one item definition. Placed blocks are
  matched exactly and break back into the same item; natural terrain is
  quantised by `BlockKind` so inventories stack. A unit test checks every
  variant has a matching `ItemKind::Block` asset.
- **Sprites are never collateral damage unless they are plain decoration.**
  A sprite may only be destroyed by an edit if it has no collision, can't be
  collected, mined, opened, sat or slept on, isn't a door or controller, and
  can be destroyed by explosions (`is_disposable_sprite`). Anything else
  rejects the edit as obstructed.
- **Breaking** needs a gatherable block and, for rock and ice, a pickaxe in
  any hand slot. A disposable sprite on top is removed with the block, and
  that removal is persisted too (the generic unsupported-sprite cleanup isn't
  persisted). If the inventory is full, the item drops into the world, owned
  by the editor for a short time.
- **Placing** needs a fluid target (air or water) whose sprite is disposable
  and that doesn't overlap a living creature, and a matching item, which is
  consumed.
- **Protection** covers every creative build area (whoever may build there,
  only creative edits are allowed) except the reserved `world` area, which
  the server creates over the whole map so that admins can be permitted to
  build anywhere, and every site tile that holds something
  the site built (buildings, walls, roads, plazas, fields, bridges, ...) plus
  a one-tile margin (requires the `worldgen` feature). Positions outside the
  world are protected. Sites also tag the natural terrain up to ~576 blocks
  around them as `Hazard` (slopes, water) or `Path` (overland trails) for
  their own planning. Those tiles are deliberately *not* protected: an early
  version protected every non-empty tile, and the end-to-end check found
  almost no buildable ground left near towns.
- **Persistence is required.** The effective flag is
  `gameplay.survival_building && persistent_world && experimental_terrain_persistence`.
  Singleplayer always enables terrain persistence. The effective flag is
  advertised to clients in `ServerConstants::survival_building`.
- **Client.** Build mode is a client-local toggle (`GameInput::ToggleBuildMode`,
  `Z` by default), active only while the server allows it and creative
  building isn't. Targeting uses the same eye-to-centre reach as the server,
  checked against the voxel that is actually edited (the voxel in front of
  the targeted block when placing). The survival targeting ray stops at
  sprites that have their own interaction, so ores are still mined and
  collectibles still collected in build mode. The client never sends an edit
  it knows will fail (no item, out of reach), but the server re-checks
  everything.
- **Feedback.** Every failure is shown in the build HUD. Placing plays a
  material-specific sound (soft, wood, stone), carried by
  `Outcome::PlaceBlock { block }`; the sounds are CC0 (Kenney) mixes made by
  `scripts/gen_building_sounds.py`.

**Consequences / known limitations.**

- `ServerConstants` gained a field, so old clients and servers are
  incompatible. That is acceptable: Valocraft ships both together.
- There is no server-side rate limit beyond one edit per message; a modified
  client can edit as fast as it can send. Edits are still bound by reach,
  protection and items.
- Creative build areas override survival rules inside them, and are fully
  protected from survival edits.
- Only the sprite directly on top of a gathered block is removed and
  persisted. Other sprites that lose their support (hanging or wall-mounted
  ones) are removed by the generic cleanup, which isn't persisted, so they can
  come back after the chunk is reloaded.
- In build mode the roll key picks the targeted block, so there is no dodge
  roll while looking at terrain within reach.
- Placement doesn't require an adjacent solid block; a modified client could
  place floating blocks within reach.

**Verification.** `cargo test -p veloren-common building`, plus the end-to-end
check `client/examples/survival_build_check.rs`, which drives a headless client
against a real server and verifies every rule above and persistence across a
restart. `scripts/survival_build_e2e.sh` runs it, restart included (Unix only:
the graceful restart that saves the terrain uses SIGUSR1). The check clears
NPCs around its test spot first: a wildlife attack once knocked the character
out of reach and killed it half way through.
