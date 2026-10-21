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

    #[cfg(not(windows))]
    #[test]
    fn other_systems_offer_no_extra_roots() {
        assert!(drive_roots().is_empty());
    }
}
