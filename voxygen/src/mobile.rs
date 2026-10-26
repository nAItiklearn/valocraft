//! Android-specific bootstrap.
//!
//! Android packages the game as a native shared library and launches it
//! through `android_main`.  The APK's asset manager is read-only, so on first
//! launch we copy the bundled game data into the app's internal storage and
//! point the normal userdata/asset environment variables at that copy.

use std::{
    ffi::{CString, OsString},
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use ndk::asset::AssetManager;
use winit::platform::android::activity::AndroidApp;

// cargo-apk places the contents of the configured assets directory at the
// APK's asset root, so the repository `assets/` directory has no extra
// `assets/` component at runtime.
const APK_ASSETS_PREFIX: &str = "";
const ASSET_CANARY: &str = "common/canary.canary";
// Fallback for AssetManager implementations that refuse to open the empty
// asset-relative path as a directory.  Keep this in sync with the top-level
// entries in the repository's assets/ directory.
const ASSET_ROOTS: &[&str] = &[
    "common",
    "credits.ron",
    "server",
    "test",
    "voxygen",
    "world",
];

/// Prepare writable asset and userdata directories and install the paths in the
/// process environment before the game's normal startup path runs.
#[allow(unsafe_code)]
pub fn prepare(app: &AndroidApp) {
    let data_dir = app
        .internal_data_path()
        .or_else(|| app.external_data_path())
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/valocraft"));

    let assets_dir = data_dir.join("assets");
    let userdata_dir = data_dir.join("userdata");

    if let Err(error) = fs::create_dir_all(&userdata_dir) {
        panic!("failed to create Android userdata directory: {error}");
    }

    if !assets_dir.join(ASSET_CANARY).is_file() {
        eprintln!("Valocraft: extracting bundled assets (first launch only)");
        let manager = app.asset_manager();
        if let Err(error) = copy_asset_tree(&manager, APK_ASSETS_PREFIX, &assets_dir, &mut 0) {
            panic!("failed to extract Android game assets: {error}");
        }
        eprintln!("Valocraft: asset extraction complete");
    }

    // `set_var` is unsafe in edition 2024 because another thread could be
    // reading the environment.  Android calls us before the game starts any
    // worker threads, so this is safe here.
    unsafe {
        std::env::set_var("VELOREN_ASSETS", &assets_dir);
        std::env::set_var("VELOREN_USERDATA", &userdata_dir);
        std::env::set_var("VELOREN_USERDATA_STRATEGY", "executable");
    }

    eprintln!(
        "Valocraft: assets={} userdata={}",
        assets_dir.display(),
        userdata_dir.display()
    );
}

fn copy_asset_tree(
    manager: &AssetManager,
    source: &str,
    destination: &Path,
    copied: &mut usize,
) -> io::Result<()> {
    let source_c = CString::new(source)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "asset path contains NUL"))?;

    if let Some(directory) = manager.open_dir(&source_c) {
        let names: Vec<OsString> = directory
            .map(|name| OsString::from(name.to_string_lossy().into_owned()))
            .collect();
        if !names.is_empty() {
            fs::create_dir_all(destination)?;
            for name in names {
                let child_source = if source.is_empty() {
                    name.to_string_lossy().into_owned()
                } else {
                    format!("{source}/{}", name.to_string_lossy())
                };
                copy_asset_tree(manager, &child_source, &destination.join(name), copied)?;
            }
            return Ok(());
        }
        if !source.is_empty() {
            fs::create_dir_all(destination)?;
            return Ok(());
        }
    }

    if source.is_empty() {
        // Some Android versions/asset managers do not expose the empty path as
        // a directory; enumerate the known roots explicitly instead.
        for name in ASSET_ROOTS {
            copy_asset_tree(manager, name, &destination.join(name), copied)?;
        }
        return Ok(());
    }

    let mut asset = manager.open(&source_c).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing APK asset: {source}"),
        )
    })?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = File::create(destination)?;
    io::copy(&mut asset, &mut output)?;
    *copied += 1;
    if *copied % 500 == 0 {
        eprintln!("Valocraft: extracted {copied} asset files");
    }
    Ok(())
}
