# Releasing a desktop build

The release workflow is `.github/workflows/release.yml`. It builds, packages, and
publishes the downloadable desktop archives. You do not upload release assets by
hand.

## Trigger a release

### Option A: push a version tag

```bash
git tag -a v0.1.0 -m "Valocraft v0.1.0"
git push github v0.1.0
```

The tag must include the release workflow commit. The workflow creates the
GitHub Release for that tag and attaches the platform archives.

### Option B: run the workflow manually

1. Open **Actions → Build desktop release → Run workflow**.
2. Enter the release tag (for example `v0.1.0`).
3. Leave **Mark the GitHub Release as a prerelease** enabled for alpha builds.
4. Run it.

The workflow creates the tag/release at the selected commit if it does not exist.

## What gets built

| Job | Runner | Package |
|---|---|---|
| Linux x86_64 | `ubuntu-22.04` | `.tar.gz` |
| Windows x86_64 | `windows-2022` | `.zip` |
| macOS Apple Silicon | `macos-15` | `.dmg` and `.tar.gz` |
| macOS Intel | `macos-15-intel` | `.dmg` and `.tar.gz` (optional; failure does not block the release) |

Each package contains the client, the dedicated server, `assets/`, launchers, and
the player documentation. The client is built with

```text
--no-default-features --features default-publish,shaderc-from-source
```

and the server is built with its default feature set. A `prepare-assets` job
checks out the Git LFS assets **once** and shares them with the platform build
jobs through an Actions artifact, which keeps LFS bandwidth usage to roughly one
asset download per release. The repository's LFS objects must be present before a
release run.

## Before tagging

- Make sure the source builds locally.
- Make sure `assets/` contains real files, not LFS pointers. The canary must start
  with `VELOREN_CANARY_MAGIC`.
- Update `README.md`, `RUNNING.md`, and the version notes if needed.
- Bump the version string in `website/` (search for the old tag, e.g. `v0.1.3`,
  across `website/*.html`) — see `website/README.md`. The site redeploys to
  GitHub Pages automatically on push.
- Commit everything and push `main` to the GitHub remote.
- If the release is intended to be stable, set the prerelease input to false.

## After the workflow starts

Builds are long (the shaderc source build plus a thin-LTO game build). The four
platforms run in parallel. The final `Publish GitHub Release` job downloads the
successful artifacts and creates the release.

If one platform fails:

1. Read that job's log.
2. Fix the build or dependency.
3. Re-run the failed job from the Actions page.
4. The release job can be re-run after the artifacts exist.

The release job is intentionally allowed to publish the artifacts that did build;
review the release page and download each archive before announcing it broadly.
macOS Intel is optional because GitHub's Intel runner is the last Intel image and
will eventually be retired.

## Local packaging check

You can validate the package layout without uploading anything:

```bash
python3 scripts/package_release.py \
  --platform linux \
  --arch x86_64 \
  --version v0.1.0-test \
  --binary-dir target/release \
  --skip-assets \
  --out-dir /tmp/valocraft-package-test
```

Remove `--skip-assets` for a real package. A normal Linux package is roughly
800 MB unpacked and 600 MB compressed because of the game assets.

## Versioning

The game's internal workspace version is not changed by the release tag. The tag
is the distribution version; the binaries still report their compiled version.
Use a new tag for every release. Do not reuse a published tag for a different
build.

## Windows download reliability

The Windows package is the first-class download. The release must contain the
archive named `Valocraft-<tag>-windows-x86_64.zip` and a `SHA256SUMS.txt` file.
Players should download that game archive, not GitHub's `Source code (zip)`.
The release notes should link directly to the Windows archive and explain that
browsers may show a **Keep** / **Download anyway** prompt because the build is
not code-signed yet.
