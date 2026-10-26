# Running Valocraft

Valocraft runs on 64-bit Windows, macOS, and Linux. The packaged client boots
straight into an offline singleplayer world as the profile `Player`; no account,
login, or server setup is required.

The release page is:

<https://github.com/nAItiklearn/valocraft/releases>

Download the archive for your platform, extract it completely, and use the
launcher inside it. Do not run the client directly from inside the `.zip` or
`.tar.gz` — it needs the `assets/` folder next to it.

> **Mobile (experimental):** An arm64 Android APK is available on the
> [Android release](https://github.com/nAItiklearn/valocraft/releases/tag/android-v0.1.2).
> It is debug-signed for sideloading, uses touch controls, and still needs testing
> on physical devices. iPhone/iPad builds are not available; Apple signing and a
> Mac are required. See [`MOBILE.md`](MOBILE.md).

---

## What is in the download?

| File | Purpose |
|---|---|
| `Valocraft` / `Valocraft.exe` | Game client |
| `ValocraftServer` / `ValocraftServer.exe` | Optional dedicated server |
| `assets/` | Game data, models, textures, sounds, translations |
| `userdata/` | Created automatically: settings, logs, saves, server database |
| `run.sh` / `run.bat` | Launcher that sets up the portable asset and save paths |
| `README.md`, `RUNNING.md`, `LICENSE` | Documentation and license |

The client is portable by default: its saves and settings live in the `userdata/`
folder beside the binary. You can move the whole folder to another location or
another computer.

---

## Windows 10 / 11

1. Download `Valocraft-v*-windows-x86_64.zip`.
2. Right-click the zip and choose **Extract All**. Extract it to a normal folder,
   for example `C:\Games\Valocraft`. Do not run it from the Downloads preview.
3. Open the extracted folder and double-click `run.bat` (or `Valocraft.exe`).
4. If Windows SmartScreen appears, choose **More info → Run anyway**. The build is
   not code-signed yet.
5. If Windows reports a missing `VCRUNTIME140.dll` or similar file, install the
   [Microsoft Visual C++ Redistributable (x64)](https://learn.microsoft.com/cpp/windows/latest-supported-vc-redist)
   and try again.
6. A Vulkan, DirectX 12, or OpenGL-capable GPU driver is required. Update your
   graphics driver if the game cannot find a rendering device.

The first launch creates `userdata\voxygen\` and an offline world. Later launches
go straight into that world.

To run a dedicated server, double-click `run-server.bat`. The default game port is
14004. Server settings and saves are created under `userdata\server\`.

---

## macOS

1. Download the build for your Mac:
   - **Apple Silicon** (M1/M2/M3/M4): `Valocraft-v*-macos-aarch64.dmg` or
     `.tar.gz`.
   - **Intel**: `Valocraft-v*-macos-x86_64.dmg` or `.tar.gz` (when published).
2. Open the `.dmg` and drag **Valocraft** to **Applications**, or extract the
   `.tar.gz` and move `Valocraft.app` to Applications.
3. The app is not notarized yet. On first launch, **right-click the app → Open**
   and confirm **Open**. If macOS says the app is damaged or cannot be opened,
   remove the quarantine flag:

   ```bash
   xattr -dr com.apple.quarantine /Applications/Valocraft.app
   open /Applications/Valocraft.app
   ```

4. The game needs Metal. macOS 10.15 or newer and a Metal-capable GPU are
   required; Apple Silicon and most 2012-or-newer Macs are supported.

Saves and settings are stored in:

```text
~/Library/Application Support/Valocraft/userdata/
```

The optional server launcher is `Run Valocraft Server.command` in the extracted
folder. It uses the same assets and a separate server data directory.

> Unsigned macOS builds cannot be distributed through the App Store or opened
> without a Gatekeeper confirmation. Proper signing and notarization require an
> Apple Developer account.

---

## Linux

1. Download `Valocraft-v*-linux-x86_64.tar.gz`.
   The release is built on Ubuntu 22.04 and targets glibc 2.35 or newer; very old
   distributions may need a source build.
2. Extract it:

   ```bash
   tar -xzf Valocraft-v*-linux-x86_64.tar.gz
   cd Valocraft-v*-linux-x86_64
   ```

3. Run the launcher:

   ```bash
   ./run.sh
   ```

   If the archive was extracted without executable permissions, restore them:

   ```bash
   chmod +x Valocraft ValocraftServer run.sh run-server.sh
   ```

4. Install the runtime libraries if your distribution does not already have
   them. On Ubuntu/Debian:

   ```bash
   sudo apt update
   sudo apt install -y \
     libasound2 libudev1 libx11-6 libxcursor1 libxrandr2 libxi6 \
     libxkbcommon0 libwayland-client0 libvulkan1 mesa-vulkan-drivers \
     libgl1 libegl1
   ```

   On Fedora:

   ```bash
   sudo dnf install alsa-lib systemd-libs libX11 libXcursor libXrandr \
     libXi libxkbcommon wayland vulkan-loader mesa-vulkan-drivers \
     mesa-libGL mesa-libEGL
   ```

   On Arch/Manjaro:

   ```bash
   sudo pacman -S alsa-lib systemd-libs libx11 libxcursor libxrandr libxi \
     libxkbcommon wayland vulkan-icd-loader mesa
   ```

   NVIDIA users should use the proprietary NVIDIA driver; AMD/Intel users should
   install Mesa's Vulkan driver. Verify Vulkan with `vulkaninfo` if available.

5. If the client starts but cannot create a rendering device, try the OpenGL
   backend:

   ```bash
   WGPU_BACKEND=gl ./run.sh
   ```

The launcher keeps saves and settings in the extracted folder's `userdata/`
directory. To use a different location:

```bash
VELOREN_USERDATA="$HOME/.local/share/valocraft" ./run.sh
```

Run the dedicated server with:

```bash
./run-server.sh
```

The default port is 14004. The server writes its settings and saves under
`userdata/server/`.

---

## Build from source

These instructions build the same client and server as the release archives.

### Common prerequisites

- Rust: install with [rustup](https://rustup.rs/). The checkout pins its required
  nightly in `rust-toolchain`; rustup installs it automatically.
- Git and Git LFS. The game assets are stored in Git LFS.
- A 64-bit toolchain, CMake, Ninja, and a C/C++ compiler.
- At least 4 GB RAM, a Vulkan/DirectX 12/Metal-capable GPU, and about 2 GB of
  free disk space for the game.

```bash
git lfs install
git lfs pull
```

The repository's `.cargo/config.toml` uses `lld` on Linux. Install `lld` as well,
or remove the `-fuse-ld=lld` linker flag if your toolchain cannot find it.

### Linux

Install the build dependencies (Ubuntu/Debian example):

```bash
sudo apt update
sudo apt install -y build-essential pkg-config clang lld cmake ninja-build \
  libasound2-dev libudev-dev libx11-dev libxcursor-dev libxrandr-dev \
  libxi-dev libxkbcommon-dev libwayland-dev libvulkan-dev \
  libgl1-mesa-dev libegl1-mesa-dev libssl-dev libsqlite3-dev
```

Build:

```bash
cargo build --release --bin veloren-voxygen --bin veloren-server-cli
```

The default `shaderc-from-source` feature compiles the shader compiler with
CMake. If CMake is unavailable but your system has `libshaderc` and
`libshaderc_combined.a`, this checkout has been verified with:

```bash
cargo build --release --bin veloren-voxygen \
  --no-default-features \
  --features default-publish,hot-reloading,egui-ui,simd
```

Run from the repository root:

```bash
./target/release/veloren-voxygen
./target/release/veloren-server-cli
```

### macOS

Install the Xcode command-line tools, then:

```bash
xcode-select --install
brew install cmake ninja git-lfs
```

Build and run:

```bash
cargo build --release --bin veloren-voxygen --bin veloren-server-cli
./target/release/veloren-voxygen
```

The native target is Apple Silicon on M-series Macs. To build an Intel binary on
an Apple Silicon Mac is not supported by this checkout's packaging script; use an
Intel machine or a CI runner for Intel releases.

### Windows

1. Install **Visual Studio Build Tools** with the **Desktop development with C++**
   workload (MSVC and the Windows SDK).
2. Install [CMake](https://cmake.org/download/), [Ninja](https://ninja-build.org/),
   [Rust](https://rustup.rs/), Git, and Git LFS.
3. Open **x64 Native Tools Command Prompt for VS** (or a PowerShell window with
   the same environment), then:

   ```powershell
   git lfs install
   git lfs pull
   cargo build --release --bin veloren-voxygen --bin veloren-server-cli
   .\target\release\veloren-voxygen.exe
   ```

   If the build cannot find `shaderc`, confirm that `cmake` and `ninja` are on
   `PATH` and that you are in the x64 developer environment.

### Build features and the pinned toolchain

The release workflow uses the `release-thinlto` profile. The client uses
`default-publish` plus `shaderc-from-source`; the dedicated server keeps its
default features:

```bash
cargo build --profile release-thinlto --bin veloren-voxygen \
  --no-default-features --features default-publish,shaderc-from-source

cargo build --profile release-thinlto --bin veloren-server-cli
```

The exact flags used for this checkout's verified local Linux build are recorded
in [`CODEBASE_INDEX.md`](CODEBASE_INDEX.md).

---

## Controls

| Action | Default |
|---|---|
| Move | `W` `A` `S` `D` |
| Look | Mouse |
| Attack / use | Left mouse |
| Secondary / block | Right mouse |
| Jump | `Space` |
| Sneak | `Shift` |
| Glide | `Ctrl` |
| Interact | `E` |
| Inventory | `I` |
| Crafting | `C` |
| Map | `M` |
| Mount | `F` |
| Chat | `Enter` |
| Menu | `Esc` |
| Settings | `F10` |
| Fullscreen | `F11` |

Bindings can be changed in **Settings → Controls**.

---

## Troubleshooting

### “Game assets are missing” or the client exits immediately

The archive was not extracted completely, or only the executable was copied.
Extract the full release archive so that `assets/common/canary.canary` exists next
to the launcher.

For a source checkout, the file may still be a Git LFS pointer:

```bash
git lfs install
git lfs pull
```

### “No compatible rendering backend” / black window

Update your GPU driver and install the Vulkan loader and driver for your GPU.
On Linux, check `vulkaninfo`; on Windows, update the graphics driver from AMD,
Intel, or NVIDIA. Try `WGPU_BACKEND=gl` on Linux.

### Audio is missing

Check that the system audio libraries are installed (`libasound2` on
Ubuntu/Debian, `alsa-lib` on Fedora/Arch). The game creates logs under
`userdata/voxygen/logs/`; the latest log usually names the failing device.

### Where are my saves?

- Windows/Linux release folder: `userdata/`
- macOS: `~/Library/Application Support/Valocraft/userdata/`
- Source checkout: `userdata/` in the repository root, unless
  `VELOREN_USERDATA` is set.

Back up the whole `userdata/` folder to preserve worlds, characters, settings,
and server data.

### The client opened a login or multiplayer screen

The official Valocraft snapshot auto-boots into offline singleplayer. If you are
running a build from another branch or you logged out, use the main menu to start
singleplayer or connect to a server. To restore the automatic boot, set
`AUTO_BOOT_SINGLEPLAYER` to `true` in `voxygen/src/lib.rs` and rebuild.

### macOS says the app is damaged

The build is unsigned. Remove the quarantine attribute:

```bash
xattr -cr /Applications/Valocraft.app
```

Then right-click the app and choose **Open**. If your organization blocks
unsigned software, you will need a signed and notarized build.

---

## Dedicated server notes

- Default port: **14004**.
- The server creates its configuration and saves under `userdata/server/`.
- Local/private servers can run without remote authentication; public servers may
  require an account.
- The client's first launch goes to singleplayer. Log out to the main menu to use
  the multiplayer server browser.
- For a server on another machine, allow the game port through the firewall and
  connect using the server's address.

---

## More documentation

- [`README.md`](README.md) — features, activities, and controls
- [`MOBILE.md`](MOBILE.md) — Android/iOS status and porting plan
- [`RELEASING.md`](RELEASING.md) — maintainer release workflow
- [`VOXEL_BUILDING_PLAN.md`](VOXEL_BUILDING_PLAN.md) — block building roadmap
- [`CODEBASE_INDEX.md`](CODEBASE_INDEX.md) — developer map and build notes
