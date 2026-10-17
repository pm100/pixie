use crate::attach::Attachment;
use crate::recipients::Resolved;
use lettre::message::{header::ContentType, Attachment as MimeAttachment, Mailbox, MultiPart, SinglePart};
use lettre::{Address, Message};
use pixie_core::{MailAccount, PixieError, Result};

/// lettre panics when it later formats a header that holds a line break, so nothing with a control
/// character gets this far (callers validate too; this is the last line of defence).
fn single_line(s: &str) -> Result<()> {
    if s.chars().any(|c| c.is_control()) {
        return Err(PixieError::Invalid("The email contains characters that cannot be sent.".into()));
    }
    Ok(())
}

fn mailbox(name: Option<&str>, email: &str) -> Result<Mailbox> {
    single_line(email)?;
    if let Some(n) = name {
        single_line(n)?;
    }
    let addr: Address = email.parse().map_err(|_| PixieError::Invalid("That email address is not valid.".into()))?;
    Ok(Mailbox::new(name.map(str::to_string), addr))
}

pub fn build_message(
    account: &MailAccount,
    to: &[Resolved],
    subject: &str,
    body: &str,
    attachments: Vec<Attachment>,
) -> Result<Message> {
    single_line(subject)?;
    let mut b = Message::builder().from(mailbox(account.display_name.as_deref(), &account.email)?).subject(subject);
    for r in to {
        b = b.to(mailbox(r.name.as_deref(), &r.email)?);
    }
    let mut multi = MultiPart::mixed().singlepart(SinglePart::plain(body.to_string()));
    for a in attachments {
        single_line(&a.filename)?;
        let ct = ContentType::parse(&a.content_type)
            .map_err(|_| PixieError::Invalid("An attachment has an unsupported type.".into()))?;
        multi = multi.singlepart(MimeAttachment::new(a.filename).body(a.bytes, ct));
    }
    b.multipart(multi).map_err(|_| PixieError::Invalid("The email could not be put together.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_core::MailSecurity;

    fn account() -> MailAccount {
        MailAccount {
            display_name: Some("Paul".into()), email: "me@example.com".into(), provider: "gmail".into(),
            host: "smtp.gmail.com".into(), port: 587, security: MailSecurity::Starttls, limit_mb: 25,
        }
    }

    #[test]
    fn message_has_headers_text_and_every_attachment() {
        let to = vec![
            Resolved { name: Some("Frank".into()), email: "dad@example.com".into() },
            Resolved { name: None, email: "si@example.com".into() },
        ];
        let atts = vec![
            Attachment { filename: "a.jpg".into(), content_type: "image/jpeg".into(), bytes: vec![1; 3000] },
            Attachment { filename: "b (2).jpg".into(), content_type: "image/jpeg".into(), bytes: vec![2; 10] },
        ];
        let m = build_message(&account(), &to, "Holiday", "Hello\nthere", atts).unwrap();
        let text = String::from_utf8_lossy(&m.formatted()).to_string();
        assert!(text.contains("Subject: Holiday"));
        assert!(text.contains("dad@example.com") && text.contains("si@example.com"));
        assert!(text.contains("From: ") && text.contains("me@example.com"));
        assert!(text.contains("filename=\"a.jpg\"") && text.contains("b (2).jpg"));
        assert!(text.contains("Hello"));
        assert!(m.formatted().len() > 4000); // a 3 KB attachment as base64 is about 4 KB
    }

    #[test]
    fn hostile_text_cannot_add_headers_or_recipients() {
        let to = vec![Resolved { name: Some("Evil\r\nBcc: x@y.com".into()), email: "ok@example.com".into() }];
        let r = build_message(&account(), &to, "Subj\r\nBcc: x@y.com", "body", vec![]);
        assert!(r.is_err(), "line breaks in names or subjects must be rejected, not encoded");
        let to = vec![Resolved { name: None, email: "ok@example.com".into() }];
        let bad_file = Attachment { filename: "a\r\nBcc: x@y.com.jpg".into(), content_type: "image/jpeg".into(), bytes: vec![1] };
        assert!(build_message(&account(), &to, "S", "b", vec![bad_file]).is_err());
    }

    #[test]
    fn the_real_builder_is_exercised_not_the_stub() {
        // guards the two tests above against passing vacuously on an always-failing builder
        let to = vec![Resolved { name: None, email: "ok@example.com".into() }];
        assert!(build_message(&account(), &to, "S", "b", vec![]).is_ok());
    }
}
