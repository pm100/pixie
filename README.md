<p align="center"><img src="assets/icon.svg" alt="Pixie" width="120"></p>

# Pixie

A desktop photo manager for everyday users, not professionals. Point it at the folders where your photos already live and browse them by date, folder and place. Pixie never moves or changes your originals.

Built with [Tauri 2](https://tauri.app), Rust and Svelte 5. It targets Windows and macOS. Today it is developed and tested on Windows only; the macOS build is untried and unsigned.

> **Status: early development.** Browsing, maps, trips, Auto tags, people and duplicate finding work. Slideshow, print, editing and collages are designed but not built yet. See [What works today](#what-works-today) and the [issue tracker](https://github.com/pm100/pixie/issues).

## What works today

**Your library**
- **Add folders and forget them.** A welcome screen suggests where your photos probably are (Pictures, OneDrive, other drives). Pixie indexes them in place in the background, rescans only what changed, and keeps folders up to date by itself. Missing files are marked, not forgotten. Pixie never moves, edits or deletes your originals.
- **Photos and videos.** JPEG, PNG, WebP, TIFF and HEIC photos, and MP4, MOV and M4V videos.
- **Fast browsing.** Thumbnails are built in the background and cached; a library of thousands of photos scrolls smoothly. Pause and resume the background work whenever you like.

**Browse**
- **Three-pane layout.** A list on the left, the photo grid in the middle, details on the right. Divider widths are remembered.
- **Filters that work together.** Pick a date (year, month, day, "Last N days" or undated), a media type, a folder, an Auto tag, a person or a trip, or use the map. Counts in the list follow whatever else is chosen, and Clear all starts again.
- **By date.** Photos sit under day headings ("Sat 3 Jun 2024 · 14 photos"); sort newest or oldest first.
- **Viewer.** A full-window viewer, also available in its own window; arrow keys move through the current order. Select with click, ctrl-click and shift-click.
- **Details.** Date, place, camera information, the people in the photo and a small map.

**Places**
- **Location.** For a photo with GPS, Pixie shows a map and the place name in your language. Famous places are named (for example "Colosseum") alongside a "town, region, country" line.
- **Map of everything.** Switch Browse to the map to see all geotagged photos as clustered pins, filter by the area on screen and by date.
- **Trips.** Pixie guesses where home is (you can move it), then finds runs of days spent away. Trips are listed by year and destination, with nights away, a route on the map and names you can edit. You choose what counts as a trip in Settings.

**What is in your photos** (all analysed on your computer, in one background pass)
- **Auto tags.** Labels such as Beach, Museum or Dog, grouped into categories you can browse by.
- **People.** Pixie finds faces and groups them into people. Name a group in the People tab and the rest of that person's photos follow. A walk-through lets you confirm uncertain matches, ignore groups, and move faces out of a wrong group.
- **Duplicates.** Files that appear more than once are found and shown as one item in Browse. The Duplicates tab lets you pick which copy to keep. Nothing is deleted for good.

Planned next: a Windows wallpaper and lock-screen slideshow, simple editing, print, collages and emailing photos to people by nickname. The design is in [`docs/superpowers/specs`](docs/superpowers/specs).

## Privacy

Pixie works offline, and your photos never leave your computer. There are two uses of the network:

- **Locations.** When you select a photo that has GPS, Pixie sends **that photo's coordinates** (and nothing else) to OpenStreetMap, for map tiles and the town, region and country line (Nominatim), and to Wikipedia, to find a well-known place nearby. Answers are cached on your machine. With no network the map shows "Map unavailable" and everything else keeps working.
- **Analysis models.** The first time Pixie analyses pictures it downloads small open models (CLIP for Auto tags, YuNet and SFace for faces) from Hugging Face and GitHub. Only the model files are downloaded; none of your photos or data are sent. After that the analysis runs entirely offline.

## Building and running

### Prerequisites

- [Rust](https://rustup.rs) (stable), [Node.js](https://nodejs.org) 22 or newer, and the [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) for your platform (on Windows: WebView2 and the MSVC build tools).
- [`just`](https://github.com/casey/just) is optional.
- **libheif**, for HEIC photos.
  - **Windows** ([vcpkg](https://github.com/microsoft/vcpkg)):
    ```powershell
    git clone https://github.com/microsoft/vcpkg C:\vcpkg
    C:\vcpkg\bootstrap-vcpkg.bat
    C:\vcpkg\vcpkg install "libheif[core]:x64-windows"
    setx VCPKG_ROOT C:\vcpkg
    setx VCPKGRS_TRIPLET x64-windows
    setx VCPKGRS_DYNAMIC 1
    setx PATH "%PATH%;C:\vcpkg\installed\x64-windows\bin"
    ```
    Open a new shell afterwards. libheif and libde265 are LGPL, so they are linked as DLLs (`heif.dll`, `libde265.dll`); the build copies them next to the program and into the installer. The `PATH` entry lets `cargo test` find them.
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

On Windows this produces a per-user NSIS installer (`just win-build` does the same with the vcpkg paths preset). Code signing, a macOS bundle and Store submission are not set up yet.

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

## First run

With no folders yet, Pixie opens a welcome screen that suggests where your photos probably are (Pictures, Videos,
OneDrive and similar, plus a bounded look under your home folder). Tick what you want and press Start: a full-screen
scan reads the files, then "now analysing" adds Auto tags and finds faces, then an "All set" screen says what was
found. "Carry on in the background", on either of the last two steps, lets you browse while the rest finishes. Adding a folder later in
Settings, Photo folders, uses the same scan without the welcome. Launch rescans stay in the background.

## Data and caches

Pixie keeps its database, thumbnails, map tiles and place cache in its app data folder (on Windows, `%LOCALAPPDATA%\app.pixie.desktop`). Deleting that folder resets Pixie; your photos are untouched.

## Contributing

Issues and pull requests are welcome. See [`AGENTS.md`](AGENTS.md) for working rules for AI coding agents in this repository.

## License

[MIT](LICENSE). HEIC support uses libheif and libde265 (both LGPL-3.0). On Windows they ship as separate DLLs beside the program, so you can replace them with your own build; they are built without the GPL x265 encoder (hence `libheif[core]`) because Pixie only decodes HEIC. HEVC, the codec inside HEIC, is also covered by patents that a code licence does not address; review that before distributing binaries.
