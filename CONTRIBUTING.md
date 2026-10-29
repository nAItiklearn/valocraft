# Contributing to Valocraft

Thanks for your interest in improving Valocraft! This document explains how to
get started.

## Getting oriented

- [README.md](README.md) — what the game is and how to run it
- [RUNNING.md](RUNNING.md) — detailed build and run instructions
- [CODEBASE_INDEX.md](CODEBASE_INDEX.md) — a map of the workspace crates
- [RELEASING.md](RELEASING.md) — how releases are built and published

## How to contribute

1. Fork the repository on GitHub and create a branch for your change.
2. Make your change. Keep it focused — one feature or fix per pull request.
3. Before submitting:
   - `cargo fmt` your code.
   - Make sure `cargo clippy` is clean for the crates you touched.
   - Make sure the project builds: `cargo check --release` (see RUNNING.md for
     the full feature flags used for release builds).
   - Add or update tests when you change behavior.
4. Open a pull request against `main` with a clear description of what and why.

## Guidelines

### Licensing

Valocraft is licensed under GPLv3. By contributing, you agree that your
contributions are distributed under the same license. Only commit content
(code, art, audio, translations) whose license is compatible with GPLv3, and
keep existing copyright and attribution notices intact.

### Style

- Follow the existing code style of the crate you are editing.
- Prefer small, readable changes over large rewrites.
- Write comments where the *why* is not obvious from the code.

### Reporting bugs

Open an issue at https://github.com/nAItiklearn/valocraft/issues with:

- What you did, what you expected, and what happened.
- Your platform (Windows/macOS/Linux) and GPU.
- Relevant logs from `userdata/voxygen/logs/` if the game crashed.
