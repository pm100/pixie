use crate::traits::SecretStore;
use pixie_core::{PixieError, Result};

const SERVICE: &str = "Pixie mail";

/// The password lives in the operating system's credential store (Windows Credential Manager,
/// macOS Keychain), keyed by the account's email address. Never in a file or the database.
pub struct OsSecrets;

fn err(_: keyring::Error) -> PixieError {
    PixieError::Invalid("Pixie could not use the system password store.".into())
}

impl SecretStore for OsSecrets {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match keyring::Entry::new(SERVICE, key).map_err(err)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(err(e)),
        }
    }

    fn set(&self, key: &str, secret: &str) -> Result<()> {
        keyring::Entry::new(SERVICE, key).map_err(err)?.set_password(secret).map_err(err)
    }

    fn delete(&self, key: &str) -> Result<()> {
        match keyring::Entry::new(SERVICE, key).map_err(err)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the real credential store; run by hand: `cargo test -p pixie-mail os_secrets -- --ignored`
    #[test]
    #[ignore]
    fn os_secrets_round_trip() {
        let s = OsSecrets;
        let key = "pixie-test@example.invalid";
        s.set(key, "pw-1").unwrap();
        assert_eq!(s.get(key).unwrap().as_deref(), Some("pw-1"));
        s.set(key, "pw-2").unwrap();
        assert_eq!(s.get(key).unwrap().as_deref(), Some("pw-2"));
        s.delete(key).unwrap();
        assert_eq!(s.get(key).unwrap(), None);
    }
}
