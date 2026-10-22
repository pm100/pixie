# Pixie M1: Skeleton and Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri desktop app (Windows and macOS) that indexes chosen photo folders in place, generates thumbnails in the background (including HEIC), and shows them in a fast virtualized grid with a basic viewer, plus a print test page that settles the WebView2/WebKit print-scaling risk.

**Architecture:** Rust workspace of small crates (`pixie-core` types, `pixie-image` decode/metadata/thumbnails, `pixie-index` SQLite + scanner) behind a thin Tauri command layer. The Svelte UI never touches files; it gets thumbnails through a `pixie://` custom protocol and photo lists through commands. Scanning and thumbnailing run on background threads.

**Tech Stack:** Rust 1.89, Tauri 2, Svelte 5 + Vite + TypeScript, rusqlite (bundled SQLite), `image` 0.25, `kamadak-exif`, `walkdir`, `libheif-rs` (HEIC), vitest.

**Spec:** `docs/superpowers/specs/2026-10-15-pixie-design.md` (this plan is Milestone 1 of 7; later milestones get their own plans).

## Global Constraints

- Platforms: Windows and macOS only. Originals are never modified or deleted.
- Formats in v1: JPEG, PNG, WebP, TIFF, **HEIC**. RAW is out of scope.
- Library scale: tens of thousands of photos; scanning is incremental (path + size + mtime) and runs off the UI thread.
- Missing files are marked `missing`, never deleted from the index.
- Thumbnails: ~256px grid and ~1600px viewer classes, cached on disk keyed by photo id and mtime.
- Every command returns a typed `PixieError` with a plain-language message; raw Rust errors are never shown to users.
- Write operations never overwrite an existing file.
- Tauri 2 (not 1). The globally installed `cargo tauri` is 1.5.9; do **not** use it. Use the npm-local CLI (`npm run tauri ...`).
- Commit messages end with the line `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

Spec-implied conditions no obvious happy-path test covers, most likely to bite first. Each has a test in the owning task.

1. **Photo with no EXIF at all** (screenshots, downloads): date falls back to file mtime, not NULL (Task 6).
2. **Corrupt or zero-byte file named `.jpg`**: indexed, scan continues, thumbnail request returns an error and the UI shows a placeholder, never a crash (Tasks 6, 7, 10).
3. **Watched folder unreachable** (unplugged drive): scan returns an error and marks **nothing** missing (Task 6).
4. **Same filename in different subfolders, paths with spaces and non-ASCII characters, macOS `._name.jpg` resource-fork files**: distinct photos, no collisions, junk ignored (Tasks 2, 6).
5. **EXIF-rotated photos (orientation 6/8) and replaced files**: thumbnail and stored dimensions are upright; a file replaced in place is re-indexed and gets a fresh thumbnail (Tasks 3, 4, 6).

**Plan-level deviation from the spec:** EXIF reading lives in `pixie-image::meta` (not `pixie-index`) because orientation handling and the scanner share it; `pixie-index` depends on `pixie-image`.

## File Structure

```
Cargo.toml                         workspace
package.json                       root: @tauri-apps/cli only
.gitignore
.github/workflows/ci.yml
crates/
  pixie-core/src/lib.rs            types, PixieError, is_supported_image
  pixie-testutil/src/lib.rs        test-only JPEG/EXIF builders (dev-dependency)
  pixie-image/src/lib.rs           re-exports
  pixie-image/src/meta.rs          EXIF metadata + orientation
  pixie-image/src/decode.rs        decode_oriented, apply_orientation
  pixie-image/src/heic.rs          libheif decode (feature "heic")
  pixie-image/src/thumbs.rs        thumb_path, ensure_thumbnail
  pixie-image/src/service.rs       ThumbService worker pool
  pixie-index/src/lib.rs           re-exports
  pixie-index/src/db.rs            Db, migrations, queries
  pixie-index/src/scanner.rs       scan_folder
  pixie-index/examples/bench_scan.rs   50k-photo benchmark
src-tauri/
  Cargo.toml build.rs tauri.conf.json capabilities/default.json icons/
  src/main.rs src/lib.rs           commands, pixie:// protocol, AppState
ui/
  package.json vite.config.ts svelte.config.js tsconfig.json index.html
  src/main.ts src/App.svelte
  src/lib/api.ts url.ts gridMath.ts
  src/lib/Grid.svelte Viewer.svelte PrintTest.svelte
  src/lib/url.test.ts gridMath.test.ts
docs/print-findings.md             manual print results (Task 11)
```

---

### Task 1: Repository, workspace and app shell that launches

**Files:**
- Create: `.gitignore`, `Cargo.toml`, `package.json`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
- Create: `ui/package.json`, `ui/vite.config.ts`, `ui/svelte.config.js`, `ui/tsconfig.json`, `ui/index.html`, `ui/src/main.ts`, `ui/src/App.svelte`

**Interfaces:**
- Produces: a buildable workspace (`members = ["crates/*", "src-tauri"]`), npm scripts `npm run tauri dev|build`, `npm --prefix ui run dev|build|test`.

- [ ] **Step 1: Initialise git and ignore files**

```bash
cd /c/work/pixie && git init
```

`.gitignore`:
```
/target
node_modules
/ui/dist
/src-tauri/gen
*.tmp
```

- [ ] **Step 2: Workspace manifest**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/*", "src-tauri"]

[workspace.package]
edition = "2021"
version = "0.1.0"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "1"
image = { version = "0.25", default-features = false, features = ["jpeg", "png", "webp", "tiff"] }
kamadak-exif = "0.6"
rusqlite = { version = "0.32", features = ["bundled"] }
walkdir = "2"
tempfile = "3"
```

Create the crates dir so the glob member resolves: `mkdir -p crates` (later tasks add crates; until then `cargo` complains about an empty glob only for `cargo metadata` of nothing, so do Step 5 before building).

- [ ] **Step 3: Root package.json (Tauri CLI v2, local)**

`package.json`:
```json
{
  "name": "pixie",
  "private": true,
  "scripts": { "tauri": "tauri" },
  "devDependencies": { "@tauri-apps/cli": "^2" }
}
```

- [ ] **Step 4: UI project**

`ui/package.json`:
```json
{
  "name": "pixie-ui",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "test": "vitest run"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^5",
    "svelte": "^5",
    "svelte-check": "^4",
    "typescript": "^5",
    "vite": "^6",
    "vitest": "^3"
  }
}
```

`ui/vite.config.ts`:
```ts
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  test: { include: ['src/**/*.test.ts'] },
});
```

`ui/svelte.config.js`:
```js
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
export default { preprocess: vitePreprocess() };
```

`ui/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2022", "module": "ESNext", "moduleResolution": "bundler",
    "strict": true, "skipLibCheck": true, "verbatimModuleSyntax": true,
    "types": ["svelte"], "lib": ["ES2022", "DOM"]
  },
  "include": ["src/**/*.ts", "src/**/*.svelte"]
}
```

`ui/index.html`:
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Pixie</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

`ui/src/main.ts`:
```ts
import { mount } from 'svelte';
import App from './App.svelte';

mount(App, { target: document.getElementById('app')! });
```

`ui/src/App.svelte` (placeholder, replaced in Task 10):
```svelte
<main><h1>Pixie</h1></main>
```

- [ ] **Step 5: Tauri shell**

`src-tauri/Cargo.toml`:
```toml
[package]
name = "pixie-app"
version.workspace = true
edition.workspace = true

[lib]
name = "pixie_app_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
serde = { workspace = true }
serde_json = "1"
```

`src-tauri/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```

`src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pixie_app_lib::run()
}
```

`src-tauri/src/lib.rs`:
```rust
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running Pixie");
}
```

`src-tauri/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Pixie",
  "version": "0.1.0",
  "identifier": "app.pixie.desktop",
  "build": {
    "beforeDevCommand": "npm --prefix ui run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm --prefix ui run build",
    "frontendDist": "../ui/dist"
  },
  "app": {
    "windows": [
      { "label": "main", "title": "Pixie", "width": 1200, "height": 800, "minWidth": 700, "minHeight": 500 }
    ],
    "security": {
      "csp": "default-src 'self'; img-src 'self' data: pixie: http://pixie.localhost; style-src 'self' 'unsafe-inline'"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```

`src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Main window permissions",
  "windows": ["main"],
  "permissions": ["core:default", "dialog:default"]
}
```

- [ ] **Step 6: Generate a placeholder icon**

```powershell
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap 1024,1024
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.Clear([System.Drawing.Color]::FromArgb(255,52,120,246))
$bmp.Save("$env:TEMP\pixie-icon.png", [System.Drawing.Imaging.ImageFormat]::Png)
```
Then: `npm install && npm run tauri icon "$env:TEMP/pixie-icon.png"` (writes `src-tauri/icons/*`).

- [ ] **Step 7: Install and verify build**

Run:
```bash
cd /c/work/pixie && npm install && npm --prefix ui install && npm --prefix ui run build && cargo build -p pixie-app
```
Expected: UI builds to `ui/dist`; cargo finishes with `Finished`. (First cargo build downloads Tauri deps; several minutes.)

- [ ] **Step 8: Launch smoke check**

Run: `npm run tauri dev`. Expected: a window titled "Pixie" showing the heading "Pixie". Close it.

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "chore: workspace, Tauri 2 shell and Svelte UI scaffold

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: pixie-core types and error

**Files:**
- Create: `crates/pixie-core/Cargo.toml`, `crates/pixie-core/src/lib.rs`

**Interfaces:**
- Produces (used by every later task):
  - `type PhotoId = i64; type FolderId = i64;`
  - `struct PhotoMeta { width: Option<u32>, height: Option<u32>, taken_at: Option<i64>, camera: Option<String>, gps_lat: Option<f64>, gps_lon: Option<f64> }` (`Default`, `Clone`, `PartialEq`, serde)
  - `struct Photo { id, folder_id, path: String, size: i64, mtime: i64, #[serde(flatten)] meta: PhotoMeta, missing: bool }`
  - `struct Folder { id: FolderId, path: String }`
  - `enum PixieError { Io(String), Db(String), Image(String), Unsupported(String), NotFound(String) }` with `user_message(&self) -> String`, `From<std::io::Error>`, `Serialize` (as the user message), `Clone`
  - `type Result<T> = std::result::Result<T, PixieError>`
  - `fn is_supported_image(path: &Path) -> bool`
  - times: `taken_at` is Unix **seconds**; `mtime` is Unix **milliseconds**.

- [ ] **Step 1: Crate manifest**

`crates/pixie-core/Cargo.toml`:
```toml
[package]
name = "pixie-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 2: Write the failing tests**

`crates/pixie-core/src/lib.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn supports_known_extensions_case_insensitively() {
        for name in ["a.jpg", "a.JPEG", "a.png", "a.webp", "a.tif", "a.TIFF", "a.heic", "a.HEIF"] {
            assert!(is_supported_image(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn rejects_other_files_and_macos_resource_forks() {
        for name in ["a.txt", "a.mp4", "noext", "._IMG_1.jpg", "dir/._x.heic"] {
            assert!(!is_supported_image(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn handles_unicode_and_spaces() {
        assert!(is_supported_image(Path::new("Ferien 2024/Café ☕.JPG")));
    }

    #[test]
    fn user_message_hides_internals() {
        let e = PixieError::Db("UNIQUE constraint failed: photos.path".into());
        assert!(!e.user_message().contains("UNIQUE"));
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("UNIQUE"));
    }
}
```
(`serde_json` is needed only for this test; add `[dev-dependencies] serde_json = "1"` to the manifest.)

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p pixie-core`
Expected: compile error, `is_supported_image` / `PixieError` not found.

- [ ] **Step 4: Implement**

Insert above `#[cfg(test)]` in `crates/pixie-core/src/lib.rs`:
```rust
pub type PhotoId = i64;
pub type FolderId = i64;

pub const SUPPORTED_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "webp", "tif", "tiff", "heic", "heif"];

/// True for supported image files. Ignores macOS `._name` resource-fork files.
pub fn is_supported_image(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.starts_with("._") {
        return false;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PhotoMeta {
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Unix seconds, EXIF local time treated as UTC.
    pub taken_at: Option<i64>,
    pub camera: Option<String>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Photo {
    pub id: PhotoId,
    pub folder_id: FolderId,
    pub path: String,
    pub size: i64,
    /// Unix milliseconds.
    pub mtime: i64,
    #[serde(flatten)]
    pub meta: PhotoMeta,
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: FolderId,
    pub path: String,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum PixieError {
    #[error("io: {0}")]
    Io(String),
    #[error("database: {0}")]
    Db(String),
    #[error("image: {0}")]
    Image(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("not found: {0}")]
    NotFound(String),
}

impl PixieError {
    /// Plain-language text safe to show to a non-technical user.
    pub fn user_message(&self) -> String {
        match self {
            PixieError::Io(_) => "Pixie couldn't read or write a file. Check that the folder or drive is connected.".into(),
            PixieError::Db(_) => "Pixie had a problem with its photo library data.".into(),
            PixieError::Image(_) => "This photo couldn't be opened. It may be damaged.".into(),
            PixieError::Unsupported(_) => "This kind of file isn't supported yet.".into(),
            PixieError::NotFound(_) => "Pixie couldn't find that folder or photo. It may have been moved or the drive disconnected.".into(),
        }
    }
}

impl From<std::io::Error> for PixieError {
    fn from(e: std::io::Error) -> Self {
        PixieError::Io(e.to_string())
    }
}

impl Serialize for PixieError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.user_message())
    }
}

pub type Result<T> = std::result::Result<T, PixieError>;
```

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo test -p pixie-core`
Expected: 4 passed.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(core): shared types, PixieError, supported-image filter

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Test utilities and EXIF metadata

**Files:**
- Create: `crates/pixie-testutil/Cargo.toml`, `crates/pixie-testutil/src/lib.rs`
- Create: `crates/pixie-image/Cargo.toml`, `crates/pixie-image/src/lib.rs`, `crates/pixie-image/src/meta.rs`

**Interfaces:**
- Produces (testutil): `jpeg(w: u32, h: u32) -> Vec<u8>` (left half red, right half blue); `jpeg_with_exif(w, h, orientation: u16, datetime: Option<&str>) -> Vec<u8>` (`datetime` format `"YYYY:MM:DD HH:MM:SS"`).
- Produces (pixie-image::meta): `read_orientation(path: &Path) -> u16` (1–8, default 1); `read_meta(path: &Path) -> PhotoMeta` (never fails; fields `None` when unavailable; width/height are **as displayed**, i.e. swapped for orientation 5–8); `parse_exif_datetime(&str) -> Option<i64>`.

- [ ] **Step 1: testutil crate**

`crates/pixie-testutil/Cargo.toml`:
```toml
[package]
name = "pixie-testutil"
version.workspace = true
edition.workspace = true
publish = false

[dependencies]
image = { workspace = true }
```

`crates/pixie-testutil/src/lib.rs`:
```rust
use image::{codecs::jpeg::JpegEncoder, ImageBuffer, Rgb};

/// Left half red, right half blue.
pub fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = ImageBuffer::from_fn(w, h, |x, _| {
        if x < w / 2 { Rgb([255u8, 0, 0]) } else { Rgb([0u8, 0, 255]) }
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&img)
        .expect("encode jpeg");
    out
}

fn le16(v: &mut Vec<u8>, x: u16) { v.extend_from_slice(&x.to_le_bytes()); }
fn le32(v: &mut Vec<u8>, x: u32) { v.extend_from_slice(&x.to_le_bytes()); }

fn exif_tiff(orientation: u16, datetime: Option<&str>) -> Vec<u8> {
    let mut t = vec![b'I', b'I', 0x2A, 0, 8, 0, 0, 0]; // header, IFD0 at 8
    le16(&mut t, if datetime.is_some() { 2 } else { 1 });
    // Orientation: tag 0x0112, SHORT, count 1
    le16(&mut t, 0x0112); le16(&mut t, 3); le32(&mut t, 1); le16(&mut t, orientation); le16(&mut t, 0);
    if datetime.is_some() {
        // Exif IFD pointer: 8 header + 2 count + 2*12 entries + 4 next = 38
        le16(&mut t, 0x8769); le16(&mut t, 4); le32(&mut t, 1); le32(&mut t, 38);
    }
    le32(&mut t, 0); // next IFD
    if let Some(dt) = datetime {
        assert_eq!(dt.len(), 19, "datetime must be 'YYYY:MM:DD HH:MM:SS'");
        le16(&mut t, 1);
        let data_off = t.len() as u32 + 12 + 4;
        le16(&mut t, 0x9003); le16(&mut t, 2); le32(&mut t, 20); le32(&mut t, data_off);
        le32(&mut t, 0);
        t.extend_from_slice(dt.as_bytes());
        t.push(0);
    }
    t
}

/// A JPEG with an EXIF APP1 segment inserted right after SOI.
pub fn jpeg_with_exif(w: u32, h: u32, orientation: u16, datetime: Option<&str>) -> Vec<u8> {
    let base = jpeg(w, h);
    let tiff = exif_tiff(orientation, datetime);
    let mut seg = vec![0xFF, 0xE1];
    let len = (2 + 6 + tiff.len()) as u16;
    seg.extend_from_slice(&len.to_be_bytes());
    seg.extend_from_slice(b"Exif\0\0");
    seg.extend_from_slice(&tiff);
    let mut out = base[..2].to_vec(); // SOI
    out.extend_from_slice(&seg);
    out.extend_from_slice(&base[2..]);
    out
}
```

- [ ] **Step 2: pixie-image crate manifest**

`crates/pixie-image/Cargo.toml`:
```toml
[package]
name = "pixie-image"
version.workspace = true
edition.workspace = true

[features]
heic = ["dep:libheif-rs"]

[dependencies]
pixie-core = { path = "../pixie-core" }
image = { workspace = true }
kamadak-exif = { workspace = true }
libheif-rs = { version = "1", optional = true, default-features = false }

[dev-dependencies]
pixie-testutil = { path = "../pixie-testutil" }
tempfile = { workspace = true }
```

`crates/pixie-image/src/lib.rs`:
```rust
pub mod meta;
```

- [ ] **Step 3: Write the failing tests**

`crates/pixie-image/src/meta.rs`:
```rust
use pixie_core::PhotoMeta;
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_testutil::{jpeg, jpeg_with_exif};

    #[test]
    fn parses_exif_datetime_as_utc_seconds() {
        assert_eq!(parse_exif_datetime("1970:01:01 00:00:00"), Some(0));
        assert_eq!(parse_exif_datetime("2024:06:01 12:00:00"), Some(1_717_243_200));
        assert_eq!(parse_exif_datetime("0000:00:00 00:00:00"), None);
        assert_eq!(parse_exif_datetime("garbage"), None);
    }

    #[test]
    fn reads_orientation_and_defaults_to_1() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.jpg");
        std::fs::write(&a, jpeg_with_exif(40, 20, 6, None)).unwrap();
        assert_eq!(read_orientation(&a), 6);
        let b = dir.path().join("b.jpg");
        std::fs::write(&b, jpeg(40, 20)).unwrap();
        assert_eq!(read_orientation(&b), 1);
        assert_eq!(read_orientation(&dir.path().join("missing.jpg")), 1);
    }

    #[test]
    fn meta_reports_displayed_dimensions_and_date() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("r.jpg");
        std::fs::write(&p, jpeg_with_exif(40, 20, 6, Some("2024:06:01 12:00:00"))).unwrap();
        let m = read_meta(&p);
        assert_eq!((m.width, m.height), (Some(20), Some(40))); // swapped for orientation 6
        assert_eq!(m.taken_at, Some(1_717_243_200));
    }

    #[test]
    fn meta_without_exif_has_no_date_and_never_panics() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("plain.jpg");
        std::fs::write(&p, jpeg(40, 20)).unwrap();
        let m = read_meta(&p);
        assert_eq!((m.width, m.height, m.taken_at), (Some(40), Some(20), None));

        let bad = dir.path().join("bad.jpg");
        std::fs::write(&bad, b"not a jpeg").unwrap();
        assert_eq!(read_meta(&bad), PhotoMeta::default());
        let empty = dir.path().join("empty.jpg");
        std::fs::write(&empty, b"").unwrap();
        assert_eq!(read_meta(&empty), PhotoMeta::default());
    }
}
```

- [ ] **Step 4: Run to verify failure**

Run: `cargo test -p pixie-image`
Expected: compile errors (functions undefined).

- [ ] **Step 5: Implement**

Insert above the tests in `meta.rs`:
```rust
fn is_heif(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("heic") | Some("heif")
    )
}

fn read_exif(path: &Path) -> Option<exif::Exif> {
    let f = std::fs::File::open(path).ok()?;
    exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(f))
        .ok()
}

fn ascii(e: &exif::Exif, tag: exif::Tag) -> Option<String> {
    match &e.get_field(tag, exif::In::PRIMARY)?.value {
        exif::Value::Ascii(v) => v
            .first()
            .map(|b| String::from_utf8_lossy(b).trim_matches('\0').trim().to_string())
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// EXIF orientation 1..=8; 1 when absent or unreadable.
pub fn read_orientation(path: &Path) -> u16 {
    read_exif(path)
        .and_then(|e| {
            e.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|f| f.value.get_uint(0))
        })
        .map(|v| v as u16)
        .filter(|v| (1..=8).contains(v))
        .unwrap_or(1)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "YYYY:MM:DD HH:MM:SS" -> Unix seconds (treated as UTC). None if invalid.
pub fn parse_exif_datetime(s: &str) -> Option<i64> {
    let b = s.trim();
    if b.len() < 19 { return None; }
    let n = |r: std::ops::Range<usize>| b.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (h, mi, se) = (n(11..13)?, n(14..16)?, n(17..19)?);
    if y < 1 || !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se)
}

fn gps_coord(e: &exif::Exif, tag: exif::Tag, ref_tag: exif::Tag) -> Option<f64> {
    let f = e.get_field(tag, exif::In::PRIMARY)?;
    let exif::Value::Rational(r) = &f.value else { return None };
    if r.len() < 3 { return None; }
    let v = r[0].to_f64() + r[1].to_f64() / 60.0 + r[2].to_f64() / 3600.0;
    let sign = match ascii(e, ref_tag).as_deref() {
        Some("S") | Some("W") => -1.0,
        _ => 1.0,
    };
    Some(v * sign).filter(|v| v.is_finite())
}

/// Best-effort metadata; never fails. Dimensions are as displayed (after orientation).
pub fn read_meta(path: &Path) -> PhotoMeta {
    let mut meta = PhotoMeta::default();
    let exif = read_exif(path);

    if is_heif(path) {
        // libheif applies rotation itself; ignore the EXIF orientation tag here.
        #[cfg(feature = "heic")]
        if let Some((w, h)) = crate::heic::dimensions(path) {
            meta.width = Some(w);
            meta.height = Some(h);
        }
    } else if let Ok((w, h)) = image::image_dimensions(path) {
        let o = read_orientation(path);
        let (w, h) = if o >= 5 { (h, w) } else { (w, h) };
        meta.width = Some(w);
        meta.height = Some(h);
    }

    if let Some(e) = &exif {
        meta.taken_at = ascii(e, exif::Tag::DateTimeOriginal)
            .or_else(|| ascii(e, exif::Tag::DateTime))
            .and_then(|s| parse_exif_datetime(&s));
        meta.camera = ascii(e, exif::Tag::Model);
        meta.gps_lat = gps_coord(e, exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef);
        meta.gps_lon = gps_coord(e, exif::Tag::GPSLongitude, exif::Tag::GPSLongitudeRef);
    }
    meta
}
```
Also add to `lib.rs`: `#[cfg(feature = "heic")] pub mod heic;` (the file is created in Task 8; until then do not enable the feature).

- [ ] **Step 6: Run tests**

Run: `cargo test -p pixie-image`
Expected: 4 passed. If `meta_reports_displayed_dimensions_and_date` fails on the date only, the hand-built EXIF offsets in `exif_tiff` are wrong; fix the helper (the Exif IFD pointer value must equal the byte offset where the Exif IFD count begins: 38).

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat(image): EXIF metadata and orientation reading, test JPEG builders

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Decode with orientation, and thumbnails

**Files:**
- Create: `crates/pixie-image/src/decode.rs`, `crates/pixie-image/src/thumbs.rs`
- Modify: `crates/pixie-image/src/lib.rs`

**Interfaces:**
- Consumes: `meta::read_orientation`, `pixie_core::{PixieError, Result}`.
- Produces:
  - `decode::apply_orientation(img: DynamicImage, o: u16) -> DynamicImage`
  - `decode::decode_oriented(path: &Path) -> Result<DynamicImage>` (HEIC routed to `heic::decode`, which is already upright; errors map to `PixieError::Image`/`Unsupported`)
  - `thumbs::ThumbClass { Grid, Viewer }` (`Copy, Eq, Hash`), `ThumbClass::edge() -> u32` (256 / 1600)
  - `thumbs::thumb_path(cache_dir: &Path, class: ThumbClass, photo_id: i64, mtime: i64) -> PathBuf`
  - `thumbs::ensure_thumbnail(cache_dir: &Path, class: ThumbClass, photo_id: i64, mtime: i64, src: &Path) -> Result<PathBuf>`

- [ ] **Step 1: Write the failing tests**

`crates/pixie-image/src/decode.rs`:
```rust
use crate::meta::read_orientation;
use image::DynamicImage;
use pixie_core::{PixieError, Result};
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, Rgb, RgbImage, DynamicImage};
    use pixie_testutil::{jpeg, jpeg_with_exif};

    fn two_tone() -> DynamicImage {
        // 4x2: left half red, right half blue
        let mut i = RgbImage::new(4, 2);
        for y in 0..2 { for x in 0..4 {
            i.put_pixel(x, y, if x < 2 { Rgb([255, 0, 0]) } else { Rgb([0, 0, 255]) });
        }}
        DynamicImage::ImageRgb8(i)
    }

    #[test]
    fn orientation_swaps_dimensions_for_5_to_8() {
        for o in 1..=8u16 {
            let r = apply_orientation(two_tone(), o);
            let swapped = o >= 5;
            assert_eq!(r.dimensions(), if swapped { (2, 4) } else { (4, 2) }, "o={o}");
        }
    }

    #[test]
    fn orientation_6_puts_left_side_on_top() {
        let r = apply_orientation(two_tone(), 6).to_rgb8();
        assert_eq!(r.get_pixel(0, 0).0, [255, 0, 0]); // top = old left = red
        assert_eq!(r.get_pixel(0, 3).0, [0, 0, 255]);
    }

    #[test]
    fn orientation_8_puts_left_side_on_bottom() {
        let r = apply_orientation(two_tone(), 8).to_rgb8();
        assert_eq!(r.get_pixel(0, 3).0, [255, 0, 0]);
        assert_eq!(r.get_pixel(0, 0).0, [0, 0, 255]);
    }

    #[test]
    fn decode_oriented_applies_exif() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.jpg");
        std::fs::write(&p, jpeg_with_exif(40, 20, 6, None)).unwrap();
        let img = decode_oriented(&p).unwrap();
        assert_eq!(img.dimensions(), (20, 40));
    }

    #[test]
    fn corrupt_and_empty_files_error_instead_of_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.jpg");
        std::fs::write(&bad, b"not a jpeg").unwrap();
        assert!(matches!(decode_oriented(&bad), Err(PixieError::Image(_))));
        let empty = dir.path().join("empty.jpg");
        std::fs::write(&empty, b"").unwrap();
        assert!(decode_oriented(&empty).is_err());
        let plain = dir.path().join("ok.jpg");
        std::fs::write(&plain, jpeg(8, 8)).unwrap();
        assert!(decode_oriented(&plain).is_ok());
    }
}
```

`crates/pixie-image/src/thumbs.rs`:
```rust
use crate::decode::decode_oriented;
use image::{GenericImageView, ImageFormat};
use pixie_core::{PixieError, Result};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_testutil::{jpeg, jpeg_with_exif};

    #[test]
    fn thumbnail_is_downscaled_cached_and_keyed_by_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.jpg");
        std::fs::write(&src, jpeg(1000, 500)).unwrap();
        let cache = dir.path().join("cache");

        let t1 = ensure_thumbnail(&cache, ThumbClass::Grid, 7, 111, &src).unwrap();
        assert!(t1.starts_with(&cache) && t1.exists());
        let (w, h) = image::open(&t1).unwrap().dimensions();
        assert_eq!((w, h), (256, 128));

        // Same key -> same file, not regenerated even if the source vanishes.
        std::fs::remove_file(&src).unwrap();
        assert_eq!(ensure_thumbnail(&cache, ThumbClass::Grid, 7, 111, &src).unwrap(), t1);

        // New mtime -> different path (replaced file gets a fresh thumbnail).
        assert_ne!(thumb_path(&cache, ThumbClass::Grid, 7, 222), t1);
    }

    #[test]
    fn small_images_are_not_upscaled() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("s.jpg");
        std::fs::write(&src, jpeg(100, 50)).unwrap();
        let t = ensure_thumbnail(&dir.path().join("c"), ThumbClass::Grid, 1, 1, &src).unwrap();
        assert_eq!(image::open(&t).unwrap().dimensions(), (100, 50));
    }

    #[test]
    fn exif_rotated_photo_gets_upright_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("r.jpg");
        std::fs::write(&src, jpeg_with_exif(400, 200, 6, None)).unwrap();
        let t = ensure_thumbnail(&dir.path().join("c"), ThumbClass::Grid, 2, 1, &src).unwrap();
        let img = image::open(&t).unwrap();
        let (w, h) = img.dimensions();
        assert!(h > w, "expected portrait after rotation, got {w}x{h}");
        // top of the portrait is the old left side (red), bottom is blue
        let rgb = img.to_rgb8();
        assert!(rgb.get_pixel(w / 2, 5)[0] > 200);
        assert!(rgb.get_pixel(w / 2, h - 5)[2] > 200);
    }

    #[test]
    fn corrupt_source_returns_error_and_leaves_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("bad.jpg");
        std::fs::write(&src, b"junk").unwrap();
        let cache = dir.path().join("c");
        assert!(ensure_thumbnail(&cache, ThumbClass::Grid, 3, 1, &src).is_err());
        assert!(!thumb_path(&cache, ThumbClass::Grid, 3, 1).exists());
    }
}
```
- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p pixie-image`
Expected: compile errors for undefined items.

- [ ] **Step 3: Implement `decode.rs`**

Insert above tests:
```rust
fn is_heif(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("heic") | Some("heif")
    )
}

pub fn apply_orientation(img: DynamicImage, o: u16) -> DynamicImage {
    match o {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

/// Decode and return the image upright. HEIC is already upright from libheif.
pub fn decode_oriented(path: &Path) -> Result<DynamicImage> {
    if is_heif(path) {
        #[cfg(feature = "heic")]
        return crate::heic::decode(path);
        #[cfg(not(feature = "heic"))]
        return Err(PixieError::Unsupported("HEIC support not compiled in".into()));
    }
    let img = image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()
        .map_err(|e| PixieError::Image(e.to_string()))?;
    Ok(apply_orientation(img, read_orientation(path)))
}
```

- [ ] **Step 4: Implement `thumbs.rs`**

Insert above tests:
```rust
pub const GRID_EDGE: u32 = 256;
pub const VIEWER_EDGE: u32 = 1600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbClass {
    Grid,
    Viewer,
}

impl ThumbClass {
    pub fn edge(self) -> u32 {
        match self { ThumbClass::Grid => GRID_EDGE, ThumbClass::Viewer => VIEWER_EDGE }
    }
    fn dir(self) -> &'static str {
        match self { ThumbClass::Grid => "grid", ThumbClass::Viewer => "view" }
    }
}

pub fn thumb_path(cache_dir: &Path, class: ThumbClass, photo_id: i64, mtime: i64) -> PathBuf {
    cache_dir.join(class.dir()).join(format!("{photo_id}-{mtime}.jpg"))
}

/// Returns the cached thumbnail, generating it atomically if missing.
pub fn ensure_thumbnail(
    cache_dir: &Path,
    class: ThumbClass,
    photo_id: i64,
    mtime: i64,
    src: &Path,
) -> Result<PathBuf> {
    let dst = thumb_path(cache_dir, class, photo_id, mtime);
    if dst.exists() {
        return Ok(dst);
    }
    let img = decode_oriented(src)?;
    let edge = class.edge();
    let (w, h) = img.dimensions();
    let thumb = if w <= edge && h <= edge { img } else { img.thumbnail(edge, edge) };

    std::fs::create_dir_all(dst.parent().expect("thumb path has parent"))?;
    // Unique temp name per thread so concurrent callers never clobber each other.
    let tmp = dst.with_extension(format!("{:?}.tmp", std::thread::current().id()).replace(['(', ')'], ""));
    thumb
        .to_rgb8()
        .save_with_format(&tmp, ImageFormat::Jpeg)
        .map_err(|e| PixieError::Image(e.to_string()))?;
    std::fs::rename(&tmp, &dst)?;
    Ok(dst)
}
```
Update `lib.rs`:
```rust
pub mod decode;
pub mod meta;
pub mod thumbs;
#[cfg(feature = "heic")]
pub mod heic;
```
(Do not enable `heic` yet; creating `heic.rs` is Task 8. Remove the `#[cfg(feature = "heic")] pub mod heic;` line from lib.rs from Task 3 if duplicated.)

- [ ] **Step 5: Run tests**

Run: `cargo test -p pixie-image`
Expected: all pass (meta 4 + decode 5 + thumbs 4).

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(image): oriented decode and cached thumbnails

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: pixie-index database

**Files:**
- Create: `crates/pixie-index/Cargo.toml`, `crates/pixie-index/src/lib.rs`, `crates/pixie-index/src/db.rs`

**Interfaces:**
- Consumes: `pixie_core::{Folder, FolderId, Photo, PhotoId, PhotoMeta, PixieError, Result}`.
- Produces: `Db` (`Clone`, cheap; every method locks briefly):
  - `Db::open(path: &Path) -> Result<Db>`, `Db::open_in_memory() -> Result<Db>`
  - `add_folder(&self, path: &str) -> Result<FolderId>` (idempotent)
  - `get_folder(&self, id: FolderId) -> Result<Option<Folder>>`, `list_folders(&self) -> Result<Vec<Folder>>`
  - `find_photo_stat(&self, path: &str) -> Result<Option<PhotoStat>>` where `PhotoStat { id: PhotoId, size: i64, mtime: i64, missing: bool }`
  - `upsert_photo(&self, folder_id: FolderId, path: &str, size: i64, mtime: i64, meta: &PhotoMeta) -> Result<PhotoId>` (sets `missing = 0`)
  - `get_photo(&self, id: PhotoId) -> Result<Option<Photo>>`
  - `mark_missing_except(&self, folder_id: FolderId, seen: &HashSet<String>) -> Result<usize>`
  - `list_photos(&self, offset: i64, limit: i64) -> Result<Vec<Photo>>` (non-missing, newest `taken_at` first, ties by id desc), `count_photos(&self) -> Result<i64>` (non-missing)

- [ ] **Step 1: Manifest**

`crates/pixie-index/Cargo.toml`:
```toml
[package]
name = "pixie-index"
version.workspace = true
edition.workspace = true

[dependencies]
pixie-core = { path = "../pixie-core" }
pixie-image = { path = "../pixie-image" }
rusqlite = { workspace = true }
walkdir = { workspace = true }
serde = { workspace = true }

[dev-dependencies]
pixie-testutil = { path = "../pixie-testutil" }
tempfile = { workspace = true }
```

`crates/pixie-index/src/lib.rs`:
```rust
pub mod db;
pub mod scanner;

pub use db::{Db, PhotoStat};
pub use scanner::{scan_folder, ScanProgress, ScanStats};
```
(Create an empty `scanner.rs` containing only `use serde::Serialize;` plus placeholder structs in Task 6; for now comment out the `scanner` lines in `lib.rs` and re-enable in Task 6.)

- [ ] **Step 2: Write the failing tests**

`crates/pixie-index/src/db.rs`:
```rust
use pixie_core::{Folder, FolderId, Photo, PhotoId, PhotoMeta, PixieError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(taken: Option<i64>) -> PhotoMeta {
        PhotoMeta { width: Some(10), height: Some(5), taken_at: taken, ..Default::default() }
    }

    #[test]
    fn migrations_run_once_and_reopen_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.db");
        let db = Db::open(&p).unwrap();
        db.add_folder("/a").unwrap();
        drop(db);
        let db = Db::open(&p).unwrap(); // reopening must not re-run V1
        assert_eq!(db.list_folders().unwrap().len(), 1);
    }

    #[test]
    fn opening_a_newer_database_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.db");
        Connection::open(&p).unwrap().pragma_update(None, "user_version", 999).unwrap();
        assert!(matches!(Db::open(&p), Err(PixieError::Db(_))));
    }

    #[test]
    fn add_folder_is_idempotent() {
        let db = Db::open_in_memory().unwrap();
        let a = db.add_folder("/pics").unwrap();
        assert_eq!(db.add_folder("/pics").unwrap(), a);
        assert_eq!(db.get_folder(a).unwrap().unwrap().path, "/pics");
        assert!(db.get_folder(999).unwrap().is_none());
    }

    #[test]
    fn upsert_inserts_then_updates_same_row_and_clears_missing() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let id = db.upsert_photo(f, "/p/a.jpg", 100, 5, &meta(Some(1000))).unwrap();
        let id2 = db.upsert_photo(f, "/p/a.jpg", 200, 6, &meta(Some(2000))).unwrap();
        assert_eq!(id, id2);
        let p = db.get_photo(id).unwrap().unwrap();
        assert_eq!((p.size, p.mtime, p.meta.taken_at, p.missing), (200, 6, Some(2000), false));
        let st = db.find_photo_stat("/p/a.jpg").unwrap().unwrap();
        assert_eq!((st.id, st.size, st.mtime, st.missing), (id, 200, 6, false));
        assert!(db.find_photo_stat("/p/zzz.jpg").unwrap().is_none());
    }

    #[test]
    fn same_filename_in_different_folders_are_distinct() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let a = db.upsert_photo(f, "/p/x/IMG_1.jpg", 1, 1, &meta(None)).unwrap();
        let b = db.upsert_photo(f, "/p/y/IMG_1.jpg", 1, 1, &meta(None)).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn mark_missing_hides_from_list_and_upsert_restores() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let g = db.add_folder("/q").unwrap();
        db.upsert_photo(f, "/p/a.jpg", 1, 1, &meta(Some(1))).unwrap();
        db.upsert_photo(f, "/p/b.jpg", 1, 1, &meta(Some(2))).unwrap();
        db.upsert_photo(g, "/q/c.jpg", 1, 1, &meta(Some(3))).unwrap();
        let seen: HashSet<String> = ["/p/a.jpg".to_string()].into();
        assert_eq!(db.mark_missing_except(f, &seen).unwrap(), 1);
        assert_eq!(db.count_photos().unwrap(), 2); // b hidden; other folder untouched
        db.upsert_photo(f, "/p/b.jpg", 1, 1, &meta(Some(2))).unwrap();
        assert_eq!(db.count_photos().unwrap(), 3);
    }

    #[test]
    fn list_orders_newest_first_with_null_dates_last_and_pages() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        for (n, t) in [("a", Some(10)), ("b", Some(30)), ("c", Some(20)), ("d", None)] {
            db.upsert_photo(f, &format!("/p/{n}.jpg"), 1, 1, &meta(t)).unwrap();
        }
        let all: Vec<String> = db.list_photos(0, 10).unwrap().into_iter().map(|p| p.path).collect();
        assert_eq!(all, ["/p/b.jpg", "/p/c.jpg", "/p/a.jpg", "/p/d.jpg"]);
        let page: Vec<String> = db.list_photos(1, 2).unwrap().into_iter().map(|p| p.path).collect();
        assert_eq!(page, ["/p/c.jpg", "/p/a.jpg"]);
        assert!(db.list_photos(100, 10).unwrap().is_empty());
    }

    #[test]
    fn unicode_and_spaces_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/Ferien 2024").unwrap();
        let id = db.upsert_photo(f, "/Ferien 2024/Café ☕ 1.jpg", 1, 1, &meta(None)).unwrap();
        assert_eq!(db.get_photo(id).unwrap().unwrap().path, "/Ferien 2024/Café ☕ 1.jpg");
    }
}
```
Add `tempfile` is already a dev-dependency.

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p pixie-index`
Expected: compile errors (`Db` undefined).

- [ ] **Step 4: Implement**

Insert above the tests in `db.rs`:
```rust
fn db_err<E: std::fmt::Display>(e: E) -> PixieError {
    PixieError::Db(e.to_string())
}

const MIGRATIONS: &[&str] = &[
    // V1
    "CREATE TABLE folders (
        id   INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE
     );
     CREATE TABLE photos (
        id        INTEGER PRIMARY KEY,
        folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
        path      TEXT NOT NULL UNIQUE,
        size      INTEGER NOT NULL,
        mtime     INTEGER NOT NULL,
        width     INTEGER,
        height    INTEGER,
        taken_at  INTEGER,
        camera    TEXT,
        gps_lat   REAL,
        gps_lon   REAL,
        missing   INTEGER NOT NULL DEFAULT 0
     );
     CREATE INDEX idx_photos_list   ON photos(missing, taken_at DESC, id DESC);
     CREATE INDEX idx_photos_folder ON photos(folder_id);",
];

#[derive(Debug, Clone, PartialEq)]
pub struct PhotoStat {
    pub id: PhotoId,
    pub size: i64,
    pub mtime: i64,
    pub missing: bool,
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

const PHOTO_COLS: &str =
    "id, folder_id, path, size, mtime, width, height, taken_at, camera, gps_lat, gps_lon, missing";

fn row_to_photo(r: &rusqlite::Row) -> rusqlite::Result<Photo> {
    Ok(Photo {
        id: r.get(0)?,
        folder_id: r.get(1)?,
        path: r.get(2)?,
        size: r.get(3)?,
        mtime: r.get(4)?,
        meta: PhotoMeta {
            width: r.get(5)?,
            height: r.get(6)?,
            taken_at: r.get(7)?,
            camera: r.get(8)?,
            gps_lat: r.get(9)?,
            gps_lon: r.get(10)?,
        },
        missing: r.get::<_, i64>(11)? != 0,
    })
}

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        Self::init(Connection::open(path).map_err(db_err)?)
    }

    pub fn open_in_memory() -> Result<Db> {
        Self::init(Connection::open_in_memory().map_err(db_err)?)
    }

    fn init(mut conn: Connection) -> Result<Db> {
        conn.pragma_update(None, "journal_mode", "WAL").map_err(db_err)?;
        conn.pragma_update(None, "foreign_keys", "ON").map_err(db_err)?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(db_err)?;
        let known = MIGRATIONS.len() as i64;
        if version > known {
            return Err(PixieError::Db(format!(
                "library was created by a newer Pixie (schema {version}, this build knows {known})"
            )));
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = conn.transaction().map_err(db_err)?;
            tx.execute_batch(sql).map_err(db_err)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64).map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }
        Ok(Db { conn: Arc::new(Mutex::new(conn)) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn add_folder(&self, path: &str) -> Result<FolderId> {
        let c = self.conn();
        c.execute("INSERT OR IGNORE INTO folders(path) VALUES (?1)", params![path]).map_err(db_err)?;
        c.query_row("SELECT id FROM folders WHERE path = ?1", params![path], |r| r.get(0)).map_err(db_err)
    }

    pub fn get_folder(&self, id: FolderId) -> Result<Option<Folder>> {
        self.conn()
            .query_row("SELECT id, path FROM folders WHERE id = ?1", params![id], |r| {
                Ok(Folder { id: r.get(0)?, path: r.get(1)? })
            })
            .optional()
            .map_err(db_err)
    }

    pub fn list_folders(&self) -> Result<Vec<Folder>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT id, path FROM folders ORDER BY path").map_err(db_err)?;
        let rows = s
            .query_map([], |r| Ok(Folder { id: r.get(0)?, path: r.get(1)? }))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    pub fn find_photo_stat(&self, path: &str) -> Result<Option<PhotoStat>> {
        self.conn()
            .query_row("SELECT id, size, mtime, missing FROM photos WHERE path = ?1", params![path], |r| {
                Ok(PhotoStat { id: r.get(0)?, size: r.get(1)?, mtime: r.get(2)?, missing: r.get::<_, i64>(3)? != 0 })
            })
            .optional()
            .map_err(db_err)
    }

    pub fn upsert_photo(&self, folder_id: FolderId, path: &str, size: i64, mtime: i64, meta: &PhotoMeta) -> Result<PhotoId> {
        self.conn()
            .query_row(
                "INSERT INTO photos (folder_id, path, size, mtime, width, height, taken_at, camera, gps_lat, gps_lon, missing)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)
                 ON CONFLICT(path) DO UPDATE SET
                    folder_id = excluded.folder_id, size = excluded.size, mtime = excluded.mtime,
                    width = excluded.width, height = excluded.height, taken_at = excluded.taken_at,
                    camera = excluded.camera, gps_lat = excluded.gps_lat, gps_lon = excluded.gps_lon,
                    missing = 0
                 RETURNING id",
                params![folder_id, path, size, mtime, meta.width, meta.height, meta.taken_at, meta.camera, meta.gps_lat, meta.gps_lon],
                |r| r.get(0),
            )
            .map_err(db_err)
    }

    pub fn get_photo(&self, id: PhotoId) -> Result<Option<Photo>> {
        self.conn()
            .query_row(&format!("SELECT {PHOTO_COLS} FROM photos WHERE id = ?1"), params![id], row_to_photo)
            .optional()
            .map_err(db_err)
    }

    pub fn mark_missing_except(&self, folder_id: FolderId, seen: &HashSet<String>) -> Result<usize> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let stale: Vec<PhotoId> = {
            let mut s = tx
                .prepare("SELECT id, path FROM photos WHERE folder_id = ?1 AND missing = 0")
                .map_err(db_err)?;
            let rows = s
                .query_map(params![folder_id], |r| Ok((r.get::<_, PhotoId>(0)?, r.get::<_, String>(1)?)))
                .map_err(db_err)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_err)?;
            rows.into_iter().filter(|(_, p)| !seen.contains(p)).map(|(id, _)| id).collect()
        };
        for id in &stale {
            tx.execute("UPDATE photos SET missing = 1 WHERE id = ?1", params![id]).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(stale.len())
    }

    pub fn list_photos(&self, offset: i64, limit: i64) -> Result<Vec<Photo>> {
        let c = self.conn();
        let mut s = c
            .prepare(&format!(
                "SELECT {PHOTO_COLS} FROM photos WHERE missing = 0
                 ORDER BY taken_at DESC, id DESC LIMIT ?1 OFFSET ?2"
            ))
            .map_err(db_err)?;
        let rows = s
            .query_map(params![limit, offset], row_to_photo)
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    pub fn count_photos(&self) -> Result<i64> {
        self.conn()
            .query_row("SELECT COUNT(*) FROM photos WHERE missing = 0", [], |r| r.get(0))
            .map_err(db_err)
    }
}
```
SQLite sorts NULLs first for ASC and **last for DESC**, so `taken_at DESC` puts NULL dates last, matching the test.

- [ ] **Step 5: Run tests**

Run: `cargo test -p pixie-index`
Expected: 7 passed.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(index): SQLite library with migrations and photo/folder queries

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Folder scanner

**Files:**
- Create: `crates/pixie-index/src/scanner.rs`
- Modify: `crates/pixie-index/src/lib.rs` (re-enable `scanner` exports)

**Interfaces:**
- Consumes: `Db` methods from Task 5, `pixie_image::meta::read_meta`, `pixie_core::is_supported_image`.
- Produces:
  - `ScanStats { added, updated, unchanged, unreadable, marked_missing: usize, cancelled: bool }` (`Default`, `Clone`, `PartialEq`, `Debug`, `Serialize`)
  - `ScanProgress { folder_id: FolderId, seen: usize }` (`Serialize`, `Clone`)
  - `scan_folder(db: &Db, folder_id: FolderId, cancel: &AtomicBool, on_progress: impl FnMut(&ScanProgress)) -> Result<ScanStats>`; `taken_at` falls back to `mtime / 1000` when EXIF has no date.

- [ ] **Step 1: Write the failing tests**

`crates/pixie-index/src/scanner.rs`:
```rust
use crate::db::Db;
use pixie_core::{is_supported_image, FolderId, PixieError, Result};
use pixie_image::meta::read_meta;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_testutil::{jpeg, jpeg_with_exif};
    use std::fs;

    fn setup() -> (tempfile::TempDir, Db, FolderId) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder(dir.path().to_str().unwrap()).unwrap();
        (dir, db, f)
    }

    fn scan(db: &Db, f: FolderId) -> ScanStats {
        scan_folder(db, f, &AtomicBool::new(false), |_| {}).unwrap()
    }

    #[test]
    fn first_scan_adds_second_scan_changes_nothing() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg(20, 10)).unwrap();
        fs::write(dir.path().join("notes.txt"), "hi").unwrap();
        let s1 = scan(&db, f);
        assert_eq!((s1.added, s1.updated, s1.unchanged), (1, 0, 0));
        let s2 = scan(&db, f);
        assert_eq!((s2.added, s2.updated, s2.unchanged), (0, 0, 1));
        assert_eq!(db.count_photos().unwrap(), 1);
    }

    #[test]
    fn photo_without_exif_falls_back_to_mtime_for_date() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("shot.png.jpg"), jpeg(20, 10)).unwrap();
        scan(&db, f);
        let p = &db.list_photos(0, 10).unwrap()[0];
        assert_eq!(p.meta.taken_at, Some(p.mtime / 1000));
    }

    #[test]
    fn exif_date_wins_over_mtime() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg_with_exif(20, 10, 1, Some("2020:01:02 03:04:05"))).unwrap();
        scan(&db, f);
        let p = &db.list_photos(0, 10).unwrap()[0];
        assert_eq!(p.meta.taken_at, Some(1_577_934_245));
    }

    #[test]
    fn corrupt_and_empty_jpgs_are_indexed_and_scan_continues() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("bad.jpg"), b"junk").unwrap();
        fs::write(dir.path().join("empty.jpg"), b"").unwrap();
        fs::write(dir.path().join("good.jpg"), jpeg(20, 10)).unwrap();
        let s = scan(&db, f);
        assert_eq!(s.added, 3);
        assert_eq!(s.unreadable, 2);
        assert_eq!(db.count_photos().unwrap(), 3);
    }

    #[test]
    fn same_name_in_subfolders_unicode_paths_and_resource_forks() {
        let (dir, db, f) = setup();
        for sub in ["x", "y", "Ferien 2024/Café ☕"] {
            fs::create_dir_all(dir.path().join(sub)).unwrap();
            fs::write(dir.path().join(sub).join("IMG_1.jpg"), jpeg(8, 8)).unwrap();
        }
        fs::write(dir.path().join("x").join("._IMG_1.jpg"), b"appledouble").unwrap();
        let s = scan(&db, f);
        assert_eq!(s.added, 3);
        assert_eq!(db.count_photos().unwrap(), 3);
    }

    #[test]
    fn replaced_file_is_reindexed() {
        let (dir, db, f) = setup();
        let p = dir.path().join("a.jpg");
        fs::write(&p, jpeg(20, 10)).unwrap();
        scan(&db, f);
        fs::write(&p, jpeg(200, 100)).unwrap(); // different size
        let s = scan(&db, f);
        assert_eq!((s.added, s.updated), (0, 1));
        assert_eq!(db.list_photos(0, 1).unwrap()[0].meta.width, Some(200));
    }

    #[test]
    fn deleted_file_is_marked_missing_not_removed() {
        let (dir, db, f) = setup();
        let p = dir.path().join("a.jpg");
        fs::write(&p, jpeg(20, 10)).unwrap();
        fs::write(dir.path().join("b.jpg"), jpeg(20, 10)).unwrap();
        scan(&db, f);
        fs::remove_file(&p).unwrap();
        let s = scan(&db, f);
        assert_eq!(s.marked_missing, 1);
        assert_eq!(db.count_photos().unwrap(), 1);
        // reappears -> restored
        fs::write(&p, jpeg(20, 10)).unwrap();
        scan(&db, f);
        assert_eq!(db.count_photos().unwrap(), 2);
    }

    #[test]
    fn unreachable_root_errors_and_marks_nothing_missing() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg(20, 10)).unwrap();
        scan(&db, f);
        let root = dir.path().to_path_buf();
        drop(dir); // "unplug the drive": directory disappears
        assert!(!root.exists());
        let r = scan_folder(&db, f, &AtomicBool::new(false), |_| {});
        assert!(matches!(r, Err(PixieError::NotFound(_))));
        assert_eq!(db.count_photos().unwrap(), 1); // still listed, not missing
    }

    #[test]
    fn cancel_stops_early_without_marking_missing() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg(20, 10)).unwrap();
        scan(&db, f);
        let s = scan_folder(&db, f, &AtomicBool::new(true), |_| {}).unwrap();
        assert!(s.cancelled);
        assert_eq!(s.marked_missing, 0);
        assert_eq!(db.count_photos().unwrap(), 1);
    }

    #[test]
    fn progress_is_reported_and_final_count_matches() {
        let (dir, db, f) = setup();
        for i in 0..250 {
            fs::write(dir.path().join(format!("{i}.jpg")), jpeg(8, 8)).unwrap();
        }
        let mut last = 0;
        scan_folder(&db, f, &AtomicBool::new(false), |p| last = p.seen).unwrap();
        assert_eq!(last, 250);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p pixie-index`
Expected: compile errors (`scan_folder`, `ScanStats` undefined).

- [ ] **Step 3: Implement**

Insert above tests in `scanner.rs`:
```rust
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct ScanStats {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub unreadable: usize,
    pub marked_missing: usize,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    pub folder_id: FolderId,
    pub seen: usize,
}

fn mtime_ms(md: &std::fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Incrementally scan a watched folder. Never touches originals.
/// If the root is unreachable, returns `NotFound` and changes nothing.
pub fn scan_folder(
    db: &Db,
    folder_id: FolderId,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(&ScanProgress),
) -> Result<ScanStats> {
    let folder = db
        .get_folder(folder_id)?
        .ok_or_else(|| PixieError::NotFound(format!("folder {folder_id}")))?;
    let root = Path::new(&folder.path);
    if !root.is_dir() {
        return Err(PixieError::NotFound(folder.path.clone()));
    }

    let mut stats = ScanStats::default();
    let mut seen: HashSet<String> = HashSet::new();

    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        if cancel.load(Ordering::Relaxed) {
            stats.cancelled = true;
            return Ok(stats);
        }
        if !entry.file_type().is_file() || !is_supported_image(entry.path()) {
            continue;
        }
        let Some(path) = entry.path().to_str().map(str::to_owned) else { continue };
        seen.insert(path.clone());

        let Ok(md) = entry.metadata() else {
            stats.unreadable += 1;
            continue;
        };
        let (size, mtime) = (md.len() as i64, mtime_ms(&md));

        match db.find_photo_stat(&path)? {
            Some(s) if s.size == size && s.mtime == mtime && !s.missing => stats.unchanged += 1,
            existing => {
                let mut meta = read_meta(entry.path());
                if meta.width.is_none() {
                    stats.unreadable += 1;
                }
                if meta.taken_at.is_none() {
                    meta.taken_at = Some(mtime / 1000);
                }
                db.upsert_photo(folder_id, &path, size, mtime, &meta)?;
                if existing.is_some() { stats.updated += 1 } else { stats.added += 1 }
            }
        }
        if seen.len() % 100 == 0 {
            on_progress(&ScanProgress { folder_id, seen: seen.len() });
        }
    }

    stats.marked_missing = db.mark_missing_except(folder_id, &seen)?;
    on_progress(&ScanProgress { folder_id, seen: seen.len() });
    Ok(stats)
}
```
In `crates/pixie-index/src/lib.rs` ensure both `pub mod scanner;` and the `pub use` lines are active.

- [ ] **Step 4: Run tests**

Run: `cargo test -p pixie-index`
Expected: 16 passed (7 db + 9 scanner).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(index): incremental folder scanner with missing-file handling

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Background thumbnail service

**Files:**
- Create: `crates/pixie-image/src/service.rs`
- Modify: `crates/pixie-image/src/lib.rs` (`pub mod service;`)

**Interfaces:**
- Consumes: `thumbs::{ensure_thumbnail, ThumbClass}`.
- Produces: `ThumbService` (`Clone`; shares workers):
  - `ThumbService::new(cache_dir: PathBuf, workers: usize) -> ThumbService`
  - `request(&self, class: ThumbClass, photo_id: i64, mtime: i64, src: PathBuf) -> std::sync::mpsc::Receiver<Result<PathBuf>>`
  - Behavior: newest request is served first (LIFO, so the rows the user is looking at win); duplicate requests for the same key share one generation; workers exit when the last clone is dropped.

- [ ] **Step 1: Write the failing tests**

`crates/pixie-image/src/service.rs`:
```rust
use crate::thumbs::{ensure_thumbnail, ThumbClass};
use pixie_core::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_testutil::jpeg;
    use std::time::Duration;

    fn wait(rx: Receiver<Result<PathBuf>>) -> Result<PathBuf> {
        rx.recv_timeout(Duration::from_secs(10)).expect("thumbnail result in time")
    }

    #[test]
    fn generates_thumbnail_for_request() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.jpg");
        std::fs::write(&src, jpeg(600, 300)).unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 2);
        let out = wait(svc.request(ThumbClass::Grid, 1, 1, src)).unwrap();
        assert!(out.exists());
    }

    #[test]
    fn duplicate_requests_share_one_result() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.jpg");
        std::fs::write(&src, jpeg(600, 300)).unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 1);
        let a = svc.request(ThumbClass::Grid, 1, 1, src.clone());
        let b = svc.request(ThumbClass::Grid, 1, 1, src);
        assert_eq!(wait(a).unwrap(), wait(b).unwrap());
    }

    #[test]
    fn corrupt_file_yields_error_and_service_keeps_working() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.jpg");
        std::fs::write(&bad, b"junk").unwrap();
        let good = dir.path().join("good.jpg");
        std::fs::write(&good, jpeg(100, 100)).unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 1);
        assert!(wait(svc.request(ThumbClass::Grid, 1, 1, bad)).is_err());
        assert!(wait(svc.request(ThumbClass::Grid, 2, 1, good)).is_ok());
    }

    #[test]
    fn many_requests_all_complete() {
        let dir = tempfile::tempdir().unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 4);
        let mut rxs = vec![];
        for i in 0..40 {
            let p = dir.path().join(format!("{i}.jpg"));
            std::fs::write(&p, jpeg(300, 200)).unwrap();
            rxs.push(svc.request(ThumbClass::Grid, i, 1, p));
        }
        for rx in rxs {
            assert!(wait(rx).is_ok());
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p pixie-image service`
Expected: compile error (`ThumbService` undefined).

- [ ] **Step 3: Implement**

Insert above tests:
```rust
type Reply = Sender<Result<PathBuf>>;
type Key = (ThumbClass, i64, i64);

struct Pending {
    src: PathBuf,
    replies: Vec<Reply>,
}

#[derive(Default)]
struct State {
    stack: Vec<Key>,
    pending: HashMap<Key, Pending>,
    shutdown: bool,
}

struct Inner {
    cache_dir: PathBuf,
    state: Mutex<State>,
    cv: Condvar,
}

/// Dropping the last clone stops the workers.
#[derive(Clone)]
pub struct ThumbService {
    inner: Arc<Inner>,
    _guard: Arc<Guard>,
}

struct Guard(Arc<Inner>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.state.lock().unwrap_or_else(|p| p.into_inner()).shutdown = true;
        self.0.cv.notify_all();
    }
}

impl ThumbService {
    pub fn new(cache_dir: PathBuf, workers: usize) -> ThumbService {
        let inner = Arc::new(Inner { cache_dir, state: Mutex::new(State::default()), cv: Condvar::new() });
        for _ in 0..workers.max(1) {
            let inner = inner.clone();
            std::thread::spawn(move || worker(inner));
        }
        ThumbService { _guard: Arc::new(Guard(inner.clone())), inner }
    }

    pub fn request(&self, class: ThumbClass, photo_id: i64, mtime: i64, src: PathBuf) -> Receiver<Result<PathBuf>> {
        let (tx, rx) = channel();
        let key = (class, photo_id, mtime);
        let mut st = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        match st.pending.get_mut(&key) {
            Some(p) => p.replies.push(tx),
            None => {
                st.pending.insert(key, Pending { src, replies: vec![tx] });
                st.stack.push(key);
                self.inner.cv.notify_one();
            }
        }
        rx
    }
}

fn worker(inner: Arc<Inner>) {
    loop {
        let (key, src) = {
            let mut st = inner.state.lock().unwrap_or_else(|p| p.into_inner());
            loop {
                if st.shutdown {
                    return;
                }
                if let Some(key) = st.stack.pop() {
                    let src = st.pending[&key].src.clone();
                    break (key, src);
                }
                st = inner.cv.wait(st).unwrap_or_else(|p| p.into_inner());
            }
        };
        let (class, id, mtime) = key;
        let result = ensure_thumbnail(&inner.cache_dir, class, id, mtime, &src);
        let replies = inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pending
            .remove(&key)
            .map(|p| p.replies)
            .unwrap_or_default();
        for r in replies {
            let _ = r.send(result.clone());
        }
    }
}
```
Add `pub mod service;` to `lib.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p pixie-image`
Expected: all pass (previous 13 + 4 service).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(image): background thumbnail worker pool (LIFO, deduplicated)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: HEIC support (build risk, decide here)

This task resolves the spec's open item on `libheif`. **Decision point:** if after about two hours of effort the Windows build cannot link libheif, stop and report to the user with the exact error; the fallback is to ship v1 without HEIC (the app already degrades: HEIC files are indexed, thumbnails return `Unsupported`, UI shows the placeholder) and revisit.

**Files:**
- Create: `crates/pixie-image/src/heic.rs`, `crates/pixie-image/tests/heic.rs`, `crates/pixie-image/tests/fixtures/sample.heic`
- Modify: `crates/pixie-image/src/lib.rs`, `src-tauri/Cargo.toml`, `.github/workflows/ci.yml` (Task 12)

**Interfaces:**
- Produces: `heic::decode(path: &Path) -> Result<DynamicImage>` (already upright), `heic::dimensions(path: &Path) -> Option<(u32, u32)>`.

- [ ] **Step 1: Install libheif**

Windows (vcpkg):
```powershell
git clone https://github.com/microsoft/vcpkg C:\vcpkg
C:\vcpkg\bootstrap-vcpkg.bat
C:\vcpkg\vcpkg install "libheif[core]:x64-windows"
setx VCPKG_ROOT C:\vcpkg
setx VCPKGRS_TRIPLET x64-windows
setx VCPKGRS_DYNAMIC 1
```
(Open a new shell afterwards.) macOS: `brew install libheif pkg-config`.

Note for the user's information (not blocking): HEIC from iPhones is HEVC; the vcpkg libheif bundles `libde265` (LGPL). Distribution licensing/patent posture for HEVC should be reviewed before public release.

- [ ] **Step 2: Add a fixture**

Copy a small HEIC (an iPhone photo, or `examples/example.heic` from the strukturag/libheif repository) to `crates/pixie-image/tests/fixtures/sample.heic`. Prefer one with a visible orientation (portrait iPhone photo), under 1 MB.

- [ ] **Step 3: Write the failing integration test**

`crates/pixie-image/tests/heic.rs`:
```rust
#![cfg(feature = "heic")]
use image::GenericImageView;
use pixie_image::{decode::decode_oriented, meta::read_meta, thumbs::{ensure_thumbnail, ThumbClass}};
use std::path::Path;

const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample.heic");

#[test]
fn heic_decodes_and_metadata_matches_decoded_dimensions() {
    let img = decode_oriented(Path::new(SAMPLE)).expect("decode heic");
    let meta = read_meta(Path::new(SAMPLE));
    // Guards against double-applying rotation (libheif already rotates).
    assert_eq!((meta.width, meta.height), (Some(img.width()), Some(img.height())));
}

#[test]
fn heic_thumbnail_is_generated() {
    let dir = tempfile::tempdir().unwrap();
    let t = ensure_thumbnail(dir.path(), ThumbClass::Grid, 1, 1, Path::new(SAMPLE)).unwrap();
    let (w, h) = image::open(&t).unwrap().dimensions();
    assert!(w.max(h) <= 256 && w.max(h) > 0);
}
```

- [ ] **Step 4: Run to verify failure**

Run: `cargo test -p pixie-image --features heic --test heic`
Expected: compile error (`pixie_image::heic` missing) or link error if libheif is not found.

- [ ] **Step 5: Implement**

`crates/pixie-image/src/heic.rs`:
```rust
use image::{DynamicImage, RgbImage};
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use pixie_core::{PixieError, Result};
use std::path::Path;

fn err<E: std::fmt::Display>(e: E) -> PixieError {
    PixieError::Image(e.to_string())
}

fn open(path: &Path) -> Result<HeifContext<'static>> {
    let p = path.to_str().ok_or_else(|| PixieError::Io("non-UTF-8 path".into()))?;
    HeifContext::read_from_file(p).map_err(err)
}

/// Decode the primary image. libheif applies rotation/mirroring transforms itself.
pub fn decode(path: &Path) -> Result<DynamicImage> {
    let ctx = open(path)?;
    let handle = ctx.primary_image_handle().map_err(err)?;
    let img = LibHeif::new()
        .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)
        .map_err(err)?;
    let planes = img.planes();
    let plane = planes.interleaved.ok_or_else(|| PixieError::Image("no interleaved plane".into()))?;
    let (w, h, stride) = (plane.width as usize, plane.height as usize, plane.stride);
    let mut buf = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        buf.extend_from_slice(&plane.data[y * stride..y * stride + w * 3]);
    }
    RgbImage::from_raw(w as u32, h as u32, buf)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PixieError::Image("bad pixel buffer".into()))
}

pub fn dimensions(path: &Path) -> Option<(u32, u32)> {
    let ctx = open(path).ok()?;
    let h = ctx.primary_image_handle().ok()?;
    Some((h.width(), h.height()))
}
```
Ensure `lib.rs` contains exactly one `#[cfg(feature = "heic")] pub mod heic;`.

If the libheif-rs API differs from the above (it has changed across 1.x), consult `cargo doc -p libheif-rs --open` and adapt only the bodies of these two functions; the signatures are what the rest of the plan relies on.

- [ ] **Step 6: Run tests**

Run: `cargo test -p pixie-image --features heic`
Expected: all pass including the 2 HEIC tests. Also run `cargo test -p pixie-image` (no feature) and confirm it still passes.

- [ ] **Step 7: Make the app use it**

In `src-tauri/Cargo.toml` (this is wired fully in Task 9; add now):
```toml
[features]
default = ["heic"]
heic = ["pixie-image/heic"]
```

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat(image): HEIC decoding via libheif behind the heic feature

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Tauri commands and `pixie://` protocol

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`
- Create: `src-tauri/src/state.rs`

**Interfaces:**
- Consumes: `Db`, `scan_folder`, `ThumbService`, `ThumbClass`.
- Produces commands (names are the contract with the UI, args camelCase in JS):
  - `add_folder(path: String) -> Result<i64, PixieError>`
  - `list_folders() -> Result<Vec<Folder>, PixieError>`
  - `start_scan(folder_id: i64) -> Result<(), PixieError>`; emits events `scan-progress` (`ScanProgress`), `scan-done` (`{ folder_id, stats }`), `scan-error` (`{ folder_id, message }`)
  - `cancel_scan()`
  - `list_photos(offset: i64, limit: i64) -> Result<Vec<Photo>, PixieError>`, `count_photos() -> Result<i64, PixieError>`
  - `print_page() -> Result<(), String>` (opens the system print dialog for the webview; used in Task 11)
  - Protocol `pixie://localhost/{grid|view}/{photo_id}?v={mtime}` (Windows: `http://pixie.localhost/...`) returns `image/jpeg`, 404 for unknown photo or class, 500 with empty body when the thumbnail cannot be generated.

- [ ] **Step 1: Dependencies**

Replace the `[dependencies]` section of `src-tauri/Cargo.toml`:
```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
serde = { workspace = true }
serde_json = "1"
pixie-core = { path = "../crates/pixie-core" }
pixie-image = { path = "../crates/pixie-image" }
pixie-index = { path = "../crates/pixie-index" }
```
(keep the `[features]` block from Task 8, or add it now.)

- [ ] **Step 2: State**

`src-tauri/src/state.rs`:
```rust
use pixie_image::service::ThumbService;
use pixie_index::Db;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub thumbs: ThumbService,
    pub cancel_scan: Arc<AtomicBool>,
}
```

- [ ] **Step 3: Commands and protocol**

`src-tauri/src/lib.rs`:
```rust
mod state;

use pixie_core::{Folder, Photo, PixieError};
use pixie_image::thumbs::ThumbClass;
use pixie_index::{scan_folder, Db};
use serde::Serialize;
use state::AppState;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::http::{Response, StatusCode};
use tauri::{Emitter, Manager};

#[tauri::command]
fn add_folder(state: tauri::State<AppState>, path: String) -> Result<i64, PixieError> {
    state.db.add_folder(&path)
}

#[tauri::command]
fn list_folders(state: tauri::State<AppState>) -> Result<Vec<Folder>, PixieError> {
    state.db.list_folders()
}

#[tauri::command]
fn list_photos(state: tauri::State<AppState>, offset: i64, limit: i64) -> Result<Vec<Photo>, PixieError> {
    state.db.list_photos(offset.max(0), limit.clamp(1, 1000))
}

#[tauri::command]
fn count_photos(state: tauri::State<AppState>) -> Result<i64, PixieError> {
    state.db.count_photos()
}

#[derive(Serialize, Clone)]
struct ScanDone {
    folder_id: i64,
    stats: pixie_index::ScanStats,
}

#[derive(Serialize, Clone)]
struct ScanErrorEvent {
    folder_id: i64,
    message: String,
}

#[tauri::command]
fn start_scan(app: tauri::AppHandle, state: tauri::State<AppState>, folder_id: i64) -> Result<(), PixieError> {
    let db = state.db.clone();
    let cancel = state.cancel_scan.clone();
    cancel.store(false, Ordering::Relaxed);
    std::thread::spawn(move || {
        let progress_app = app.clone();
        match scan_folder(&db, folder_id, &cancel, |p| {
            let _ = progress_app.emit("scan-progress", p);
        }) {
            Ok(stats) => {
                let _ = app.emit("scan-done", ScanDone { folder_id, stats });
            }
            Err(e) => {
                let _ = app.emit("scan-error", ScanErrorEvent { folder_id, message: e.user_message() });
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn cancel_scan(state: tauri::State<AppState>) {
    state.cancel_scan.store(true, Ordering::Relaxed);
}

#[tauri::command]
fn print_page(window: tauri::WebviewWindow) -> Result<(), String> {
    window.print().map_err(|e| e.to_string())
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder().status(code).body(Vec::new()).unwrap()
}

/// `/{grid|view}/{photo_id}` -> JPEG thumbnail bytes.
fn handle_thumb(state: &AppState, path: &str) -> Response<Vec<u8>> {
    let mut parts = path.trim_matches('/').split('/');
    let class = match parts.next() {
        Some("grid") => ThumbClass::Grid,
        Some("view") => ThumbClass::Viewer,
        _ => return status(StatusCode::NOT_FOUND),
    };
    let Some(id) = parts.next().and_then(|s| s.parse::<i64>().ok()) else {
        return status(StatusCode::NOT_FOUND);
    };
    let photo = match state.db.get_photo(id) {
        Ok(Some(p)) => p,
        Ok(None) => return status(StatusCode::NOT_FOUND),
        Err(_) => return status(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let rx = state.thumbs.request(class, photo.id, photo.mtime, PathBuf::from(&photo.path));
    match rx.recv() {
        Ok(Ok(file)) => match std::fs::read(file) {
            Ok(bytes) => Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "image/jpeg")
                .header("Cache-Control", "public, max-age=31536000, immutable")
                .header("Access-Control-Allow-Origin", "*")
                .body(bytes)
                .unwrap(),
            Err(_) => status(StatusCode::INTERNAL_SERVER_ERROR),
        },
        _ => status(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("pixie", |ctx, request, responder| {
            let state = ctx.app_handle().state::<AppState>().inner().clone();
            let path = request.uri().path().to_string();
            // Generation can take a while; never block the protocol thread.
            std::thread::spawn(move || responder.respond(handle_thumb(&state, &path)));
        })
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("pixie.db"))?;
            let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
            let thumbs = pixie_image::service::ThumbService::new(dir.join("thumbs"), workers);
            app.manage(AppState { db, thumbs, cancel_scan: Arc::new(Default::default()) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_folder, list_folders, list_photos, count_photos, start_scan, cancel_scan, print_page
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pixie");
}
```
`Db::open(...)?` inside `setup` needs `PixieError` to convert into `Box<dyn Error>`. Add to `pixie-core` `impl std::error::Error` is already provided by `thiserror`; `?` works because `PixieError: Error + 'static`.

- [ ] **Step 4: Build**

Run: `cargo build -p pixie-app`
Expected: `Finished`. (If `window.print()` is not found on your Tauri 2 version, check `cargo doc -p tauri` for the current method name on `WebviewWindow`; Task 11 needs it.)

- [ ] **Step 5: Run workspace tests**

Run: `cargo test --workspace`
Expected: all prior tests still pass (HEIC tests included because `pixie-app` enables the feature through the workspace; if libheif is not installed on this machine, run `cargo test --workspace --exclude pixie-app` instead).

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(app): Tauri commands, scan events and pixie:// thumbnail protocol

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Library UI (folders, virtualized grid, viewer)

**Files:**
- Create: `ui/src/lib/api.ts`, `ui/src/lib/url.ts`, `ui/src/lib/gridMath.ts`, `ui/src/lib/url.test.ts`, `ui/src/lib/gridMath.test.ts`, `ui/src/lib/Grid.svelte`, `ui/src/lib/Viewer.svelte`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: the Task 9 commands and events.
- Produces: `thumbUrl(cls: 'grid'|'view', id: number, mtime: number, isWindows?: boolean): string`; `columnsFor(width, cell, gap): number`; `visibleRange(total, cols, rowH, scrollTop, viewH, overscan?) -> { firstIndex, endIndex, rows }`.

- [ ] **Step 1: Write the failing unit tests**

`ui/src/lib/url.test.ts`:
```ts
import { describe, expect, it } from 'vitest';
import { thumbUrl } from './url';

describe('thumbUrl', () => {
  it('uses the http host form on Windows', () => {
    expect(thumbUrl('grid', 12, 345, true)).toBe('http://pixie.localhost/grid/12?v=345');
  });
  it('uses the custom scheme elsewhere', () => {
    expect(thumbUrl('view', 7, 1, false)).toBe('pixie://localhost/view/7?v=1');
  });
});
```

`ui/src/lib/gridMath.test.ts`:
```ts
import { describe, expect, it } from 'vitest';
import { columnsFor, visibleRange } from './gridMath';

describe('columnsFor', () => {
  it('fits as many cells as possible, at least 1', () => {
    expect(columnsFor(800, 180, 6)).toBe(4);
    expect(columnsFor(50, 180, 6)).toBe(1);
  });
});

describe('visibleRange', () => {
  it('returns an empty range for no photos', () => {
    expect(visibleRange(0, 4, 186, 0, 600, 2)).toEqual({ firstIndex: 0, endIndex: 0, rows: 0 });
  });
  it('covers the viewport at the top', () => {
    expect(visibleRange(1000, 4, 186, 0, 600, 0)).toEqual({ firstIndex: 0, endIndex: 20, rows: 250 });
  });
  it('applies overscan and clamps at the end', () => {
    expect(visibleRange(1000, 4, 186, 0, 600, 2).endIndex).toBe(28);
    expect(visibleRange(10, 4, 186, 0, 600, 2)).toEqual({ firstIndex: 0, endIndex: 10, rows: 3 });
  });
  it('starts at the scrolled row', () => {
    expect(visibleRange(1000, 4, 186, 1860, 600, 0).firstIndex).toBe(40);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `npm --prefix ui test`
Expected: FAIL, modules not found.

- [ ] **Step 3: Implement the pure modules**

`ui/src/lib/url.ts`:
```ts
export type ThumbClass = 'grid' | 'view';

export function thumbUrl(
  cls: ThumbClass,
  id: number,
  mtime: number,
  isWindows: boolean = /Windows/i.test(navigator.userAgent),
): string {
  const base = isWindows ? 'http://pixie.localhost' : 'pixie://localhost';
  return `${base}/${cls}/${id}?v=${mtime}`;
}
```

`ui/src/lib/gridMath.ts`:
```ts
export function columnsFor(width: number, cell: number, gap: number): number {
  return Math.max(1, Math.floor((width + gap) / (cell + gap)));
}

export function visibleRange(
  total: number,
  cols: number,
  rowH: number,
  scrollTop: number,
  viewH: number,
  overscan = 2,
): { firstIndex: number; endIndex: number; rows: number } {
  if (total <= 0) return { firstIndex: 0, endIndex: 0, rows: 0 };
  const rows = Math.ceil(total / cols);
  const firstRow = Math.max(0, Math.floor(scrollTop / rowH) - overscan);
  const lastRow = Math.min(rows - 1, Math.ceil((scrollTop + viewH) / rowH) + overscan);
  return { firstIndex: firstRow * cols, endIndex: Math.min(total, (lastRow + 1) * cols), rows };
}
```

- [ ] **Step 4: Run tests**

Run: `npm --prefix ui test`
Expected: 6 passed.

- [ ] **Step 5: API wrapper**

`ui/src/lib/api.ts`:
```ts
import { invoke } from '@tauri-apps/api/core';

export interface Photo {
  id: number; folder_id: number; path: string; size: number; mtime: number;
  width: number | null; height: number | null; taken_at: number | null;
  camera: string | null; gps_lat: number | null; gps_lon: number | null; missing: boolean;
}
export interface Folder { id: number; path: string }

export const listFolders = () => invoke<Folder[]>('list_folders');
export const addFolder = (path: string) => invoke<number>('add_folder', { path });
export const startScan = (folderId: number) => invoke<void>('start_scan', { folderId });
export const cancelScan = () => invoke<void>('cancel_scan');
export const listPhotos = (offset: number, limit: number) => invoke<Photo[]>('list_photos', { offset, limit });
export const countPhotos = () => invoke<number>('count_photos');
export const printPage = () => invoke<void>('print_page');
```

- [ ] **Step 6: Grid**

`ui/src/lib/Grid.svelte`:
```svelte
<script lang="ts">
  import { listPhotos, type Photo } from './api';
  import { columnsFor, visibleRange } from './gridMath';
  import { thumbUrl } from './url';

  let { total, version, onopen }: { total: number; version: number; onopen: (index: number) => void } = $props();

  const CELL = 180, GAP = 6, PAGE = 200, ROW_H = CELL + GAP;
  let width = $state(800), height = $state(600), scrollTop = $state(0);
  let cache = $state<Record<number, Photo>>({});
  let broken = $state<Record<number, boolean>>({});
  const requested = new Set<number>();

  const cols = $derived(columnsFor(width, CELL, GAP));
  const range = $derived(visibleRange(total, cols, ROW_H, scrollTop, height));
  const indices = $derived(Array.from({ length: range.endIndex - range.firstIndex }, (_, i) => range.firstIndex + i));

  $effect(() => {
    version; // reset when the library changes
    cache = {}; broken = {}; requested.clear();
  });

  $effect(() => {
    if (range.endIndex === 0) return;
    for (let page = Math.floor(range.firstIndex / PAGE); page <= Math.floor((range.endIndex - 1) / PAGE); page++) {
      if (requested.has(page)) continue;
      requested.add(page);
      listPhotos(page * PAGE, PAGE).then((photos) => {
        const next = { ...cache };
        photos.forEach((p, i) => (next[page * PAGE + i] = p));
        cache = next;
      }).catch(() => requested.delete(page));
    }
  });
</script>

<div class="scroller" bind:clientWidth={width} bind:clientHeight={height}
     onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
  <div class="inner" style:height="{range.rows * ROW_H}px">
    {#each indices as i (i)}
      {@const p = cache[i]}
      <button class="cell" style:top="{Math.floor(i / cols) * ROW_H}px" style:left="{(i % cols) * (CELL + GAP)}px"
              onclick={() => onopen(i)} aria-label={p ? p.path : 'photo'}>
        {#if p && !broken[i]}
          <img loading="lazy" alt="" src={thumbUrl('grid', p.id, p.mtime)} onerror={() => (broken[i] = true)} />
        {:else if broken[i]}
          <span class="ph">⚠︎</span>
        {/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .scroller { height: 100%; overflow-y: auto; }
  .inner { position: relative; }
  .cell { position: absolute; width: 180px; height: 180px; padding: 0; border: 0; background: #e8e8ec; border-radius: 6px; overflow: hidden; cursor: pointer; }
  img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .ph { display: grid; place-items: center; height: 100%; color: #888; font-size: 28px; }
</style>
```

- [ ] **Step 7: Viewer**

`ui/src/lib/Viewer.svelte`:
```svelte
<script lang="ts">
  import { listPhotos, type Photo } from './api';
  import { thumbUrl } from './url';

  let { index, total, onclose, onnav }: {
    index: number; total: number; onclose: () => void; onnav: (i: number) => void;
  } = $props();

  let photo = $state<Photo | null>(null);
  let failed = $state(false);

  $effect(() => {
    failed = false; photo = null;
    listPhotos(index, 1).then((r) => (photo = r[0] ?? null));
  });

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
    else if (e.key === 'ArrowRight' && index < total - 1) onnav(index + 1);
    else if (e.key === 'ArrowLeft' && index > 0) onnav(index - 1);
  }
</script>

<svelte:window onkeydown={key} />
<div class="overlay" role="dialog" aria-label="Photo viewer">
  <button class="close" onclick={onclose} aria-label="Close">✕</button>
  {#if photo && !failed}
    <img alt="" src={thumbUrl('view', photo.id, photo.mtime)} onerror={() => (failed = true)} />
  {:else if failed}
    <p>This photo couldn't be opened. It may be damaged.</p>
  {/if}
  <div class="count">{index + 1} / {total}</div>
</div>

<style>
  .overlay { position: fixed; inset: 0; background: #111; display: grid; place-items: center; color: #eee; }
  img { max-width: 100%; max-height: 100%; object-fit: contain; }
  .close { position: absolute; top: 12px; right: 16px; background: none; border: 0; color: #eee; font-size: 22px; cursor: pointer; }
  .count { position: absolute; bottom: 10px; font-size: 13px; opacity: .7; }
</style>
```

- [ ] **Step 8: App shell**

`ui/src/App.svelte`:
```svelte
<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import { addFolder, cancelScan, countPhotos, listFolders, startScan } from './lib/api';
  import Grid from './lib/Grid.svelte';
  import Viewer from './lib/Viewer.svelte';
  import PrintTest from './lib/PrintTest.svelte';

  let total = $state(0), version = $state(0);
  let scanning = $state(false), seen = $state(0), message = $state('');
  let openIndex = $state<number | null>(null);
  let view = $state<'library' | 'print'>('library');

  async function refresh() { total = await countPhotos(); version++; }

  async function chooseFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== 'string') return;
    const id = await addFolder(dir);
    scanning = true; seen = 0; message = '';
    await startScan(id);
  }

  onMount(() => {
    refresh();
    // Re-scan known folders on launch to pick up changes.
    listFolders().then((fs) => { if (fs[0]) { scanning = true; startScan(fs[0].id); } });
    const subs = [
      listen<{ seen: number }>('scan-progress', (e) => { seen = e.payload.seen; refresh(); }),
      listen('scan-done', () => { scanning = false; refresh(); }),
      listen<{ message: string }>('scan-error', (e) => { scanning = false; message = e.payload.message; }),
    ];
    return () => subs.forEach((s) => s.then((un) => un()));
  });
</script>

<main>
  <header>
    <strong>Pixie</strong>
    <button onclick={chooseFolder}>Add photo folder…</button>
    <button onclick={() => (view = view === 'print' ? 'library' : 'print')}>Print test</button>
    {#if scanning}
      <span>Scanning… {seen} photos</span>
      <button onclick={() => cancelScan()}>Stop</button>
    {:else}
      <span>{total} photos</span>
    {/if}
    {#if message}<span class="err">{message}</span>{/if}
  </header>
  <section>
    {#if view === 'print'}
      <PrintTest />
    {:else if total === 0 && !scanning}
      <p class="empty">Add a folder of photos to get started.</p>
    {:else}
      <Grid {total} {version} onopen={(i) => (openIndex = i)} />
    {/if}
  </section>
  {#if openIndex !== null}
    <Viewer index={openIndex} {total} onclose={() => (openIndex = null)} onnav={(i) => (openIndex = i)} />
  {/if}
</main>

<style>
  :global(body) { margin: 0; font-family: system-ui, sans-serif; }
  main { display: flex; flex-direction: column; height: 100vh; }
  header { display: flex; gap: 12px; align-items: center; padding: 10px 14px; border-bottom: 1px solid #ddd; }
  section { flex: 1; min-height: 0; padding: 8px; }
  .empty { text-align: center; margin-top: 20vh; color: #666; }
  .err { color: #b00020; }
</style>
```
Note: the placeholder `PrintTest.svelte` is created in Task 11; create an empty component now (`<p>print test</p>`) so this step compiles.

- [ ] **Step 9: Type-check and build**

Run: `npm --prefix ui run check && npm --prefix ui run build`
Expected: no errors.

- [ ] **Step 10: Manual end-to-end check**

Run: `npm run tauri dev`. In the window: click "Add photo folder…", choose a folder of real photos (include a HEIC, a rotated phone photo, and rename a text file to `broken.jpg`). Expected: count rises while scanning; grid fills with thumbnails; the broken file shows a ⚠︎ placeholder; clicking opens the viewer; ←/→ navigate; Esc closes; restarting the app shows the same photos immediately.

- [ ] **Step 11: Commit**

```bash
git add -A && git commit -m "feat(ui): library grid, viewer, folder picker and scan progress

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Print test page and findings

**Files:**
- Create: `ui/src/lib/PrintTest.svelte` (replace placeholder), `docs/print-findings.md`

**Interfaces:**
- Consumes: `printPage()` from `api.ts` (Task 9 `print_page`).
- Produces: a 4×6 in test sheet with 1-inch grid and a 0.25-inch inset frame, and a recorded verdict on whether WebView2 (Windows) and WKWebView (macOS) print at true size. Later print work (Milestone 5) depends on this verdict.

- [ ] **Step 1: Print sheet component**

`ui/src/lib/PrintTest.svelte`:
```svelte
<script lang="ts">
  import { printPage } from './api';
  let error = $state('');
  async function go() {
    error = '';
    try { await printPage(); } catch (e) { error = String(e); }
  }
</script>

<div class="controls">
  <button onclick={go}>Print 4×6 test sheet</button>
  <span>Print to PDF and to a real printer at "100% / actual size", then measure the grid squares.</span>
  {#if error}<span class="err">{error}</span>{/if}
</div>

<div class="sheet">
  <div class="frame"></div>
  <div class="label tl">TL</div><div class="label tr">TR</div>
  <div class="label bl">BL</div><div class="label br">BR</div>
  <div class="note">Each grid square is 1in × 1in.<br />Frame is inset 0.25in from every edge.</div>
</div>

<style>
  .controls { display: flex; gap: 12px; align-items: center; padding: 8px 0; }
  .err { color: #b00020; }
  .sheet {
    position: relative; width: 4in; height: 6in; background-color: #fff; color: #000;
    background-image:
      linear-gradient(to right, #7aa7ff 1px, transparent 1px),
      linear-gradient(to bottom, #7aa7ff 1px, transparent 1px);
    background-size: 1in 1in;
    outline: 1px solid #aaa; box-sizing: border-box;
  }
  .frame { position: absolute; inset: 0.25in; border: 2px solid #e00; box-sizing: border-box; }
  .label { position: absolute; font: 700 12px monospace; }
  .tl { top: .3in; left: .3in; } .tr { top: .3in; right: .3in; }
  .bl { bottom: .3in; left: .3in; } .br { bottom: .3in; right: .3in; }
  .note { position: absolute; top: 2.4in; left: .4in; font: 12px system-ui; }

  @media print {
    @page { size: 4in 6in; margin: 0; }
    :global(header), .controls { display: none !important; }
    :global(body), :global(main), :global(section) { margin: 0 !important; padding: 0 !important; height: auto !important; }
    .sheet { outline: none; print-color-adjust: exact; -webkit-print-color-adjust: exact; }
  }
</style>
```

- [ ] **Step 2: Build check**

Run: `npm --prefix ui run check && npm --prefix ui run build`
Expected: no errors.

- [ ] **Step 3: Manual test on Windows**

Run `npm run tauri dev`, click "Print test", click "Print 4×6 test sheet". (a) Choose "Microsoft Print to PDF", then open the PDF and verify page size is 4×6 in and the page count is 1; with a PDF viewer's measure tool or by printing at 100% confirm grid squares are 1 in. (b) Print on a real printer if available and measure with a ruler. Note: scaling setting, margins applied, page count, colour of the blue grid.

- [ ] **Step 4: Manual test on macOS**

Repeat Step 3 on a Mac (or a macOS CI/VM if no Mac is available; if neither exists, record "macOS not yet verified" and flag it to the user as an open risk).

- [ ] **Step 5: Record findings**

Create `docs/print-findings.md` with one section per platform containing: date, OS and webview version, whether `window.print()` opened the system dialog, measured square size, page size/page count, margin behaviour, and the verdict **"true-size printing: works / needs workaround"**. If either platform fails, add the workaround to try in Milestone 5 (generate a PDF in Rust at exact page size and open it with the system viewer for printing).

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(ui): 4x6 print test sheet and recorded print findings

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 12: CI and scan benchmark

**Files:**
- Create: `.github/workflows/ci.yml`, `crates/pixie-index/examples/bench_scan.rs`

**Interfaces:**
- Consumes: `scan_folder`, `Db`, `ThumbService`.
- Produces: green CI on Windows and macOS; `cargo run --release -p pixie-index --example bench_scan -- 50000` prints scan time, and thumbnail throughput for the first 500 photos.

- [ ] **Step 1: Benchmark example**

`crates/pixie-index/examples/bench_scan.rs` (add `pixie-testutil` and `tempfile` are already dev-dependencies; add `pixie-image` is already a dependency):
```rust
use pixie_image::{service::ThumbService, thumbs::ThumbClass};
use pixie_index::{scan_folder, Db};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(50_000);
    let dir = tempfile::tempdir().unwrap();
    let sample = pixie_testutil::jpeg(1600, 1200);
    for i in 0..n {
        let sub = dir.path().join(format!("d{:03}", i % 200));
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(format!("{i}.jpg")), &sample).unwrap();
    }

    let db = Db::open_in_memory().unwrap();
    let f = db.add_folder(dir.path().to_str().unwrap()).unwrap();

    let t = Instant::now();
    let s = scan_folder(&db, f, &AtomicBool::new(false), |_| {}).unwrap();
    println!("first scan:  {n} files in {:.1}s  ({s:?})", t.elapsed().as_secs_f64());

    let t = Instant::now();
    scan_folder(&db, f, &AtomicBool::new(false), |_| {}).unwrap();
    println!("rescan:      {:.1}s (nothing changed)", t.elapsed().as_secs_f64());

    let cache: PathBuf = dir.path().join("cache");
    let svc = ThumbService::new(cache, std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
    let photos = db.list_photos(0, 500).unwrap();
    let t = Instant::now();
    let rxs: Vec<_> = photos
        .iter()
        .map(|p| svc.request(ThumbClass::Grid, p.id, p.mtime, PathBuf::from(&p.path)))
        .collect();
    for rx in rxs { rx.recv().unwrap().unwrap(); }
    let secs = t.elapsed().as_secs_f64();
    println!("thumbnails:  {} in {:.1}s ({:.0}/s)", photos.len(), secs, photos.len() as f64 / secs);
}
```

- [ ] **Step 2: Run the benchmark**

Run: `cargo run --release -p pixie-index --example bench_scan -- 50000`
Expected: completes. Record the three numbers in the commit message. Targets for a first pass: first scan under ~2 minutes, rescan under ~15 seconds, and the thumbnail rate reported (a 1600×1200 JPEG should be well over 20/s with 4+ cores). If the rescan is slow, the cause is the per-file `find_photo_stat` query; batch it before proceeding to Milestone 2.

- [ ] **Step 3: CI workflow**

`.github/workflows/ci.yml`:
```yaml
name: ci
on: [push, pull_request]

jobs:
  test:
    strategy:
      fail-fast: false
      matrix:
        os: [windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    env:
      VCPKGRS_TRIPLET: x64-windows
      VCPKGRS_DYNAMIC: "1"
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: actions/setup-node@v4
        with: { node-version: 22 }

      - name: libheif (Windows)
        if: runner.os == 'Windows'
        run: vcpkg install "libheif[core]:x64-windows"
      - name: libheif (macOS)
        if: runner.os == 'macOS'
        run: brew install libheif pkg-config

      - run: npm install
      - run: npm --prefix ui install
      - run: npm --prefix ui test
      - run: npm --prefix ui run check
      - run: npm --prefix ui run build
      - run: cargo test --workspace
      - run: cargo build -p pixie-app
```

- [ ] **Step 4: Verify locally**

Run the same commands in order locally: `npm --prefix ui test && npm --prefix ui run check && npm --prefix ui run build && cargo test --workspace && cargo build -p pixie-app`
Expected: all succeed. (CI itself can only be confirmed after the repository is pushed to GitHub; say so in the hand-off rather than claiming CI is green.)

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "ci: Windows/macOS workflow and 50k-photo scan benchmark

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage (Milestone 1 scope):** project setup (T1); scan, SQLite, EXIF, missing-not-deleted, incremental (T5–T6); thumbnails at two sizes with disk cache keyed by id+mtime, background multi-worker, newest-request-first (T4, T7); HEIC build solved with explicit stop condition (T8); typed plain-language errors (T2, T9); thumbnail grid and viewer (T10); print test page and the WebView2/WebKit risk (T11); CI on both platforms and the 50k benchmark (T12). Deliberately **not** in M1 (belongs to later plans): file watcher while the app is open, search/tags/ratings/duplicates, "missing" UI and re-locate, organize tool, slideshow/print/edit/collage. The spec lists the file watcher under scanning; it is deferred to Milestone 2's plan to keep this one shippable, and re-scan-on-launch (T10) stands in for it until then.

**Placeholder scan:** no TBD/TODO. Two honest conditionals remain and are stated: libheif-rs API drift (T8 step 5) and `WebviewWindow::print` naming (T9 step 4), each with how to resolve.

**Type consistency:** `PhotoMeta`/`Photo`/`Folder` defined in T2 and used unchanged in T3, T5, T6, T9, T10 (TS `Photo` mirrors the flattened serde shape). `ThumbClass`, `ensure_thumbnail`, `thumb_path` signatures (T4) match their use in T7 and T9. `Db` method names (T5) match T6 and T9. `scan_folder` signature (T6) matches T9 and T12. mtime is milliseconds everywhere; `taken_at` seconds everywhere.

**Review Focus coverage:** items 1–5 map to tests in T6 (no-EXIF fallback, corrupt/empty, unreachable root, same-name/unicode/resource-fork, replaced file), T4 (upright EXIF thumbnail), T3 (swapped dimensions), T7 and T10 (broken thumbnail → placeholder).

**Known fragile point to watch during execution:** the hand-built EXIF segment in `pixie-testutil` (T3) is the foundation of several tests; if `kamadak-exif` rejects it, fix the helper first.
