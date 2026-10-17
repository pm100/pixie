use crate::error::MailError;
use pixie_core::{MailAccount, Result};

/// Where the account password lives. The real one is the OS credential store.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, secret: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// Sends one finished message. The real one speaks SMTP.
pub trait Mailer: Send + Sync {
    fn send(&self, account: &MailAccount, password: &str, message: &lettre::Message) -> std::result::Result<(), MailError>;
}

#[cfg(any(test, feature = "testing"))]
pub mod testing {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct MemorySecrets(pub Mutex<HashMap<String, String>>);

    impl SecretStore for MemorySecrets {
        fn get(&self, key: &str) -> Result<Option<String>> {
            Ok(self.0.lock().unwrap().get(key).cloned())
        }
        fn set(&self, key: &str, secret: &str) -> Result<()> {
            self.0.lock().unwrap().insert(key.into(), secret.into());
            Ok(())
        }
        fn delete(&self, key: &str) -> Result<()> {
            self.0.lock().unwrap().remove(key);
            Ok(())
        }
    }

    /// Records what it was asked to send; answers with a preset result.
    pub struct FakeMailer {
        /// (account email, password used, formatted message)
        pub sent: Mutex<Vec<(String, String, Vec<u8>)>>,
        pub result: Mutex<std::result::Result<(), MailError>>,
    }

    impl FakeMailer {
        pub fn ok() -> Self {
            FakeMailer { sent: Mutex::new(vec![]), result: Mutex::new(Ok(())) }
        }
        pub fn failing(e: MailError) -> Self {
            FakeMailer { sent: Mutex::new(vec![]), result: Mutex::new(Err(e)) }
        }
    }

    impl Mailer for FakeMailer {
        fn send(&self, a: &MailAccount, password: &str, m: &lettre::Message) -> std::result::Result<(), MailError> {
            self.sent.lock().unwrap().push((a.email.clone(), password.into(), m.formatted()));
            *self.result.lock().unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::MemorySecrets;
    use super::*;

    #[test]
    fn memory_secrets_store_read_and_delete() {
        let s = MemorySecrets::default();
        assert_eq!(s.get("a").unwrap(), None);
        s.set("a", "pw").unwrap();
        assert_eq!(s.get("a").unwrap().as_deref(), Some("pw"));
        s.delete("a").unwrap();
        assert_eq!(s.get("a").unwrap(), None);
        s.delete("a").unwrap(); // deleting nothing is fine
    }
}
