set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# HEIC support links libheif and libde265 (LGPL, so as DLLs) from this vcpkg clone; see README. Windows only: on a Mac
# libheif comes from Homebrew (`brew install libheif pkg-config`).
export VCPKG_ROOT := "C:/work/vcpkg-pixie"
export VCPKGRS_TRIPLET := "x64-windows"
export VCPKGRS_DYNAMIC := "1"
# So tests and cargo run find the DLLs.
export PATH := if os() == "windows" { VCPKG_ROOT / "installed/x64-windows/bin;" + env_var("PATH") } else { env_var("PATH") }

# RUN ON WINDOWS. Launch the app in dev mode (hot reload for the UI). `tauri dev` also rebuilds and restarts the app whenever Rust code changes.
winlaunch:
    npm run tauri dev

# Rust changes are not picked up until it is restarted, so use it while someone else is editing the Rust code, or while something long
# (a sync, a load) must not be interrupted.

# RUN ON WINDOWS. Launch the app in dev mode without watching the Rust code (the UI still hot reloads).
winlaunch-quiet:
    npm run tauri dev -- --no-watch

# An interrupted build can leave half-written output in whichever of our crates was being built, which shows as "unresolved external
# symbol anon..." on the next launch. Cleaning our crates fixes it (third-party ones stay cached; rebuilding ours takes about a minute and
# a half).

# RUN ON WINDOWS. Clean our own crates, then launch as winlaunch does: run it if a launch fails with unresolved external symbols.
winlaunch-clean:
    cargo clean -p pixie-analysis -p pixie-core -p pixie-geo -p pixie-image -p pixie-index -p pixie-mail -p pixie-testutil -p pixie-app
    npm run tauri dev

# RUN ON THE MAC. Launch the app in dev mode (hot reload for the UI). Needs Homebrew's libheif and pkg-config for HEIC pictures.
maclaunch:
    npm install
    npm run tauri dev

# RUN ON WINDOWS. Build the installer (a per-user NSIS setup program), copy it to builds/ and show where it is.
# It fetches Microsoft's Visual C++ runtime on the target machine if that is missing.
[doc("RUN ON WINDOWS. Build the installer (per-user NSIS setup) into builds/ and show its checksum.")]
win-build:
    npm run tauri build
    New-Item -ItemType Directory -Force builds | Out-Null
    Copy-Item target/release/bundle/nsis/*-setup.exe builds/ -Force
    Get-ChildItem builds/*-setup.exe | ForEach-Object { Write-Host ("Installer: " + $_.FullName + "  (" + [math]::Round($_.Length / 1MB, 1) + " MB)  SHA256 " + ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([IO.File]::ReadAllBytes($_.FullName))) -replace '-', '')) }

# RUN ON WINDOWS (it uses ssh and PowerShell). Build the macOS disk image on the Mac (`ssh mac`) from what is pushed to origin (it runs `just mac-release` there), copy it
# to target/ here and show its size and checksum. The disk image is signed with a Developer ID and notarized by Apple, so it
# opens normally; an unsigned one shows "app is damaged" on Apple silicon. See README, Release build.
[doc("RUN ON WINDOWS. Build the signed, notarized macOS disk image on the Mac over ssh, copy it to target/.")]
mac-build:
    ssh mac 'export PATH=/opt/homebrew/bin:$HOME/.cargo/bin:$PATH; cd ~/work/pixie && git pull --ff-only && npm install && npm --prefix ui install && just mac-release'
    scp -o ServerAliveInterval=10 -o ServerAliveCountMax=3 "mac:/tmp/pixie_release/*.dmg" target/
    Get-ChildItem target/*.dmg | ForEach-Object { Write-Host ("Disk image: " + $_.FullName + "  (" + [math]::Round($_.Length / 1MB, 1) + " MB)  SHA256 " + (Get-FileHash $_.FullName -Algorithm SHA256).Hash) }

# RUN ON THE MAC (`just mac-build` on Windows does it over ssh). Build, sign (Developer ID), notarize and staple the macOS disk image.
# Needs on the Mac: the "Developer ID Application" identity in the earring-build keychain, the App Store Connect API key at
# ~/.private_keys/AuthKey_<KeyID>.p8 (both shared with the Ear Ring project, see its AGENTS.md), and HEIC from the vcpkg clone
# (see README; not Homebrew, whose libheif drags in x265, which is GPL). It uses `--bundles app` and makes the disk image
# with hdiutil, because Tauri's own disk image step drives Finder and fails in an ssh session.
# Output: /tmp/pixie_release/Pixie_<version>_aarch64.dmg. Override the identity with DEVID_IDENTITY, the key with APP_STORE_KEY_ID.
[doc("RUN ON THE MAC. Build, sign, notarize and staple the macOS disk image (mac-build runs it over ssh).")]
mac-release:
    #!/bin/sh
    set -eu
    export PATH="$PATH:/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin"
    export PKG_CONFIG_PATH="$HOME/work/vcpkg-pixie/installed/arm64-osx-dynamic/lib/pkgconfig" VCPKG_ROOT="$HOME/work/vcpkg-pixie" VCPKGRS_TRIPLET=arm64-osx-dynamic
    ID="${DEVID_IDENTITY:-Developer ID Application: Paul Moore (9A4PVL859F)}"
    KEY_ID="${APP_STORE_KEY_ID:-W4T73HJBF4}"
    ISSUER="${APP_STORE_ISSUER_ID:-30e7952a-07ae-4893-95c0-3a8cf2db56c4}"
    KEY="$HOME/.private_keys/AuthKey_$KEY_ID.p8"
    [ -f "$KEY" ] || { echo "Missing $KEY" >&2; exit 1; }
    # A plain ssh session cannot use the login keychain, so the identity lives in a keychain with a stored password.
    KC="$HOME/Library/Keychains/earring-build.keychain-db"; KCPASS="$HOME/.config/earring/build-keychain-pass"
    if [ -f "$KCPASS" ] && [ -f "$KC" ]; then security unlock-keychain -p "$(cat "$KCPASS")" "$KC"; fi
    security find-identity -v -p codesigning | grep -qF "$ID" || { echo "Signing identity not found: $ID" >&2; exit 1; }
    cd "{{justfile_directory()}}"
    ver=$(node -p "require('./src-tauri/tauri.conf.json').version")
    echo "Pixie $ver, git $(git rev-parse --short=7 HEAD)"
    APPLE_SIGNING_IDENTITY="$ID" APPLE_API_ISSUER="$ISSUER" APPLE_API_KEY="$KEY_ID" APPLE_API_KEY_PATH="$KEY" npm run tauri build -- --bundles app
    app="target/release/bundle/macos/Pixie.app"
    codesign --verify --deep --strict "$app"
    out=/tmp/pixie_release; stage=$out/stage; dmg="$out/Pixie_${ver}_aarch64.dmg"
    rm -rf "$out"; mkdir -p "$stage"
    cp -R "$app" "$stage/"; ln -s /Applications "$stage/Applications"
    hdiutil create -volname "Pixie" -srcfolder "$stage" -ov -format UDZO "$dmg" >/dev/null
    rm -rf "$stage"
    codesign --force --sign "$ID" --timestamp "$dmg"
    xcrun notarytool submit "$dmg" --key "$KEY" --key-id "$KEY_ID" --issuer "$ISSUER" --wait
    xcrun stapler staple "$dmg"
    spctl -a -vvv -t open --context context:primary-signature "$dmg"
    echo "Built $dmg"

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
