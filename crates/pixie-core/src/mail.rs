use crate::{PixieError, Result};
use serde::{Deserialize, Serialize};

pub const MAX_ATTACHMENTS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    pub id: i64,
    pub nickname: String,
    pub name: Option<String>,
    pub email: String,
}

/// A contact being added (`id: None`) or edited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewContact {
    pub id: Option<i64>,
    pub nickname: String,
    pub name: Option<String>,
    pub email: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MailSecurity {
    Starttls,
    Ssl,
}

/// Non-secret settings of the one mail account. The password lives in the credential store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailAccount {
    pub display_name: Option<String>,
    pub email: String,
    pub provider: String,
    pub host: String,
    pub port: u16,
    pub security: MailSecurity,
    pub limit_mb: u32,
}

fn bad(msg: &str) -> PixieError {
    PixieError::Invalid(msg.into())
}

/// Plain ASCII addresses only: one `@`, a dotted domain, no spaces, quotes, brackets, separators
/// or control characters. Anything that could add a header or a second recipient is rejected.
pub fn validate_email(s: &str) -> Result<String> {
    let s = s.trim();
    let invalid = || bad("That email address is not valid.");
    if s.is_empty() || s.len() > 254 {
        return Err(invalid());
    }
    let ok_chars = s.chars().all(|c| c.is_ascii_alphanumeric() || "._%+-@".contains(c));
    let (local, domain) = s.split_once('@').ok_or_else(invalid)?;
    let domain_ok = domain.contains('.')
        && !domain.starts_with(['.', '-'])
        && !domain.ends_with(['.', '-'])
        && !domain.contains("..")
        && !domain.contains(['@', '%', '+', '_'])
        && domain.len() <= 253;
    if !ok_chars || local.is_empty() || local.len() > 64 || !domain_ok {
        return Err(invalid());
    }
    Ok(s.to_string())
}

pub fn validate_nickname(s: &str) -> Result<String> {
    let s = s.trim();
    if s.is_empty() || s.chars().count() > 40 {
        return Err(bad("A nickname must be 1 to 40 characters."));
    }
    if s.chars().any(|c| c.is_control() || ",;<>\"".contains(c)) {
        return Err(bad("A nickname cannot contain commas, semicolons, quotes, angle brackets or line breaks."));
    }
    Ok(s.to_string())
}

/// A DNS host name: letters, digits, dots and hyphens.
pub fn validate_host(s: &str) -> Result<String> {
    let s = s.trim().to_ascii_lowercase();
    let ok = !s.is_empty()
        && s.len() <= 253
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && !s.starts_with(['.', '-'])
        && !s.contains("..");
    if ok { Ok(s) } else { Err(bad("The mail server name is not valid.")) }
}

/// Free text that ends up in a header (display name, subject): no control characters, bounded.
pub fn validate_header_text(s: &str, max: usize, what: &str) -> Result<String> {
    let s = s.trim();
    if s.chars().count() > max || s.chars().any(|c| c.is_control()) {
        return Err(PixieError::Invalid(format!("The {what} is too long or contains line breaks.")));
    }
    Ok(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_addresses_are_accepted_and_trimmed() {
        assert_eq!(validate_email("  Dad@Example.com ").unwrap(), "Dad@Example.com");
        // surrounding whitespace, including a trailing line break, is trimmed away; the result is clean
        assert_eq!(validate_email("a@b.com\r\n").unwrap(), "a@b.com");
        for a in ["a@b.co", "first.last+tag@sub.example.org", "x_y-z@ex-ample.com"] {
            assert!(validate_email(a).is_ok(), "{a}");
        }
    }

    #[test]
    fn bad_addresses_and_header_injection_are_rejected() {
        for a in [
            "", "plain", "a@", "@b.com", "a@b", "a@@b.com", "a b@c.com", "a@b..com", "a@.b.com", "a@b.com.",
            "a@-b.com", "a@b.com\nBcc: evil@x.com", "a@b.com\r\nBcc: x@y.com", "a@b.com,c@d.com", "a@b.com;c@d.com",
            "<a@b.com>", "\"a\"@b.com", "a(b)@c.com", "é@b.com", "a@bé.com", "a\0@b.com",
        ] {
            assert!(validate_email(a).is_err(), "{a:?} should be rejected");
        }
        assert!(validate_email(&format!("{}@b.com", "a".repeat(65))).is_err());
        assert!(validate_email(&format!("a@{}.com", "b".repeat(250))).is_err());
    }

    #[test]
    fn nicknames_are_short_plain_text() {
        assert_eq!(validate_nickname(" dad ").unwrap(), "dad");
        assert_eq!(validate_nickname("Simon B").unwrap(), "Simon B");
        for n in ["", "   ", "a,b", "a;b", "a<b", "a>b", "a\"b", "a\nb", &"x".repeat(41)] {
            assert!(validate_nickname(n).is_err(), "{n:?}");
        }
    }

    #[test]
    fn hosts_are_plain_dns_names() {
        assert_eq!(validate_host(" Smtp.Gmail.com ").unwrap(), "smtp.gmail.com");
        for h in ["", "a b", "x/y", "a@b", "http://x.com", "-x.com", "x..com", "a\nb", &"h".repeat(256)] {
            assert!(validate_host(h).is_err(), "{h:?}");
        }
    }

    #[test]
    fn header_text_is_bounded_and_single_line() {
        assert_eq!(validate_header_text("  Paul ", 10, "name").unwrap(), "Paul");
        assert!(validate_header_text("a\nb", 10, "name").is_err());
        assert!(validate_header_text(&"x".repeat(11), 10, "name").is_err());
    }

    #[test]
    fn invalid_error_shows_its_own_text() {
        assert_eq!(PixieError::Invalid("Nickname is too long.".into()).user_message(), "Nickname is too long.");
    }
}
