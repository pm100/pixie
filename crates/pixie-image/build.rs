fn main() {
    // The static x265 encoder pulled in by libheif uses registry APIs.
    if std::env::var_os("CARGO_FEATURE_HEIC").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
