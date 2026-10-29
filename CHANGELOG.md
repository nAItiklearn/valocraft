# Changelog

All notable changes to Valocraft are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.3] - 2026-10-29

### Added

- Emberwatch contract system: the Hearthpost Ember Board offers Survey,
  Bounty, and Courier contracts, tracked in the new Field Journal (`U` key),
  with map/minimap objective markers, coin and standing rewards, and an
  Emberwatch Beacon bonus after three completions.
- Built-in FPS/frame-time readout at the top-centre of the HUD.
- Official Valocraft website (static, deployed to GitHub Pages from
  `website/`): home, downloads, game guide, license, and privacy pages.

### Changed

- Players now take 50% reduced incoming damage, making early-game combat
  more forgiving.
- Multiplayer has no central account service: servers are account-free by
  default, and server owners can run their own self-hosted authentication.
  The client no longer ships with a default public server or trusted auth
  server.
- The account/auth client (`authc`) and a self-hostable auth server are now
  vendored in `libs/auth/` — building Valocraft no longer depends on any
  external project infrastructure.
- Crash reporting, `/wiki` command, and in-game notices now point to
  Valocraft resources.
- Release workflow publishes `SHA256SUMS.txt` and per-platform download
  tables, marks tag releases as latest, and removes stale non-desktop
  releases.

### Removed

- Veloren logo and in-game Credits screen (audio/art attribution data remains
  shipped with the game for license compliance).
- Android/iOS ports and all mobile documentation; Valocraft is desktop-only
  (Windows, macOS, Linux).

## [0.1.2] - 2026-10-26

- First public Valocraft desktop release for Windows, macOS, and Linux.

---

Valocraft is derived from the GPLv3-licensed Veloren project; upstream change
history prior to the Valocraft fork is preserved in git history.
