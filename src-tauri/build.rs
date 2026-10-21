//! HEIC support comes from libheif and libde265, which are LGPL, so they ship as DLLs next to the program (a user can
//! swap in their own build). vcpkg installs them; this gathers them into `dlls/`, which `tauri.conf.json` lists as
//! resources, so the installer carries them and `tauri dev` puts them beside the program.
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

fn main() {
    if cfg!(windows) {
        gather_dlls();
    }
    tauri_build::build()
}
