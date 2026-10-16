set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# HEIC support links libheif from this vcpkg clone (see docs/superpowers/plans).
export VCPKG_ROOT := "C:/work/vcpkg-pixie"
export VCPKGRS_TRIPLET := "x64-windows-static-md"

# Launch the app on Windows in dev mode (hot reload for the UI).
winlaunch:
    npm run tauri dev
