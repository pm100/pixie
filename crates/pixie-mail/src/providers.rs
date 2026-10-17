use pixie_core::MailSecurity;

pub struct Provider {
    pub id: &'static str,
    pub label: &'static str,
    pub host: &'static str,
    pub port: u16,
    pub security: MailSecurity,
    pub limit_mb: u32,
    /// One line on how to get the password Pixie needs.
    pub hint: &'static str,
}

const PROVIDERS: &[Provider] = &[
    Provider {
        id: "gmail", label: "Gmail", host: "smtp.gmail.com", port: 587, security: MailSecurity::Starttls, limit_mb: 25,
        hint: "Turn on 2-step verification in your Google account, then create an app password and paste it here.",
    },
    Provider {
        id: "outlook", label: "Outlook.com / Hotmail", host: "smtp-mail.outlook.com", port: 587,
        security: MailSecurity::Starttls, limit_mb: 20,
        hint: "Create an app password in your Microsoft account security settings. Microsoft may refuse app passwords for some accounts.",
    },
    Provider {
        id: "icloud", label: "iCloud", host: "smtp.mail.me.com", port: 587, security: MailSecurity::Starttls, limit_mb: 20,
        hint: "Create an app-specific password at appleid.apple.com and paste it here.",
    },
    Provider {
        id: "yahoo", label: "Yahoo", host: "smtp.mail.yahoo.com", port: 587, security: MailSecurity::Starttls, limit_mb: 25,
        hint: "Create an app password in your Yahoo account security settings.",
    },
    Provider {
        id: "other", label: "Other", host: "", port: 587, security: MailSecurity::Starttls, limit_mb: 20,
        hint: "Enter your mail provider's outgoing (SMTP) server, port and security.",
    },
];

pub fn providers() -> &'static [Provider] {
    PROVIDERS
}

pub fn provider(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_match_the_spec() {
        let g = provider("gmail").unwrap();
        assert_eq!((g.host, g.port, g.security, g.limit_mb), ("smtp.gmail.com", 587, MailSecurity::Starttls, 25));
        assert_eq!(provider("outlook").unwrap().host, "smtp-mail.outlook.com");
        assert_eq!(provider("icloud").unwrap().limit_mb, 20);
        assert_eq!(provider("yahoo").unwrap().host, "smtp.mail.yahoo.com");
        assert!(provider("nope").is_none());
        assert_eq!(providers().iter().map(|p| p.id).collect::<Vec<_>>(), ["gmail", "outlook", "icloud", "yahoo", "other"]);
        assert!(providers().iter().all(|p| !p.hint.is_empty()));
    }
}
