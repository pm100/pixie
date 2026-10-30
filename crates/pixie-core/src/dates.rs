use serde::{Deserialize, Serialize};

/// The days since 1970-01-01 of a calendar date (proleptic Gregorian), so 1904 and 2000 can be turned into times without a
/// date library.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The time (unix seconds) at which 1 January of this year begins.
fn jan_1(year: i32) -> i64 {
    days_from_civil(year as i64, 1, 1) * 86_400
}

/// Which dates in a photo say nothing about when it was taken, and when to use the file's own date in their place. The
/// person chooses (Settings, Dates); the defaults suit most cameras.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DateRules {
    /// Years whose 1 January is a date a camera shows when its clock was never set (a clock that has just started counts up
    /// from midnight, so any time of that day counts). 1904 is where QuickTime time begins, 1970 Unix time, 1980 DOS and 2000
    /// the default of many older cameras.
    pub odd_years: Vec<i32>,
    /// Dates before 1 January of this year count as missing too (1980 by default, before any camera that stamped a date; clear it
    /// for old scanned prints that are genuinely old).
    pub before_year: Option<i32>,
    /// Use the file's own date when the photo has no date inside it.
    pub file_when_none: bool,
    /// Use the file's own date when the date inside is one that counts as missing.
    pub file_when_odd: bool,
}

impl Default for DateRules {
    fn default() -> Self {
        DateRules { odd_years: vec![1904, 1970, 1980, 2000], before_year: Some(1980), file_when_none: true, file_when_odd: true }
    }
}

impl DateRules {
    /// Whether this time (unix seconds) is a date that counts as missing.
    pub fn is_odd(&self, t: i64) -> bool {
        self.odd_years.iter().any(|y| (jan_1(*y)..jan_1(*y) + 86_400).contains(&t)) || self.before_year.is_some_and(|y| t < jan_1(y))
    }

    /// The date to keep for a photo: the one inside it, or when that is missing or odd (as the rules say) the file's own date.
    /// The file's date is never used if it is odd too: a camera that was never set stamps its files with the same dead clock.
    /// `None` means the photo has no date.
    pub fn effective(&self, inside: Option<i64>, file: i64) -> Option<i64> {
        let from_file = |wanted: bool| if wanted { Some(file).filter(|t| !self.is_odd(*t)) } else { None };
        match inside {
            Some(t) if !self.is_odd(t) => Some(t),
            Some(_) => from_file(self.file_when_odd),
            None => from_file(self.file_when_none),
        }
    }
}

/// What the date rules do to the library, for the Missing dates tool.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DateSummary {
    /// Items Browse can list.
    pub total: i64,
    /// Items whose date inside the file counts as missing (a clock that was never set).
    pub odd: i64,
    /// Items with no date inside the file.
    pub none_inside: i64,
    /// Of those two groups, the items that show the file's own date instead.
    pub from_file: i64,
    /// Items left with no date.
    pub undated: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_default_dates_before_1980_are_missing_too() {
        let r = DateRules::default();
        assert_eq!(r.before_year, Some(1980));
        assert!(r.is_odd(315_532_799 - 86_400 * 400) && !r.is_odd(631_152_000));
    }

    #[test]
    fn the_first_of_january_in_unix_seconds() {
        assert_eq!(jan_1(1970), 0);
        assert_eq!(jan_1(1980), 315_532_800);
        assert_eq!(jan_1(2000), 946_684_800);
        assert_eq!(jan_1(1904), -2_082_844_800);
        assert_eq!(jan_1(2020), 1_577_836_800);
    }

    #[test]
    fn a_clock_that_was_never_set_is_odd_at_any_time_of_that_day_and_not_the_day_after() {
        let r = DateRules { before_year: None, ..Default::default() }; // (by default everything before 1980 is odd too)
        for t in [-2_082_844_800, 0, 105, 86_399, 315_532_800 + 105, 946_684_800, 946_684_800 + 86_399] {
            assert!(r.is_odd(t), "{t}");
        }
        for t in [-2_082_844_801, -1, 86_400, 315_532_799, 315_532_800 + 86_400, 946_684_799, 946_684_800 + 86_400, 1_577_934_245] {
            assert!(!r.is_odd(t), "{t}");
        }
    }

    #[test]
    fn an_odd_year_the_person_unticked_is_a_real_date() {
        let r = DateRules { odd_years: vec![1980], ..Default::default() };
        assert!(r.is_odd(315_532_800 + 105) && !r.is_odd(946_684_800));
    }

    #[test]
    fn dates_before_a_year_are_odd_only_when_asked() {
        let r = DateRules { odd_years: vec![], before_year: Some(1990), ..Default::default() };
        assert!(r.is_odd(631_152_000 - 1) && !r.is_odd(631_152_000));
        assert!(!DateRules { odd_years: vec![], before_year: None, ..Default::default() }.is_odd(631_152_000 - 1));
    }

    #[test]
    fn a_real_date_inside_the_file_is_always_kept() {
        let r = DateRules::default();
        assert_eq!(r.effective(Some(1_577_934_245), 5), Some(1_577_934_245));
    }

    #[test]
    fn a_missing_date_takes_the_file_date_when_asked_and_when_it_is_real() {
        let file = 1_313_805_178; // 2011
        assert_eq!(DateRules::default().effective(None, file), Some(file));
        assert_eq!(DateRules { file_when_none: false, ..Default::default() }.effective(None, file), None);
        assert_eq!(DateRules::default().effective(None, 946_684_800 + 7 * 3600), None, "the file carries the same dead clock");
    }

    #[test]
    fn an_odd_date_takes_the_file_date_only_when_asked_and_when_it_is_real() {
        let (odd, file) = (315_532_800 + 105, 1_313_805_178);
        assert_eq!(DateRules::default().effective(Some(odd), file), Some(file));
        assert_eq!(DateRules { file_when_odd: false, ..Default::default() }.effective(Some(odd), file), None);
        assert_eq!(DateRules::default().effective(Some(odd), 946_684_800 + 7 * 3600), None);
    }

    #[test]
    fn saved_rules_with_fields_missing_read_with_the_defaults() {
        let r: DateRules = serde_json::from_str(r#"{"file_when_odd": false}"#).unwrap();
        assert_eq!(r, DateRules { file_when_odd: false, ..Default::default() });
        let s = serde_json::to_string(&DateRules::default()).unwrap();
        assert_eq!(serde_json::from_str::<DateRules>(&s).unwrap(), DateRules::default());
    }
}
