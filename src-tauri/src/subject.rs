//! The subject line of an email that carries a photo: who is in it, where, and when, as far as Pixie knows.

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// "14 Jul 2019" for a photo's `taken_at` (seconds; the wall-clock time as stored, which the app also shows as it is).
pub fn short_date(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    // civil-from-days (proleptic Gregorian)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{day} {} {year}", MONTHS[(month - 1) as usize])
}

/// "Joan", "Joan and Ollie", and for more than two "Joan, Ollie ..." (the first two, the rest left to the dots).
fn people_text(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        [a, b, ..] => format!("{a}, {b} ..."),
    }
}

/// "Photo: " and then the people (named ones only, each once), the place and the date, whichever of them are known;
/// with nothing known, the file name.
pub fn email_subject(people: &[String], place: Option<&str>, taken_at: Option<i64>, file_name: &str) -> String {
    let mut seen: Vec<String> = Vec::new();
    for p in people.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        if !seen.iter().any(|s| s == p) {
            seen.push(p.to_string());
        }
    }
    let who_where = [people_text(&seen), place.map(str::trim).unwrap_or_default().to_string()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    match (who_where.is_empty(), taken_at.map(short_date)) {
        (false, Some(d)) => format!("Photo: {who_where} \u{2013} {d}"),
        (false, None) => format!("Photo: {who_where}"),
        (true, Some(d)) => format!("Photo: {d}"),
        (true, None) => format!("Photo: {file_name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(n: &[&str]) -> Vec<String> {
        n.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn dates_are_short() {
        assert_eq!(short_date(0), "1 Jan 1970");
        assert_eq!(short_date(1_563_062_400 + 3600), "14 Jul 2019");
        assert_eq!(short_date(951_782_400), "29 Feb 2000");
        assert_eq!(short_date(-86_400), "31 Dec 1969");
    }

    #[test]
    fn people_place_and_date_together() {
        assert_eq!(
            email_subject(&names(&["Joan", "Ollie"]), Some("Lisbon"), Some(1_563_062_400), "x.jpg"),
            "Photo: Joan and Ollie, Lisbon \u{2013} 14 Jul 2019"
        );
    }

    #[test]
    fn whatever_is_missing_is_left_out() {
        let t = Some(1_563_062_400);
        assert_eq!(email_subject(&[], Some("Lisbon"), t, "x.jpg"), "Photo: Lisbon \u{2013} 14 Jul 2019");
        assert_eq!(email_subject(&names(&["Joan"]), None, t, "x.jpg"), "Photo: Joan \u{2013} 14 Jul 2019");
        assert_eq!(email_subject(&[], None, t, "x.jpg"), "Photo: 14 Jul 2019");
        assert_eq!(email_subject(&names(&["Joan"]), Some("Lisbon"), None, "x.jpg"), "Photo: Joan, Lisbon");
        assert_eq!(email_subject(&[], None, None, "x.jpg"), "Photo: x.jpg");
    }

    #[test]
    fn names_are_listed_once_and_more_than_two_are_cut_to_two_and_dots() {
        assert_eq!(email_subject(&names(&["Joan", " ", "Joan", "Ollie"]), None, None, "x"), "Photo: Joan and Ollie");
        assert_eq!(email_subject(&names(&["A", "B", "C"]), None, None, "x"), "Photo: A, B ...");
        assert_eq!(email_subject(&names(&["A", "B", "C", "D", "E"]), Some("Rome"), None, "x"), "Photo: A, B ..., Rome");
    }
}
