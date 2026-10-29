# Mechanics research: building RPGs

Valocraft takes broad inspiration from Dragon Quest Builders 1/2 (DQB) and
Hytale. This note records which of their *ideas* we adopt, adapt or reject.
Everything here comes from public footage, reviews, official blog posts and
developer commentary. No code, models, textures, sounds or text from either
game are used, and none may be added. Valocraft's own assets are original or
permissively licensed (see `assets/credits.ron`).

## Comparison

| Mechanic | Dragon Quest Builders | Hytale (public previews) | Veloren baseline | Valocraft decision |
|---|---|---|---|---|
| Gathering terrain | Hit blocks with a weapon/hammer, blocks drop as materials | Survival mining with tools, creative-mode editing | Only ores and sprites can be mined; terrain is immutable outside admin build areas | **Done:** survival build mode turns any natural block into a stackable block item |
| Tool gating | Better hammers break harder blocks | Tool tiers | Pick needed for ores | **Done:** rock and ice need a pickaxe; soil, sand, snow, wood and leaves can be dug by hand |
| Placing | Place the selected block on any face | Place with preview, rotation and variants | Creative only (`/permit_build`) | **Done:** place the selected block from the inventory, consumes one item |
| Block variety | Many decorative blocks, crafted from basics | Large palette, connected/rotated variants | Arbitrary colours per voxel | **Done (v1):** 11 canonical blocks, each a fixed voxel, so placed blocks always round-trip into the same item |
| Crafting blocks | Workbench recipes turn raw materials into bricks, planks, ... | Crafting benches | Recipe book and crafting stations exist | **Done:** planks, stone bricks and clay bricks |
| Protected areas | Story areas are fixed | Server-side permissions | Sites are generated, not editable | **Done:** generated sites (towns, dungeons, ...) are protected |
| Persistence | Saved per island | Saved world | Experimental terrain persistence | **Done:** survival building requires terrain persistence; singleplayer always enables it |
| Room recognition | Enclosed rooms with a door and a light become "rooms" that NPCs use | Not a core mechanic in previews | None | **Next candidate:** recognise enclosed player-built rooms and let settlers use them |
| Villagers / town building | Villagers move in, request facilities, help build | NPC factions | rtsim NPCs with jobs and sites | Planned: settlers that react to player-built rooms |
| Blueprints | Blueprints show where to build | Prefab tooling for creators | None | Later: blueprint ghosts as a building guide |
| Creative tools | Limited | Brushes, selections, prefabs | Admin-only build areas | Kept as-is: creative build areas take precedence over survival rules |
| Combat | Action combat, simple | Action combat with weapons | Rich ability-based combat | Keep Veloren's combat; tune encounters for the RPG loop |

## What makes these loops feel good

- **Every hit gives something.** Breaking a block immediately adds a visible
  item. Valocraft merges repeated pickups in the loot feed so mass digging
  doesn't spam the screen.
- **The world is the material.** Players don't need a separate resource
  layer: terrain *is* the inventory.
- **Clear feedback on failure.** A rejected edit always tells you why
  (protected, out of reach, needs a pickaxe, something in the way, none
  left).
- **Reach limits make building physical.** You build where you stand; this
  is what separates survival building from creative mode.
- **Crafted blocks are a reward.** Turning dirt and sand into clay bricks is
  an early, cheap upgrade that makes a house look "built" rather than dug.

## Rejected ideas

- **Ripping or recreating proprietary art.** Valocraft must stay original.
  We can share genre conventions (chunky voxel characters, bright palettes)
  but not specific characters, monsters, logos or textures.
- **Unlimited free-form colours in survival.** Exact-colour items would make
  inventories unstackable. Natural terrain is quantised by material instead.
- **Instant creative-style editing anywhere.** It would trivialise the
  dungeon and town content that Veloren already generates.
