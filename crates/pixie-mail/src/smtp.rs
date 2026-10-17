use crate::error::MailError;
use crate::traits::Mailer;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use pixie_core::{MailAccount, MailSecurity};
use std::time::Duration;

/// What was learned from a failed SMTP attempt, reduced to what decides the message shown.
#[derive(Debug, Clone, Copy)]
pub struct SmtpFailure {
    pub code: Option<u16>,
    pub timeout: bool,
    pub tls: bool,
    pub connection: bool,
}

pub fn classify(f: &SmtpFailure) -> MailError {
    if f.timeout {
        return MailError::Timeout;
    }
    if f.tls {
        return MailError::Tls;
    }
    match f.code {
        Some(530 | 534 | 535 | 538) => MailError::Auth,
        Some(552 | 554) => MailError::TooLarge,
        Some(550 | 551 | 553 | 501) => MailError::RecipientRejected,
        Some(_) => MailError::Other,
        None if f.connection => MailError::Connect,
        None => MailError::Other,
    }
}

fn failure_of(e: &lettre::transport::smtp::Error) -> SmtpFailure {
    SmtpFailure {
        code: e.status().and_then(|c| c.to_string().parse().ok()),
        timeout: e.is_timeout(),
        tls: e.is_tls(),
        connection: e.is_client() || (e.is_transient() && e.status().is_none()),
    }
}

pub struct SmtpMailer;

impl Mailer for SmtpMailer {
    fn send(&self, a: &MailAccount, password: &str, m: &Message) -> Result<(), MailError> {
        let builder = match a.security {
            MailSecurity::Starttls => SmtpTransport::starttls_relay(&a.host),
            MailSecurity::Ssl => SmtpTransport::relay(&a.host),
        }
        .map_err(|_| MailError::Connect)?;
        let transport = builder
            .port(a.port)
            .credentials(Credentials::new(a.email.clone(), password.to_string()))
            .timeout(Some(Duration::from_secs(60))) // per network operation
            .build();
        transport.send(m).map(|_| ()).map_err(|e| {
            let failure = failure_of(&e);
            // Server text can hold the user name and more, so only the category is logged. Never the password.
            eprintln!("[mail] send failed: {failure:?}");
            classify(&failure)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(code: Option<u16>) -> SmtpFailure {
        SmtpFailure { code, timeout: false, tls: false, connection: false }
    }

    #[test]
    fn smtp_failures_map_to_plain_categories() {
        for c in [530, 534, 535, 538] {
            assert_eq!(classify(&f(Some(c))), MailError::Auth, "{c}");
        }
        for c in [552, 554] {
            assert_eq!(classify(&f(Some(c))), MailError::TooLarge, "{c}");
        }
        for c in [550, 551, 553, 501] {
            assert_eq!(classify(&f(Some(c))), MailError::RecipientRejected, "{c}");
        }
        assert_eq!(classify(&SmtpFailure { timeout: true, ..f(None) }), MailError::Timeout);
        assert_eq!(classify(&SmtpFailure { tls: true, ..f(None) }), MailError::Tls);
        assert_eq!(classify(&SmtpFailure { connection: true, ..f(None) }), MailError::Connect);
        assert_eq!(classify(&f(Some(421))), MailError::Other);
        assert_eq!(classify(&f(None)), MailError::Other);
    }
}
