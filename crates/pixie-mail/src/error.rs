#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailError {
    Auth,
    Connect,
    Tls,
    Timeout,
    TooLarge,
    RecipientRejected,
    Other,
}

impl MailError {
    /// Plain-language text. `provider_hint` is shown for a refused login. Server error text is never shown.
    pub fn user_message(&self, provider_hint: &str) -> String {
        match self {
            MailError::Auth => format!("Your mail provider did not accept the password. {provider_hint}"),
            MailError::Connect => "Pixie could not reach the mail server. Check your internet connection and the server name and port in Email settings.".into(),
            MailError::Tls => "Pixie could not make a secure connection to the mail server. Check the server name, port and security setting in Email settings.".into(),
            MailError::Timeout => "The mail server took too long to answer. Please try again.".into(),
            MailError::TooLarge => "The mail server says this email is too big. Choose a smaller size or fewer items.".into(),
            MailError::RecipientRejected => "The mail server rejected one of the recipient addresses. Check the address and try again.".into(),
            MailError::Other => "The email could not be sent. Please try again.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_failure_has_a_plain_message_and_the_provider_hint_where_it_helps() {
        let hint = "Use an app password.";
        assert!(MailError::Auth.user_message(hint).contains(hint));
        assert!(MailError::Auth.user_message(hint).to_lowercase().contains("password"));
        for e in [MailError::Connect, MailError::Tls, MailError::Timeout, MailError::TooLarge, MailError::Other] {
            let m = e.user_message(hint);
            assert!(!m.is_empty() && !m.contains("535") && !m.contains("SMTP"), "{e:?}: {m}");
        }
        assert!(MailError::RecipientRejected.user_message(hint).contains("address"));
    }
}
