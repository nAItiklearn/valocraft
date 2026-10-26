# Mobile status and porting plan

## Short answer

Valocraft is currently a **desktop game**. There is no official Android or iOS
client, no touch-control layout, and no mobile release artifact. Turning the
desktop client into a playable mobile game is a port, not a packaging toggle, and
it cannot be completed by simply rebuilding the existing binary as an APK or IPA.

The desktop release pipeline is separate and works today. See
[`RUNNING.md`](RUNNING.md) for Windows, macOS, and Linux downloads and local
instructions.

---

## Why a rebuild is not enough

The upstream technology stack has mobile-capable pieces:

- `winit` 0.30 has Android and iOS backends.
- `wgpu` can render through Vulkan on Android and Metal on iOS.
- The game already has an offline singleplayer mode and a local server, so a
  mobile client could run the whole simulation on-device.

The game layer above those pieces is desktop-shaped:

| Area | Current behavior | What mobile needs |
|---|---|---|
| Entry point | `voxygen/src/main.rs` uses a normal desktop `main()` and a `winit` event loop | Android `android_main` / `android-activity` integration; iOS app lifecycle and an Xcode project |
| Input | Keyboard, mouse, and gamepad bindings in `voxygen/src/game_input.rs` and the HUD | Touch joystick, look area, attack/jump/interact buttons, menu gestures, and UI scaling |
| UI | Desktop menus and HUD designed for a mouse and large screen | Touch-first menus, larger hit targets, safe-area handling, on-screen controls |
| Assets | 443 MB in `assets/`, loaded from a directory beside the binary | Embed in the app or extract from the APK on first launch; manage storage and updates |
| User data | `common/base/src/userdata_dir.rs` uses compile-time workspace/system paths | Android internal storage; iOS Application Support directory |
| Rendering defaults | Desktop view distance, shadows, resolution, and effects | Mobile quality presets, thermal/power throttling, smaller render targets |
| Audio | Desktop `cpal` backend | Verify Android/iOS audio backend selection and lifecycle |
| Packaging | Desktop archives with `run.sh` / `run.bat` / `.app` | Signed APK/AAB, or a signed IPA and App Store/TestFlight distribution |
| Testing | Desktop GPU and window testing | Physical Android devices and iPhones, not just emulators |

The asset and input work alone is substantial. The game also needs a full
touch-control and menu pass; a Bluetooth keyboard and mouse is not a mobile
release strategy.

---

## Android plan

Android is the more realistic mobile target because users can install a signed
APK directly from a GitHub Release; Google Play is not required.

### Phase 1 — Build a headless prototype

1. Add a mobile feature set that excludes desktop-only integrations:
   `singleplayer,simd` (no Discord, native dialogs, or plugins at first).
2. Refactor `voxygen/src/main.rs` so the application body is callable from both
   `main()` and `android_main()`.
3. Add `android-activity` and a minimal `cargo-apk`/Gradle project.
4. Cross-compile for `aarch64-linux-android` with the Android NDK.
5. Prove that the client can create a window and render a frame on a device.

### Phase 2 — Storage and assets

1. Set `VELOREN_ASSETS` to an extracted asset directory in the app's internal
   storage, or teach the asset loader to read from Android assets.
2. Set `VELOREN_USERDATA` to the app's internal data directory before the first
   asset or settings access.
3. Add a first-run progress screen while assets are copied.
4. Validate SQLite, save files, logs, and the integrated server under Android's
   storage rules.

### Phase 3 — Touch controls

1. Add a virtual left joystick for movement and a right-side look area.
2. Add buttons for attack, block, jump, interact, inventory, map, and menu.
3. Map touch events into the existing `GameInput` state rather than adding a
   second gameplay path.
4. Add safe-area insets, landscape orientation, UI scaling, and a controller
   fallback.
5. Make menus usable without hover and with finger-sized targets.

### Phase 4 — Performance and power

1. Add a mobile quality preset: lower render distance, shadow quality, resolution
   scale, and effects.
2. Test on low-end, mid-range, and flagship devices; record frame time and
   thermal throttling.
3. Add pause/resume handling for Android lifecycle events.
4. Decide whether the integrated server should tick while the app is backgrounded
   (it should not, for power and save safety).

### Phase 5 — Release

1. Build a signed APK for `aarch64` (and optionally `x86_64` for emulators).
2. Attach it to GitHub Releases with install instructions and a clear
   "experimental" label.
3. Test install, update, save migration, and uninstall behavior.
4. Add crash reporting that does not require a desktop log viewer.

**Estimate:** several focused weeks for a functional prototype, plus device
testing and UI polish. It is not a one-session packaging change.

---

## iOS / iPhone plan

The same engine and renderer work is required, plus Apple-specific work:

1. Add an iOS app target and the `aarch64-apple-ios` Rust target. Winit's iOS
   backend and Metal rendering are the foundation.
2. Port the same touch controls, storage, lifecycle, and quality preset work.
3. Build and test in Xcode on macOS with a physical device.
4. Sign with an Apple Developer certificate and provisioning profile.
5. Distribute through TestFlight/App Store, or use ad-hoc/enterprise signing.

### The distribution blocker

Unlike Android, an iPhone will not install an arbitrary unsigned app from a
GitHub Release. A broadly installable build requires:

- an Apple Developer Program membership (paid annually),
- an Apple-issued signing certificate,
- a provisioning profile that includes the test devices or the App Store, and
- notarization/App Store review for normal distribution.

A free Apple ID can sign a build for a single device for a limited time, but that
is a developer workflow, not a "download and play" release. A GitHub Actions
macOS runner can compile an unsigned simulator build, but it cannot produce an
IPA that a normal iPhone will install.

**Conclusion:** an iPhone release is only possible after the engine/UI port and
with an Apple Developer account. We can prepare the project structure and
unsigned CI build before that, but we cannot honestly promise a downloadable
iPhone release today.

---

## What is already prepared for mobile work

- Offline singleplayer boot: no account server is required on the device.
- A local integrated server: the game does not need a remote server to play.
- A pinned Rust toolchain and reproducible desktop release build.
- A clear desktop release pipeline that can be reused for Android/iOS artifacts
  once the platform code exists.
- Asset and userdata environment overrides (`VELOREN_ASSETS`,
  `VELOREN_USERDATA`) that a mobile launcher can set before startup.

## Recommended order

1. Ship and verify the desktop release.
2. Build the Android headless/window prototype on one physical device.
3. Add touch controls and storage.
4. Add the mobile quality preset and lifecycle handling.
5. Only then start the iOS port and Apple signing work.

If you want to proceed, the next concrete step is an Android prototype branch
with `android-activity`, NDK cross-compilation, and a debug APK. It should be
developed and tested separately from the desktop release branch so desktop
downloads remain stable.
