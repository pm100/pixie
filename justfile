set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# HEIC support links libheif and libde265 (LGPL, so as DLLs) from this vcpkg clone; see README. Windows only: on a Mac
# libheif comes from Homebrew (`brew install libheif pkg-config`).
export VCPKG_ROOT := "C:/work/vcpkg-pixie"
export VCPKGRS_TRIPLET := "x64-windows"
export VCPKGRS_DYNAMIC := "1"
# So tests and cargo run find the DLLs.
export PATH := if os() == "windows" { VCPKG_ROOT / "installed/x64-windows/bin;" + env_var("PATH") } else { env_var("PATH") }

# Launch the app on Windows in dev mode (hot reload for the UI). `tauri dev` also rebuilds and restarts the app whenever Rust code changes.
winlaunch:
    npm run tauri dev

# Rust changes are not picked up until it is restarted, so use it while someone else is editing the Rust code, or while something long
# (a sync, a load) must not be interrupted.

# Launch the app on Windows in dev mode without watching the Rust code (the UI still hot reloads).
winlaunch-quiet:
    npm run tauri dev -- --no-watch

# An interrupted build can leave half-written output in whichever of our crates was being built, which shows as "unresolved external
# symbol anon..." on the next launch. Cleaning our crates fixes it (third-party ones stay cached; rebuilding ours takes about a minute and
# a half).

# Clean our own crates, then launch as winlaunch does: run it if a launch fails with unresolved external symbols.
winlaunch-clean:
    cargo clean -p pixie-analysis -p pixie-core -p pixie-geo -p pixie-image -p pixie-index -p pixie-mail -p pixie-testutil -p pixie-app
    npm run tauri dev

# Launch the app on a Mac in dev mode (hot reload for the UI). Needs Homebrew's libheif and pkg-config for HEIC pictures.
maclaunch:
    npm install
    npm run tauri dev

# Build the Windows installer (a per-user NSIS setup program), copy it to builds/ and show where it is.
# It fetches Microsoft's Visual C++ runtime on the target machine if that is missing.
win-build:
    npm run tauri build
    New-Item -ItemType Directory -Force builds | Out-Null
    Copy-Item target/release/bundle/nsis/*-setup.exe builds/ -Force
    Get-ChildItem builds/*-setup.exe | ForEach-Object { Write-Host ("Installer: " + $_.FullName + "  (" + [math]::Round($_.Length / 1MB, 1) + " MB)  SHA256 " + ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([IO.File]::ReadAllBytes($_.FullName))) -replace '-', '')) }

# Build the macOS disk image (Apple silicon, unsigned) on the Mac (`ssh mac`) from what is pushed to origin, copy it to
# target/ here and show its size and checksum. HEIC support comes from a vcpkg clone on the Mac (see README), not Homebrew,
# because Homebrew's libheif drags in x265 (GPL).
mac-build:
    ssh mac 'export PATH=/opt/homebrew/bin:$HOME/.cargo/bin:$PATH PKG_CONFIG_PATH=$HOME/work/vcpkg-pixie/installed/arm64-osx-dynamic/lib/pkgconfig VCPKG_ROOT=$HOME/work/vcpkg-pixie VCPKGRS_TRIPLET=arm64-osx-dynamic; cd ~/work/pixie && git pull --ff-only && npm install && npm --prefix ui install && npm run tauri build'
    scp -o ServerAliveInterval=10 -o ServerAliveCountMax=3 "mac:work/pixie/target/release/bundle/dmg/*.dmg" target/
    Get-ChildItem target/*.dmg | ForEach-Object { Write-Host ("Disk image: " + $_.FullName + "  (" + [math]::Round($_.Length / 1MB, 1) + " MB)  SHA256 " + (Get-FileHash $_.FullName -Algorithm SHA256).Hash) }

# Rebuild THIRD-PARTY-NOTICES.md and what Settings, About lists (the licences of everything Pixie uses). Run it when
# dependencies change, then commit the result. Needs `cargo install cargo-about --locked --features cli`.
notices:
    node scripts/notices.mjs

# Ask Pixie's server (ask-proxy/, a Cloudflare Worker named pixie-ask). It holds the API key, the daily limit and the fixed rules; what
# the model is told about the app (the guide, the places a button can lead to) travels with each question from the app, so the app's
# own tests cover it and old and new versions of Pixie can be in use at once. Redeploy only when ask-proxy/ itself changes.

# Run the server's tests and type check.
ask-test:
    npm --prefix ask-proxy install
    npm --prefix ask-proxy run typecheck
    npm --prefix ask-proxy test

# Run the server on this computer (http://localhost:8787); point Pixie at it with PIXIE_ASK_URL=http://localhost:8787/v1/ask. Secrets go in ask-proxy/.dev.vars.
ask-dev:
    npm --prefix ask-proxy install
    npm --prefix ask-proxy run dev

# Test, then deploy the server to Cloudflare, live for every copy of Pixie at once (needs `npx wrangler login` once and the secrets set once; see ask-proxy/README.md).
ask-deploy: ask-test
    npm --prefix ask-proxy run deploy
