set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# HEIC support links libheif and libde265 (LGPL, so as DLLs) from this vcpkg clone; see README. Windows only: on a Mac
# libheif comes from Homebrew (`brew install libheif pkg-config`).
export VCPKG_ROOT := "C:/work/vcpkg-pixie"
export VCPKGRS_TRIPLET := "x64-windows"
export VCPKGRS_DYNAMIC := "1"
# So tests and cargo run find the DLLs.
export PATH := if os() == "windows" { VCPKG_ROOT / "installed/x64-windows/bin;" + env_var("PATH") } else { env_var("PATH") }

# `tauri dev` rebuilds on every edit. Ctrl-C while it is compiling or linking leaves half-written output, and the next launch
# then fails with "unresolved external symbol anon..." (in whichever of our crates was being built, not just the app). So the
# launch leaves a marker, a normal exit removes it, and a launch that finds it cleans all of our own crates (third-party ones
# stay cached; rebuilding ours takes about a minute and a half).

# Launch the app on Windows in dev mode (hot reload for the UI).
winlaunch:
    New-Item -ItemType Directory -Force target | Out-Null
    if (Test-Path target/.dev-running) { cargo clean -p pixie-analysis -p pixie-core -p pixie-geo -p pixie-image -p pixie-index -p pixie-mail -p pixie-testutil -p pixie-app }
    New-Item -ItemType File -Force target/.dev-running | Out-Null
    -npm run tauri dev
    Remove-Item target/.dev-running -ErrorAction SilentlyContinue

# Launch the app on a Mac in dev mode (hot reload for the UI). Needs Homebrew's libheif and pkg-config for HEIC pictures.
maclaunch:
    npm install
    npm run tauri dev

# Build the Windows installer (a per-user NSIS setup program) and show where it is.
# It fetches Microsoft's Visual C++ runtime on the target machine if that is missing.
win-build:
    npm run tauri build
    Get-ChildItem target/release/bundle/nsis/*-setup.exe | ForEach-Object { Write-Host ("Installer: " + $_.FullName + "  (" + [math]::Round($_.Length / 1MB, 1) + " MB)  SHA256 " + ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([IO.File]::ReadAllBytes($_.FullName))) -replace '-', '')) }

# Build the macOS disk image (Apple silicon, unsigned) on the Mac (`ssh mac`) from what is pushed to origin, copy it to
# target/ here and show its size and checksum. HEIC support comes from a vcpkg clone on the Mac (see README), not Homebrew,
# because Homebrew's libheif drags in x265 (GPL).
mac-build:
    ssh mac 'export PATH=/opt/homebrew/bin:$HOME/.cargo/bin:$PATH PKG_CONFIG_PATH=$HOME/work/vcpkg-pixie/installed/arm64-osx-dynamic/lib/pkgconfig VCPKG_ROOT=$HOME/work/vcpkg-pixie VCPKGRS_TRIPLET=arm64-osx-dynamic; cd ~/work/pixie && git pull --ff-only && npm install && npm --prefix ui install && npm run tauri build'
    scp -o ServerAliveInterval=10 -o ServerAliveCountMax=3 "mac:work/pixie/target/release/bundle/dmg/*.dmg" target/
    Get-ChildItem target/*.dmg | ForEach-Object { Write-Host ("Disk image: " + $_.FullName + "  (" + [math]::Round($_.Length / 1MB, 1) + " MB)  SHA256 " + (Get-FileHash $_.FullName -Algorithm SHA256).Hash) }
