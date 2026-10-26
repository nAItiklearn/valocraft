# Valocraft

**A voxel open-world action-adventure RPG.**

Valocraft is a fantasy adventure set in a vast, procedurally generated world. Explore
far-off lands, fight dangerous creatures, master weapons and magic, craft powerful
gear, and leave your mark on the world — alone offline or with friends on a dedicated
server.

> **Status:** alpha / active development. Expect bugs, unfinished systems, and
> changing balance.

---

## Features

### Explore a living voxel world
- Procedurally generated continents with forests, grasslands, deserts, mountains,
  snowfields, oceans, rivers, lakes, and caves.
- Towns, villages, dungeons, ruins, camps, landmarks, and hidden points of interest.
- A dynamic day/night cycle, weather, and a full world map with markers.
- Climb, swim, glide, and travel on tamed mounts.

### Fight in real time
- Melee combat with swords, axes, hammers, and shields.
- Ranged combat with bows and thrown weapons.
- Magic with staffs and spells.
- Block, dodge, roll, and use weapon abilities.
- Fight wildlife, hostile factions, monsters, and dungeon bosses.

### Build a character
- Choose from multiple playable races and customize your appearance.
- Earn XP, level up, and unlock skills.
- Equip weapons, armor, bags, gliders, lanterns, and modular gear.
- Find loot, improve item quality, and tailor your build.

### Craft, cook, and trade
- Craft at workbenches and specialized crafting stations.
- Make weapons, armor, tools, gliders, lanterns, and bags.
- Cook food and brew potions.
- Trade with NPCs using in-game currency.
- Grow your recipe book as you discover new recipes.
- Craftable building blocks and workbench recipes are part of the building roadmap.

### Build and shape the world
- Break and place blocks in creative build areas.
- Full survival-style block building, block items, and workbench recipes are in
  development. See [`VOXEL_BUILDING_PLAN.md`](VOXEL_BUILDING_PLAN.md).

### Play solo or together
- **Offline singleplayer** with automatic boot: launch the client and you are in the
  world — no account, login form, or server selection.
- Dedicated multiplayer servers.
- Optional account authentication for public servers.
- Server-side plugins and configurable game rules.

### Quality of life
- Configurable keyboard, mouse, and gamepad bindings.
- Adjustable graphics, audio, UI scale, and camera options.
- Localization support.
- Original soundtrack and ambient audio.
- Screenshots, map markers, and a full in-game diary.

---

## What you can do

- Explore continents, caves, ruins, and settlements.
- Hunt wildlife and fight bandits, monsters, and bosses.
- Delve into dungeons for loot and treasure.
- Level up and unlock new abilities.
- Craft and upgrade weapons, armor, and tools.
- Cook food and brew potions.
- Trade with merchants and NPCs.
- Tame and ride mounts.
- Glide from cliffs and mountains.
- Build and decorate in build areas.
- Play offline singleplayer or join a dedicated server.
- Team up with other players, chat, and trade.

---

## Controls

| Action | Default |
|---|---|
| Move | `W` `A` `S` `D` |
| Look | Mouse |
| Attack / use | Left mouse |
| Secondary / block | Right mouse |
| Block | `Alt` |
| Jump | `Space` |
| Sneak | `Shift` |
| Glide | `Ctrl` |
| Interact | `E` |
| Inventory | `I` |
| Crafting | `C` |
| Map | `M` |
| Mount | `F` |
| Lantern | `G` |
| Chat | `Enter` |
| Command | `/` |
| Menu | `Esc` |
| Controls | `F1` |
| Settings | `F10` |
| Fullscreen | `F11` |
| Screenshot | `F4` |
| Toggle UI | `F6` |

All bindings can be changed in **Settings → Controls**.

---

## Getting started

### Play

1. Launch the client executable from the build output directory (`target/release/`);
   it is the executable that opens the game window.
2. The client boots directly into a local offline singleplayer world as the profile
   `Player`.
3. On first launch, a world and character are created automatically.

No account, internet connection, or server setup is required for local play.

### Multiplayer

Start a dedicated server, then connect from the client's multiplayer menu. The first
launch auto-boots into singleplayer; log out to the main menu to use the multiplayer
browser. Public servers may require an account; local and private servers can run
without authentication.

---

## Build from source

Requirements:
- Rust (the pinned toolchain is declared in `rust-toolchain`)
- Git LFS (large game assets are stored with Git LFS)
- A supported graphics backend (Vulkan, OpenGL, or Metal)
- Platform build tools for your OS (this checkout is verified on Linux)

```bash
git lfs install
git lfs pull
cargo build --release
```

If you cloned before installing Git LFS, run `git lfs pull` to fetch the assets.
Launch the generated client executable from `target/release/`. For the exact
feature flags and local toolchain workarounds used in this checkout, see
[`CODEBASE_INDEX.md`](CODEBASE_INDEX.md).

---

## Roadmap

- Survival-style block building: block items, drops, placement, and workbench
  recipes. See [`VOXEL_BUILDING_PLAN.md`](VOXEL_BUILDING_PLAN.md).
- More adventure content, dungeons, quests, and world events.
- Continued combat, crafting, and progression balance.
- Further rebranding, UI polish, and packaging work.

---

## Project status

Valocraft is in active development. Systems may change, saves may be incompatible
between versions, and bugs are expected. Feedback and contributions are welcome.

## License

Valocraft is open-source software. See [`LICENSE`](LICENSE) for the full license text.
