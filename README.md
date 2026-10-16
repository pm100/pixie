<p align="center"><img src="assets/icon.svg" alt="Pixie" width="120"></p>

# Pixie

A desktop photo manager for everyday users, not professionals. Point it at the folders where your photos already live and browse them by date, folder and place. Pixie never moves or changes your originals.

Built with [Tauri 2](https://tauri.app), Rust and Svelte 5. It targets Windows and macOS. Today it is developed and tested on Windows only.

> **Status: early development.** The library, browsing and location views work. Search, slideshow, print, editing and collages are designed but not built yet. See [What works today](#what-works-today) and the [issue tracker](https://github.com/pm100/pixie/issues).

## What works today

- **Library.** Add photo folders; Pixie indexes them in place in the background and rescans only what changed. Missing files are marked, not forgotten. JPEG, PNG, WebP, TIFF and HEIC.
- **Fast browsing.** Thumbnails are built in the background and cached. A library of about 5,800 photos scrolls smoothly.
- **Three-pane layout.** Tree on the left, photo grid in the middle, details on the right. Divider widths are remembered.
- **By date.** Photos are grouped under day headings ("Sat 3 Jun 2024 · 14 photos"). The Dates tree (year, month, day) jumps to a day and follows your scrolling. Sort newest or oldest first; photos with no date always come last.
- **By folder.** Click a folder in the tree to filter the grid to it.
- **Selection.** Click, ctrl-click and shift-click.
- **Viewer.** Full-window viewer; arrow keys move to the next photo in the current order.
- **Location.** For a photo with GPS, the details pane shows a small map and the place name, in your language. Famous places are named (for example "Colosseum"), alongside a "town, region, country" line.

Planned next: a map of all geotagged photos, emailing photos to people by nickname, videos, search and duplicates, a Windows wallpaper/lock-screen slideshow, simple editing, print, collages. The design is in [`docs/superpowers/specs`](docs/superpowers/specs).

## Privacy

Pixie works offline, and your photos never leave your computer. The one exception is the location feature. When you select a photo that has GPS, Pixie sends **that photo's coordinates** (and nothing else) to:

- OpenStreetMap, for map tiles and the town/region/country line (Nominatim);
- Wikipedia, to find a well-known place nearby.

Answers are cached on your machine. With no network the map shows "Map unavailable" and everything else keeps working.

## Building and running

### Prerequisites

- [Rust](https://rustup.rs) (stable), [Node.js](https://nodejs.org) 22 or newer, and the [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) for your platform (on Windows: WebView2 and the MSVC build tools).
- [`just`](https://github.com/casey/just) is optional.
- **libheif**, for HEIC photos.
  - **Windows** ([vcpkg](https://github.com/microsoft/vcpkg)):
    ```powershell
    git clone https://github.com/microsoft/vcpkg C:\vcpkg
    C:\vcpkg\bootstrap-vcpkg.bat
    C:\vcpkg\vcpkg install libheif:x64-windows-static-md
    setx VCPKG_ROOT C:\vcpkg
    setx VCPKGRS_TRIPLET x64-windows-static-md
    ```
    Open a new shell afterwards.
  - **macOS:** `brew install libheif pkg-config` (not yet tested by the author).
  - To build without HEIC support, pass `--no-default-features` to cargo.

### Run

```sh
npm install
npm --prefix ui install
npm run tauri dev
```

On the author's Windows machine, `just winlaunch` does the same with the vcpkg paths preset (edit the paths in the `justfile` for your setup).

Use the local Tauri CLI through `npm run tauri ...`, not a globally installed `cargo tauri` (which may be v1).

### Test

```sh
cargo test --workspace      # Rust
npm --prefix ui test        # UI unit tests
npm --prefix ui run check   # Svelte / TypeScript
```

CI runs all of these on Windows.

### Release build

```sh
npm run tauri build
```

Installers and code signing are not set up yet.

## Project layout

| Path | What it is |
|---|---|
| `crates/pixie-core` | Shared types and errors. No I/O. |
| `crates/pixie-index` | SQLite library, folder scanning, EXIF, queries. |
| `crates/pixie-image` | Decoding (including HEIC) and thumbnail pipeline. |
| `crates/pixie-geo` | Map tiles and place-name lookups, with caches. |
| `src-tauri` | Tauri app: commands and the `pixie:` image protocol. |
| `ui` | Svelte 5 + TypeScript front end. |
| `assets` | Source artwork (the app icon). |
| `docs/superpowers` | Design specs and implementation plans. |

## Data and caches

Pixie keeps its database, thumbnails, map tiles and place cache in its app data folder (on Windows, `%APPDATA%\app.pixie.desktop`). Deleting that folder resets Pixie; your photos are untouched.

## Contributing

Issues and pull requests are welcome. See [`AGENTS.md`](AGENTS.md) for working rules for AI coding agents in this repository.

## License

[MIT](LICENSE). HEIC support links libheif, libde265 and x265, which have their own licenses; review them before distributing binaries.
