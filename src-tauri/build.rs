//! HEIC support comes from libheif and libde265, which are LGPL, so they ship as DLLs next to the program (a user can
//! swap in their own build). vcpkg installs them; this gathers them into `dlls/`, which `tauri.conf.json` lists as
//! resources, so the installer carries them and `tauri dev` puts them beside the program.
//!
//! On a Mac the same two libraries come from a vcpkg clone (`libheif[core,libde265]:arm64-osx-dynamic`, so no GPL x265)
//! and are gathered into `dylibs/`; `tauri.macos.conf.json` lists them as frameworks, and the program is linked with an
//! rpath to `Contents/Frameworks` so it finds them there. Homebrew's libheif is not bundled: it pulls in x265 (GPL).
use std::path::{Path, PathBuf};

fn gather_dlls() {
    println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
    println!("cargo:rerun-if-env-changed=VCPKGRS_TRIPLET");
    let out = Path::new("dlls");
    let _ = std::fs::create_dir_all(out);
    let (Ok(root), Ok(triplet)) = (std::env::var("VCPKG_ROOT"), std::env::var("VCPKGRS_TRIPLET")) else {
        println!("cargo:warning=VCPKG_ROOT / VCPKGRS_TRIPLET not set: not bundling the HEIC DLLs");
        return;
    };
    let bin: PathBuf = [root.as_str(), "installed", triplet.as_str(), "bin"].iter().collect();
    let Ok(entries) = std::fs::read_dir(&bin) else {
        println!("cargo:warning=no {}: not bundling the HEIC DLLs", bin.display());
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dll")) {
            let to = out.join(p.file_name().unwrap());
            if let Err(err) = std::fs::copy(&p, &to) {
                println!("cargo:warning=cannot copy {}: {err}", p.display());
            }
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
}

fn gather_dylibs() {
    println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
    println!("cargo:rerun-if-env-changed=VCPKGRS_TRIPLET");
    let out = Path::new("dylibs");
    let _ = std::fs::create_dir_all(out);
    let (Ok(root), Ok(triplet)) = (std::env::var("VCPKG_ROOT"), std::env::var("VCPKGRS_TRIPLET")) else {
        println!("cargo:warning=VCPKG_ROOT / VCPKGRS_TRIPLET not set: not bundling the HEIC dylibs");
        return;
    };
    let lib: PathBuf = [root.as_str(), "installed", triplet.as_str(), "lib"].iter().collect();
    let Ok(entries) = std::fs::read_dir(&lib) else {
        println!("cargo:warning=no {}: not bundling the HEIC dylibs", lib.display());
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        // The real files only (libheif.1.23.6.dylib), not the libheif.1.dylib / libheif.dylib links to them: the program
        // and libheif ask for the full name.
        let real = std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_file());
        if real && p.extension().is_some_and(|x| x == "dylib") {
            let to = out.join(p.file_name().unwrap());
            if let Err(err) = std::fs::copy(&p, &to) {
                println!("cargo:warning=cannot copy {}: {err}", p.display());
            }
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
}

/// The commit this was built from and whether the tree had uncommitted changes, for Settings, About (`PIXIE_COMMIT` is
/// empty when there is no git checkout, such as a source archive).
fn git_info() {
    for p in ["../.git/HEAD", "../.git/refs/heads", "../.git/index", "src", "../crates", "../ui/src"] {
        println!("cargo:rerun-if-changed={p}");
    }
    let git = |args: &[&str]| {
        std::process::Command::new("git").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let commit = git(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_default();
    let modified = !commit.is_empty() && git(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty());
    println!("cargo:rustc-env=PIXIE_COMMIT={commit}");
    println!("cargo:rustc-env=PIXIE_MODIFIED={modified}");
}

fn main() {
    git_info();
    if cfg!(windows) {
        gather_dlls();
    }
    if cfg!(target_os = "macos") {
        gather_dylibs();
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
    }
    tauri_build::build()
}
