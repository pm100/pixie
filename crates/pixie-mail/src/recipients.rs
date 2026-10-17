use pixie_core::{validate_email, validate_header_text, Contact, PixieError, Result};

pub const MAX_RECIPIENTS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct RecipientRef {
    pub contact_id: Option<i64>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub name: Option<String>,
    pub email: String,
}

/// Turn what the dialog sent into addresses. Contacts are looked up again here (one may have been
/// edited or deleted while the dialog was open); plain addresses are validated; duplicates collapse.
pub fn resolve_recipients(refs: &[RecipientRef], contacts: &[Contact]) -> Result<Vec<Resolved>> {
    if refs.is_empty() {
        return Err(PixieError::Invalid("Choose at least one person to send to.".into()));
    }
    if refs.len() > MAX_RECIPIENTS {
        return Err(PixieError::Invalid(format!("You can send to at most {MAX_RECIPIENTS} people at once.")));
    }
    let mut out: Vec<Resolved> = Vec::new();
    for r in refs {
        let item = match (r.contact_id, r.email.as_deref()) {
            (Some(id), _) => {
                let c = contacts.iter().find(|c| c.id == id).ok_or_else(|| {
                    PixieError::Invalid("One of the contacts no longer exists. Pick the recipients again.".into())
                })?;
                let name = c
                    .name
                    .as_deref()
                    .and_then(|n| validate_header_text(n, 100, "name").ok())
                    .filter(|n| !n.is_empty());
                Resolved { name: name.or_else(|| Some(c.nickname.clone())), email: c.email.clone() }
            }
            (None, Some(e)) => Resolved { name: None, email: validate_email(e)? },
            (None, None) => return Err(PixieError::Invalid("A recipient is empty.".into())),
        };
        if !out.iter().any(|o| o.email.eq_ignore_ascii_case(&item.email)) {
            out.push(item);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contacts() -> Vec<Contact> {
        vec![
            Contact { id: 1, nickname: "dad".into(), name: Some("Frank Moore".into()), email: "dad@example.com".into() },
            Contact { id: 2, nickname: "Simon".into(), name: None, email: "si@example.com".into() },
        ]
    }
    fn by_id(id: i64) -> RecipientRef {
        RecipientRef { contact_id: Some(id), email: None }
    }
    fn by_email(e: &str) -> RecipientRef {
        RecipientRef { contact_id: None, email: Some(e.into()) }
    }

    #[test]
    fn contacts_and_plain_addresses_resolve() {
        let r = resolve_recipients(&[by_id(1), by_email(" a@b.com "), by_id(2)], &contacts()).unwrap();
        assert_eq!(
            r,
            vec![
                Resolved { name: Some("Frank Moore".into()), email: "dad@example.com".into() },
                Resolved { name: None, email: "a@b.com".into() },
                Resolved { name: Some("Simon".into()), email: "si@example.com".into() }, // no full name: the nickname
            ]
        );
    }

    #[test]
    fn duplicates_unknown_invalid_and_empty_are_handled() {
        let r = resolve_recipients(&[by_id(1), by_email("DAD@example.com"), by_id(1)], &contacts()).unwrap();
        assert_eq!(r.len(), 1); // the same address once, case-insensitive
        assert!(resolve_recipients(&[by_id(99)], &contacts()).is_err()); // contact deleted meanwhile
        assert!(resolve_recipients(&[by_email("nope")], &contacts()).is_err());
        assert!(resolve_recipients(&[by_email("a@b.com\nBcc: x@y.com")], &contacts()).is_err());
        assert!(resolve_recipients(&[], &contacts()).is_err());
        assert!(resolve_recipients(&[RecipientRef { contact_id: None, email: None }], &contacts()).is_err());
        let many: Vec<_> = (0..51).map(|i| by_email(&format!("u{i}@x.com"))).collect();
        assert!(resolve_recipients(&many, &contacts()).is_err());
    }

    #[test]
    fn a_contact_name_that_is_unsafe_falls_back_to_the_nickname() {
        let c = vec![Contact { id: 1, nickname: "dad".into(), name: Some("Frank\r\nBcc: x@y.com".into()), email: "d@x.com".into() }];
        let r = resolve_recipients(&[by_id(1)], &c).unwrap();
        assert_eq!(r[0].name.as_deref(), Some("dad"));
    }
}
