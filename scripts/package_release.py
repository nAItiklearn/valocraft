#!/usr/bin/env python3
"""Package a built Valocraft client for a desktop release.

The resulting archive is self-contained: it contains the client, the optional
dedicated server, the game assets, a launcher, and the player documentation.
"""

from __future__ import annotations

import argparse
import os
import plistlib
import re
import shutil
import subprocess
import sys
import tarfile
import zipfile
from pathlib import Path
from typing import NoReturn

REPO_ROOT = Path(__file__).resolve().parent.parent
DOC_FILES = ("README.md", "RUNNING.md", "LICENSE")
CANARY = Path("common") / "canary.canary"
CLIENT_BINARIES = {
    "linux": "veloren-voxygen",
    "macos": "veloren-voxygen",
    "windows": "veloren-voxygen.exe",
}
SERVER_BINARIES = {
    "linux": "veloren-server-cli",
    "macos": "veloren-server-cli",
    "windows": "veloren-server-cli.exe",
}


def log(message: str) -> None:
    print(f"[package] {message}", flush=True)


def fail(message: str) -> NoReturn:
    print(f"[package] error: {message}", file=sys.stderr, flush=True)
    raise SystemExit(1)


def default_version() -> str:
    for key in ("RELEASE_TAG", "GITHUB_REF_NAME"):
        value = os.environ.get(key, "").strip()
        if value:
            return value
    return "dev"


def safe_version(version: str) -> str:
    value = re.sub(r"[^A-Za-z0-9._-]+", "-", version.strip()).strip("-")
    return value or "dev"


def plist_version(version: str) -> str:
    value = version.strip().lstrip("v").split("-")[0]
    return value if re.fullmatch(r"\d+(?:\.\d+){0,2}", value) else "0.0.0"


def verify_assets(assets_dir: Path) -> None:
    canary = assets_dir / CANARY
    if not canary.is_file():
        fail(
            f"missing {canary}; the checkout does not contain the game assets. "
            "Run 'git lfs install && git lfs pull' first."
        )
    with canary.open("r", encoding="utf-8", errors="replace") as handle:
        head = handle.read(256)
    if not head.startswith("VELOREN_CANARY_MAGIC"):
        fail(
            f"{canary} looks like a Git LFS pointer, not the real asset. "
            "Run 'git lfs pull' before packaging."
        )


def copy_docs(destination: Path) -> None:
    for name in DOC_FILES:
        source = REPO_ROOT / name
        if source.is_file():
            shutil.copy2(source, destination / name)
        elif name != "RUNNING.md":
            log(f"warning: optional documentation file {name} is missing")


def copy_assets(source: Path, destination: Path, skip: bool) -> None:
    if skip:
        destination.mkdir(parents=True, exist_ok=True)
        log("warning: --skip-assets was used; the archive is a layout test only")
        return
    shutil.copytree(source, destination)
    log(f"copied assets ({sum(p.stat().st_size for p in destination.rglob('*') if p.is_file()) / 1_000_000:.0f} MB)")


def copy_binary(source: Path, destination: Path, label: str) -> None:
    if not source.is_file():
        fail(f"missing {label} binary: {source}")
    shutil.copy2(source, destination)
    destination.chmod(destination.stat().st_mode | 0o111)
    log(f"copied {label} ({source.name})")


def write_script(path: Path, content: str, *, crlf: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\r\n" if crlf else "\n") as handle:
        handle.write(content)
    path.chmod(path.stat().st_mode | 0o755)


def linux_launcher(server: bool = False) -> str:
    binary = "./ValocraftServer" if server else "./Valocraft"
    name = "server" if server else "client"
    return f"""#!/usr/bin/env bash
set -euo pipefail

here="$(cd -- "$(dirname -- "${{BASH_SOURCE[0]}}")" && pwd)"
cd "$here"

if [ ! -f "assets/common/canary.canary" ]; then
    echo "Valocraft: game assets are missing." >&2
    echo "Extract the full release archive, not only the executable." >&2
    exit 1
fi

export VELOREN_ASSETS="$here/assets"
export VELOREN_USERDATA="$here/userdata"
mkdir -p "$VELOREN_USERDATA"

exec {binary} "$@"
"""


def windows_launcher(server: bool = False) -> str:
    binary = "ValocraftServer.exe" if server else "Valocraft.exe"
    launch = binary if server else f'start "" "{binary}" %*'
    return f"""@echo off
setlocal
cd /d "%~dp0"

if not exist "assets\\common\\canary.canary" (
    echo Valocraft: game assets are missing. 1>&2
    echo Extract the full release archive, not only the executable. 1>&2
    exit /b 1
)

set "VELOREN_ASSETS=%CD%\\assets"
set "VELOREN_USERDATA=%CD%\\userdata"
if not exist "%VELOREN_USERDATA%" mkdir "%VELOREN_USERDATA%"

{launch}
"""


def macos_client_wrapper() -> str:
    return """#!/bin/bash
set -euo pipefail

here="$(cd -- "$(dirname -- "$0")" && pwd)"
assets="$here/../Resources/assets"

if [ ! -f "$assets/common/canary.canary" ]; then
    message="Valocraft could not find its game assets. Extract the full release archive."
    osascript -e "display alert \\"Valocraft\\" message \\"$message\\"" >/dev/null 2>&1 || true
    echo "$message" >&2
    exit 1
fi

export VELOREN_ASSETS="$assets"
export VELOREN_USERDATA="$HOME/Library/Application Support/Valocraft/userdata"
mkdir -p "$VELOREN_USERDATA"

exec "$here/Valocraft-bin" "$@"
"""


def macos_server_wrapper() -> str:
    return """#!/bin/bash
set -euo pipefail

here="$(cd -- "$(dirname -- "$0")" && pwd)"
assets="$here/../Resources/assets"

export VELOREN_ASSETS="$assets"
export VELOREN_USERDATA="$HOME/Library/Application Support/Valocraft/userdata"
mkdir -p "$VELOREN_USERDATA"

exec "$here/ValocraftServer-bin" "$@"
"""


def write_macos_bundle(stage: Path, binary_dir: Path, assets_dir: Path, version: str, skip_assets: bool) -> None:
    app = stage / "Valocraft.app"
    contents = app / "Contents"
    macos = contents / "MacOS"
    resources = contents / "Resources"
    macos.mkdir(parents=True)
    resources.mkdir(parents=True)

    copy_binary(binary_dir / CLIENT_BINARIES["macos"], macos / "Valocraft-bin", "macOS client")
    if (binary_dir / SERVER_BINARIES["macos"]).is_file():
        copy_binary(binary_dir / SERVER_BINARIES["macos"], macos / "ValocraftServer-bin", "macOS server")
    else:
        log("dedicated server binary not found; the macOS package will only contain the client")

    write_script(macos / "Valocraft", macos_client_wrapper())
    write_script(macos / "ValocraftServer", macos_server_wrapper())

    copy_assets(assets_dir, resources / "assets", skip_assets)
    copy_docs(resources)

    info = {
        "CFBundleName": "Valocraft",
        "CFBundleDisplayName": "Valocraft",
        "CFBundleIdentifier": "com.valocraft.game",
        "CFBundleVersion": plist_version(version),
        "CFBundleShortVersionString": plist_version(version),
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundlePackageType": "APPL",
        "CFBundleExecutable": "Valocraft",
        "LSMinimumSystemVersion": "10.15",
        "NSHighResolutionCapable": True,
        "NSSupportsAutomaticGraphicsSwitching": True,
    }
    with (contents / "Info.plist").open("wb") as handle:
        plistlib.dump(info, handle)
    (contents / "PkgInfo").write_text("APPL????", encoding="ascii")

    write_script(
        stage / "Run Valocraft.command",
        """#!/bin/bash
set -euo pipefail
cd -- "$(dirname -- "$0")"
open "Valocraft.app"
""",
    )
    write_script(
        stage / "Run Valocraft Server.command",
        """#!/bin/bash
set -euo pipefail
cd -- "$(dirname -- "$0")"
exec "Valocraft.app/Contents/MacOS/ValocraftServer" "$@"
""",
    )
    copy_docs(stage)
    log("created Valocraft.app")


def create_archive(stage: Path, out_dir: Path, platform: str, version: str, arch: str) -> Path:
    base = f"Valocraft-{version}-{platform}-{arch}"
    if platform == "windows":
        archive = out_dir / f"{base}.zip"
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
            for path in sorted(stage.rglob("*")):
                if path.is_file():
                    zf.write(path, path.relative_to(out_dir))
        return archive

    archive = out_dir / f"{base}.tar.gz"
    with tarfile.open(archive, "w:gz", compresslevel=6) as tf:
        tf.add(stage, arcname=stage.name)
    return archive


def create_dmg(stage: Path, out_dir: Path, version: str, arch: str) -> Path | None:
    if sys.platform != "darwin":
        return None
    dmg = out_dir / f"Valocraft-{version}-macos-{arch}.dmg"
    if dmg.exists():
        dmg.unlink()
    subprocess.run(
        [
            "hdiutil",
            "create",
            "-volname",
            "Valocraft",
            "-srcfolder",
            str(stage),
            "-ov",
            "-format",
            "UDZO",
            str(dmg),
        ],
        check=True,
    )
    return dmg


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", required=True, choices=("linux", "macos", "windows"))
    parser.add_argument("--arch", default="x86_64", help="target architecture (for filenames)")
    parser.add_argument("--version", default=default_version(), help="release version")
    parser.add_argument(
        "--binary-dir",
        type=Path,
        default=REPO_ROOT / "target" / "release",
        help="directory containing the built veloren-voxygen and veloren-server-cli",
    )
    parser.add_argument(
        "--assets-dir",
        type=Path,
        default=REPO_ROOT / "assets",
        help="directory containing the game assets",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=REPO_ROOT / "dist",
        help="directory for the generated archive",
    )
    parser.add_argument("--skip-assets", action="store_true", help="layout test: do not copy assets")
    parser.add_argument("--no-archive", action="store_true", help="prepare the stage directory only")
    parser.add_argument("--keep-stage", action="store_true", help="keep the stage directory after archiving")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    version = safe_version(args.version)
    binary_dir = args.binary_dir.resolve()
    assets_dir = args.assets_dir.resolve()
    out_dir = args.out_dir.resolve()
    platform = args.platform

    if not args.skip_assets:
        verify_assets(assets_dir)
    else:
        log("skipping asset validation because --skip-assets was passed")

    out_dir.mkdir(parents=True, exist_ok=True)
    stage = out_dir / f"Valocraft-{version}-{platform}-{args.arch}"
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)

    copy_docs(stage)

    if platform == "macos":
        write_macos_bundle(stage, binary_dir, assets_dir, version, args.skip_assets)
    else:
        client_name = "Valocraft.exe" if platform == "windows" else "Valocraft"
        server_name = "ValocraftServer.exe" if platform == "windows" else "ValocraftServer"
        copy_binary(binary_dir / CLIENT_BINARIES[platform], stage / client_name, "client")
        if (binary_dir / SERVER_BINARIES[platform]).is_file():
            copy_binary(binary_dir / SERVER_BINARIES[platform], stage / server_name, "server")
        else:
            log("dedicated server binary not found; the package will only contain the client")

        if platform == "linux":
            copy_assets(assets_dir, stage / "assets", args.skip_assets)
            write_script(stage / "run.sh", linux_launcher())
            write_script(stage / "run-server.sh", linux_launcher(server=True))
        else:
            copy_assets(assets_dir, stage / "assets", args.skip_assets)
            write_script(stage / "run.bat", windows_launcher(), crlf=True)
            write_script(stage / "run-server.bat", windows_launcher(server=True), crlf=True)

    if args.no_archive:
        log(f"stage prepared at {stage}")
        return

    archive = create_archive(stage, out_dir, platform, version, args.arch)
    log(f"created {archive} ({archive.stat().st_size / 1_000_000:.0f} MB)")

    if platform == "macos":
        dmg = create_dmg(stage, out_dir, version, args.arch)
        if dmg is not None:
            log(f"created {dmg} ({dmg.stat().st_size / 1_000_000:.0f} MB)")

    if not args.keep_stage:
        shutil.rmtree(stage)


if __name__ == "__main__":
    main()
