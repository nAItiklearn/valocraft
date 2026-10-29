# Valocraft website

The official Valocraft website — a static site (plain HTML/CSS/JS, no build step)
deployed to **GitHub Pages** from the `website/` folder by
`.github/workflows/pages.yml`.

- **Live URL:** https://naitiklearn.github.io/valocraft/
- **Pages:** `index.html` (home), `download.html`, `guide.html`, `license.html`, `privacy.html`

## Updating for a new release

1. Search-and-replace the version string (currently `v0.1.3`) across all `*.html` files
   — it appears in download links, file names, hero/footer text, and the news card.
2. Update the "Latest release" news card on `index.html`.
3. Commit to `main` — the Pages workflow redeploys automatically.

## Screenshots

Images live in `website/images/`. **The current feature/gallery art is generated
voxel-style SVG placeholder art** (by `/tmp/opencode/gen_site_art.py`) — swap in
real in-game screenshots when available (screenshot key `F4`; captures land in
`userdata/screenshots/`, convert to WebP and update the `.svg` references in
`index.html`). The hero background is set in `css/style.css` (`.hero-bg.has-image`).

## Local preview

Any static file server works:

```bash
cd website && python3 -m http.server 8000
# open http://localhost:8000
```
