set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# HEIC support links libheif from this vcpkg clone (see docs/superpowers/plans).
export VCPKG_ROOT := "C:/work/vcpkg-pixie"
export VCPKGRS_TRIPLET := "x64-windows-static-md"

# Launch the app on Windows in dev mode (hot reload for the UI).
winlaunch:
    npm run tauri dev

# Build the Windows installer (a per-user NSIS setup program) and show where it is.
# It fetches Microsoft's Visual C++ runtime on the target machine if that is missing.
win-build:
    npm run tauri build
    Get-ChildItem target/release/bundle/nsis/*-setup.exe | ForEach-Object { Write-Host ("Installer: " + $_.FullName + "  (" + [math]::Round($_.Length / 1MB, 1) + " MB)  SHA256 " + ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([IO.File]::ReadAllBytes($_.FullName))) -replace '-', '')) }
