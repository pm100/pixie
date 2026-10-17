use crate::attach::{prepare_attachment, SizeChoice};
use crate::message::build_message;
use crate::providers::provider;
use crate::recipients::Resolved;
use crate::text::{check_body, check_limit, check_subject, encoded_size};
use crate::traits::{Mailer, SecretStore};
use pixie_core::{MailAccount, MediaKind, Photo, MAX_ATTACHMENTS};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFailure(pub String);

impl From<pixie_core::PixieError> for SendFailure {
    fn from(e: pixie_core::PixieError) -> Self {
        SendFailure(e.user_message())
    }
}

pub struct SendRequest {
    pub photos: Vec<Photo>,
    pub to: Vec<Resolved>,
    pub subject: String,
    pub message: String,
    pub size: SizeChoice,
}

pub struct Progress {
    pub done: usize,
    pub total: usize,
}

fn hint(account: &MailAccount) -> &'static str {
    provider(&account.provider).map(|p| p.hint).unwrap_or("Check the password in Email settings.")
}

fn password(secrets: &dyn SecretStore, account: &MailAccount) -> Result<String, SendFailure> {
    match secrets.get(&account.email) {
        Ok(Some(p)) if !p.is_empty() => Ok(p),
        _ => Err(SendFailure("No password is saved for your email account. Open Email settings and enter it.".into())),
    }
}

/// Prepare every attachment (one at a time), check the size against the provider limit, send.
/// Returns the addresses it was sent to. Messages in `SendFailure` are plain language and never
/// contain the password or raw server text.
pub fn send_photos(
    mailer: &dyn Mailer,
    secrets: &dyn SecretStore,
    account: &MailAccount,
    req: SendRequest,
    progress: &dyn Fn(Progress),
) -> Result<Vec<String>, SendFailure> {
    if req.photos.is_empty() {
        return Err(SendFailure("Select at least one photo or video to send.".into()));
    }
    if req.photos.len() > MAX_ATTACHMENTS {
        return Err(SendFailure(format!("You can send at most {MAX_ATTACHMENTS} items in one email.")));
    }
    let subject = check_subject(&req.subject)?;
    let body = check_body(&req.message)?;
    let pw = password(secrets, account)?;

    // Cheap pre-check before any decoding: videos and Original are sent as they are, so their file
    // size is exact. Resized photos can only get smaller than their files.
    let exact: u64 = req
        .photos
        .iter()
        .filter(|p| req.size == SizeChoice::Original || p.meta.kind == MediaKind::Video)
        .map(|p| p.size.max(0) as u64)
        .sum();
    check_limit(encoded_size(&[exact]), account.limit_mb)?;

    let total = req.photos.len();
    progress(Progress { done: 0, total });
    let mut taken = HashSet::new();
    let mut atts = Vec::with_capacity(total);
    for (i, p) in req.photos.iter().enumerate() {
        atts.push(prepare_attachment(p, req.size, &mut taken)?);
        progress(Progress { done: i + 1, total });
    }
    let sizes: Vec<u64> = atts.iter().map(|a| a.bytes.len() as u64).collect();
    check_limit(encoded_size(&sizes), account.limit_mb)?;

    let message = build_message(account, &req.to, &subject, &body, atts)?;
    mailer.send(account, &pw, &message).map_err(|e| SendFailure(e.user_message(hint(account))))?;
    Ok(req.to.iter().map(|r| r.email.clone()).collect())
}

/// A short message to the account's own address, to prove the settings work.
pub fn send_test(mailer: &dyn Mailer, secrets: &dyn SecretStore, account: &MailAccount) -> Result<(), SendFailure> {
    let pw = password(secrets, account)?;
    let to = [Resolved { name: None, email: account.email.clone() }];
    let message = build_message(
        account,
        &to,
        "Pixie test email",
        "This is a test message from Pixie. If you can read it, sending photos by email will work.",
        vec![],
    )?;
    mailer.send(account, &pw, &message).map_err(|e| SendFailure(e.user_message(hint(account))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MailError;
    use crate::traits::testing::{FakeMailer, MemorySecrets};
    use pixie_core::{MailSecurity, MediaKind, PhotoMeta};

    fn account() -> MailAccount {
        MailAccount {
            display_name: None, email: "me@example.com".into(), provider: "gmail".into(),
            host: "smtp.gmail.com".into(), port: 587, security: MailSecurity::Starttls, limit_mb: 25,
        }
    }

    fn jpg_photo(d: &tempfile::TempDir, name: &str, w: u32, h: u32) -> Photo {
        let p = d.path().join(name);
        std::fs::write(&p, pixie_testutil::jpeg(w, h)).unwrap();
        let size = std::fs::metadata(&p).unwrap().len() as i64;
        Photo {
            id: 1, folder_id: 1, path: p.to_string_lossy().into(), size, mtime: 0,
            meta: PhotoMeta::default(), missing: false,
        }
    }

    fn secrets_with(pw: &str) -> MemorySecrets {
        let s = MemorySecrets::default();
        s.set("me@example.com", pw).unwrap();
        s
    }

    fn req(photos: Vec<Photo>) -> SendRequest {
        SendRequest {
            photos,
            to: vec![Resolved { name: None, email: "dad@example.com".into() }],
            subject: "Photos".into(),
            message: "".into(),
            size: SizeChoice::Small,
        }
    }

    #[test]
    fn a_good_send_prepares_each_photo_reports_progress_and_uses_the_stored_password() {
        let d = tempfile::tempdir().unwrap();
        let photos = vec![jpg_photo(&d, "a.jpg", 800, 600), jpg_photo(&d, "b.jpg", 800, 600)];
        let (mailer, secrets) = (FakeMailer::ok(), secrets_with("s3cret"));
        let seen = std::sync::Mutex::new(vec![]);
        let sent = send_photos(&mailer, &secrets, &account(), req(photos), &|p| seen.lock().unwrap().push((p.done, p.total))).unwrap();
        assert_eq!(sent, ["dad@example.com"]);
        let calls = mailer.sent.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "s3cret");
        assert!(String::from_utf8_lossy(&calls[0].2).contains("a.jpg"));
        assert_eq!(*seen.lock().unwrap(), [(0, 2), (1, 2), (2, 2)]);
    }

    #[test]
    fn nothing_is_sent_without_a_password_or_with_too_many_or_zero_items() {
        let d = tempfile::tempdir().unwrap();
        let one = vec![jpg_photo(&d, "a.jpg", 100, 100)];
        let m = FakeMailer::ok();
        let e = send_photos(&m, &MemorySecrets::default(), &account(), req(one), &|_| {}).unwrap_err();
        assert!(e.0.to_lowercase().contains("password"), "{}", e.0);
        let many: Vec<_> = (0..51).map(|i| jpg_photo(&d, &format!("{i}.jpg"), 10, 10)).collect();
        assert!(send_photos(&m, &secrets_with("x"), &account(), req(many), &|_| {}).is_err());
        assert!(send_photos(&m, &secrets_with("x"), &account(), req(vec![]), &|_| {}).is_err());
        assert!(m.sent.lock().unwrap().is_empty());
    }

    #[test]
    fn a_selection_over_the_provider_limit_is_refused_before_anything_is_prepared_or_sent() {
        let d = tempfile::tempdir().unwrap();
        let big = d.path().join("big.mov");
        std::fs::write(&big, vec![0u8; 3 * 1024 * 1024]).unwrap();
        let v = Photo {
            path: big.to_string_lossy().into(), size: 3 * 1024 * 1024,
            meta: PhotoMeta { kind: MediaKind::Video, ..Default::default() },
            ..jpg_photo(&d, "x.jpg", 10, 10)
        };
        let mut acct = account();
        acct.limit_mb = 2;
        let m = FakeMailer::ok();
        let e = send_photos(&m, &secrets_with("x"), &acct, req(vec![v]), &|_| {}).unwrap_err();
        assert!(e.0.contains("2 MB"), "{}", e.0);
        assert!(m.sent.lock().unwrap().is_empty());
    }

    #[test]
    fn failures_become_plain_messages_and_never_contain_the_password() {
        let d = tempfile::tempdir().unwrap();
        let photos = vec![jpg_photo(&d, "a.jpg", 100, 100)];
        for (err, needle) in [(MailError::Auth, "password"), (MailError::Connect, "reach"), (MailError::TooLarge, "too big")] {
            let m = FakeMailer::failing(err);
            let e = send_photos(&m, &secrets_with("hunter2"), &account(), req(photos.clone()), &|_| {}).unwrap_err();
            assert!(e.0.to_lowercase().contains(needle), "{err:?}: {}", e.0);
            assert!(!e.0.contains("hunter2"));
        }
    }

    #[test]
    fn the_test_email_goes_to_the_account_itself() {
        let (m, s) = (FakeMailer::ok(), secrets_with("pw"));
        send_test(&m, &s, &account()).unwrap();
        let text = String::from_utf8_lossy(&m.sent.lock().unwrap()[0].2).to_string();
        assert!(text.contains("To: ") && text.contains("me@example.com"));
        assert!(send_test(&m, &MemorySecrets::default(), &account()).is_err());
    }
}
