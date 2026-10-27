//! The computer's drives, for the first-run look-around: built-in drives and mapped network drives. Optical and
//! removable drives are not listed. Nothing here touches a drive's contents except `volume_label`, which a caller runs
//! on a thread of its own, because it can wait for a sleeping disk or an unreachable share.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct DriveRoot {
    /// Such as `E:\`.
    pub root: PathBuf,
    /// A mapped network drive (its contents can be slow or unreachable).
    pub network: bool,
}

/// `GetDriveTypeW` answers 3 for a drive built into (or permanently attached to) the computer.
pub fn is_fixed(drive_type: u32) -> bool {
    drive_type == 3
}

/// ...and 4 for a network drive. Asking does not touch the network.
pub fn is_network(drive_type: u32) -> bool {
    drive_type == 4
}

#[cfg(windows)]
extern "system" {
    fn GetLogicalDrives() -> u32;
    fn GetDriveTypeW(root: *const u16) -> u32;
    fn GetVolumeInformationW(
        root: *const u16, name: *mut u16, name_len: u32, serial: *mut u32, max_component: *mut u32, flags: *mut u32,
        fs_name: *mut u16, fs_name_len: u32,
    ) -> i32;
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The type of drive `letter`, or None when the letter is not in `mask` (the bit set `GetLogicalDrives` returns).
#[cfg(windows)]
fn drive_kind(letter: char, mask: u32) -> Option<u32> {
    let bit = (letter.to_ascii_uppercase() as u8).checked_sub(b'A')?;
    if bit >= 26 || mask & (1 << bit) == 0 {
        return None;
    }
    let root = wide(&format!("{}:\\", letter.to_ascii_uppercase()));
    // SAFETY: `root` is a NUL-terminated UTF-16 string that outlives the call; the call only reads it.
    Some(unsafe { GetDriveTypeW(root.as_ptr()) })
}

/// Every built-in and network drive, in letter order.
#[cfg(windows)]
pub fn drive_roots() -> Vec<DriveRoot> {
    // SAFETY: takes no arguments and only returns a bit mask.
    let mask = unsafe { GetLogicalDrives() };
    ('A'..='Z')
        .filter_map(|c| {
            let kind = drive_kind(c, mask)?;
            (is_fixed(kind) || is_network(kind)).then(|| DriveRoot { root: PathBuf::from(format!("{c}:\\")), network: is_network(kind) })
        })
        .collect()
}

/// The volume's name ("Seagate Expansion Drive"), if it has one. May block: call it off the UI thread.
#[cfg(windows)]
pub fn volume_label(root: &Path) -> Option<String> {
    let path = wide(&root.to_string_lossy());
    let mut name = [0u16; 261];
    // SAFETY: `path` is NUL-terminated and `name` is a writable buffer of the length given; the other outputs are
    // optional and passed as null.
    let ok = unsafe {
        GetVolumeInformationW(path.as_ptr(), name.as_mut_ptr(), name.len() as u32, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0)
    };
    if ok == 0 {
        return None;
    }
    let len = name.iter().position(|c| *c == 0).unwrap_or(name.len());
    let label = String::from_utf16_lossy(&name[..len]);
    (!label.trim().is_empty()).then_some(label)
}

/// What a path's form alone says about where it is: a UNC path (`\\server\share`) is on the network, a path that starts
/// with a drive letter needs the system's say-so (None), anything else is not a Windows network path.
fn network_by_form(path: &str) -> Option<bool> {
    let p = path.strip_prefix(r"\\?\").unwrap_or(path);
    if p.to_ascii_uppercase().starts_with("UNC\\") {
        return Some(true);
    }
    if path.starts_with(r"\\") && !path.starts_with(r"\\?\") {
        return Some(true);
    }
    let mut chars = p.chars();
    match (chars.next(), chars.next()) {
        (Some(c), Some(':')) if c.is_ascii_alphabetic() => None,
        _ => Some(false),
    }
}

/// Is this folder on a network share (a mapped drive or a UNC path)? Such a folder can be slow or unreachable and cannot be
/// watched for changes reliably. Asking does not touch the network.
#[cfg(windows)]
pub fn is_network_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    match network_by_form(&s) {
        Some(answer) => answer,
        None => {
            let p = s.strip_prefix(r"\\?\").unwrap_or(&s);
            let letter = p.chars().next().unwrap_or('C');
            // SAFETY: takes no arguments and only returns a bit mask.
            let mask = unsafe { GetLogicalDrives() };
            drive_kind(letter, mask).is_some_and(is_network)
        }
    }
}

/// The names macOS gives the file systems of network shares.
#[cfg(any(target_os = "macos", test))]
fn network_fs_name(name: &str) -> bool {
    matches!(name, "smbfs" | "nfs" | "afpfs" | "webdav" | "cifs" | "ftp")
}

#[cfg(target_os = "macos")]
pub fn is_network_path(path: &Path) -> bool {
    use std::ffi::{CStr, CString};
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = CString::new(path.as_os_str().as_bytes()) else { return false };
    // SAFETY: `c` is a NUL-terminated path that outlives the call, and `st` is a zeroed `statfs` the call fills in.
    unsafe {
        let mut st: libc::statfs = std::mem::zeroed();
        if libc::statfs(c.as_ptr(), &mut st) != 0 {
            return false;
        }
        network_fs_name(&CStr::from_ptr(st.f_fstypename.as_ptr()).to_string_lossy())
    }
}

/// Other systems: every folder is treated as local.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn is_network_path(_path: &Path) -> bool {
    false
}

/// Other systems: no extra roots (macOS volumes cannot yet be told from network shares).
#[cfg(not(windows))]
pub fn drive_roots() -> Vec<DriveRoot> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn volume_label(_root: &Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_fixed_drives_are_fixed() {
        assert!(is_fixed(3));
        for other in [0, 1, 2, 4, 5, 6] {
            assert!(!is_fixed(other), "type {other} is unknown, removable, network, CD or RAM");
        }
    }

    #[test]
    fn only_network_drives_are_network() {
        assert!(is_network(4));
        for other in [0, 1, 2, 3, 5, 6] {
            assert!(!is_network(other), "type {other}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn drive_roots_lists_fixed_and_network_drives_with_the_kind_marked() {
        let roots = drive_roots();
        eprintln!("drive roots on this machine: {roots:?}");
        let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let c = roots.iter().find(|r| r.root.to_string_lossy().to_uppercase().starts_with(&system.to_uppercase())).expect("the system drive");
        assert!(!c.network);
        assert!(roots.iter().all(|r| r.root.to_string_lossy().ends_with(":\\")), "{roots:?}");
        // something that is not a drive has no label to read
        assert_eq!(volume_label(Path::new("Z:\\no-such-drive")), None);
    }

    /// By hand, read-only: probes every non-system drive on this machine the way the welcome screen does.
    /// `cargo test -p pixie-app --lib live_probe -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_probe_of_this_machines_drives() {
        use pixie_index::suggest::{probe_all_with, probe_root, Probe};
        let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
        let probes: Vec<Probe> = drive_roots()
            .into_iter()
            .filter(|d| !d.root.to_string_lossy().to_uppercase().starts_with(&system))
            .map(|d| {
                let name = format!("{} (network: {})", d.root.display(), d.network);
                Probe { name, run: Box::new(move || probe_root(&d.root, &d.root.to_string_lossy(), "", std::time::Duration::from_secs(6), 50)) }
            })
            .collect();
        let start = std::time::Instant::now();
        probe_all_with(probes, std::time::Duration::from_secs(15), &mut |r| eprintln!("{:>5.1}s  {}  ->  {:?}", start.elapsed().as_secs_f32(), r.name, r.outcome));
    }

    #[test]
    fn a_unc_path_is_on_the_network_and_a_drive_letter_needs_asking() {
        assert_eq!(network_by_form(r"\\nas\photos\2019"), Some(true));
        assert_eq!(network_by_form(r"\\?\UNC\nas\photos"), Some(true));
        assert_eq!(network_by_form(r"\\?\C:\Users\me"), None);
        assert_eq!(network_by_form(r"P:\Photos"), None);
        assert_eq!(network_by_form("/Users/me/Pictures"), Some(false));
    }

    #[test]
    fn network_file_systems_are_told_by_name() {
        for n in ["smbfs", "nfs", "afpfs", "webdav"] {
            assert!(network_fs_name(n), "{n}");
        }
        for n in ["apfs", "hfs", "exfat", "msdos", "ntfs"] {
            assert!(!network_fs_name(n), "{n}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_system_drive_is_not_a_network_path() {
        let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        assert!(!is_network_path(Path::new(&format!("{system}\\Users"))));
        assert!(is_network_path(Path::new(r"\\no-such-server\share")));
    }

    #[cfg(not(windows))]
    #[test]
    fn other_systems_offer_no_extra_roots() {
        assert!(drive_roots().is_empty());
    }
}
