//! Camera names as people see them: the make and model strings in the files are written differently by every maker
//! ("Apple iPhone 12", "iPhone 12", "SAMSUNG-SM-G930A", "Google Pixel 6"), so the Camera list uses a tidied name.

/// Makes whose name is left off when the model repeats it: "Apple iPhone 12" is "iPhone 12", "Google Pixel 7" is
/// "Pixel 7". (The maker is part of what the phone is called.)
const REDUNDANT_MAKERS: &[&str] = &["apple", "google"];

/// Words the makers add after their name ("NIKON CORPORATION", "OLYMPUS IMAGING CORP.").
const COMPANY_WORDS: &[&str] = &["corporation", "corp", "corp.", "imaging", "co.", "co", "ltd", "ltd.", "inc", "inc.", "company"];

fn title_case(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// The tidied name of a camera: spaces collapsed, "SAMSUNG-SM-G930A" as "Samsung SM-G930A", a maker's name dropped when
/// it only repeats what the model already says or is a company suffix, and a maker written twice written once. Two
/// strings that mean the same camera come out the same; cameras that differ stay different.
pub fn normalize_camera(raw: &str) -> String {
    let spaced = raw.replace('_', " ");
    let mut words: Vec<String> = spaced.split_whitespace().map(str::to_string).collect();
    // "SAMSUNG-SM-G930A" (one word with the maker joined by a dash): the maker is the part before the first dash.
    if words.len() == 1 {
        if let Some((maker, rest)) = words[0].split_once('-') {
            if maker.len() > 3 && maker.chars().all(|c| c.is_ascii_alphabetic()) && maker.chars().all(|c| c.is_ascii_uppercase()) {
                words = vec![title_case(maker), rest.to_string()];
            }
        }
    }
    // Company suffixes after the maker's name.
    words.retain(|w| !COMPANY_WORDS.contains(&w.to_lowercase().as_str()));
    // The maker written twice ("NIKON NIKON D90", "Canon Canon EOS 5D") once.
    if words.len() > 1 && words[0].eq_ignore_ascii_case(&words[1]) {
        words.remove(1);
    }
    // "Apple iPhone 12" / "Google Pixel 7": the maker is part of the phone's own name.
    if words.len() > 1 && REDUNDANT_MAKERS.contains(&words[0].to_lowercase().as_str()) {
        words.remove(0);
    }
    // An all-capitals maker ("NIKON", "SONY", "CANON") is written as a name.
    if let Some(first) = words.first_mut() {
        if first.len() > 3 && first.chars().all(|c| c.is_ascii_uppercase()) && words_after_are_model(&first.clone()) {
            *first = title_case(first);
        }
    }
    words.join(" ")
}

/// A leading all-capitals word is a maker's name (and is written as a name) except for product lines that are written in
/// capitals on purpose (such as "COOLPIX").
fn words_after_are_model(first: &str) -> bool {
    !["COOLPIX", "FINEPIX", "LUMIX", "EXILIM", "DSC", "DMC", "IXUS"].contains(&first)
}

#[cfg(test)]
mod tests {
    use super::normalize_camera as n;

    #[test]
    fn phones_are_written_without_the_makers_name_when_it_is_part_of_the_phone() {
        assert_eq!(n("Apple iPhone 12"), "iPhone 12");
        assert_eq!(n("iPhone 12"), "iPhone 12");
        assert_eq!(n("Google Pixel 6"), "Pixel 6");
        assert_eq!(n("Pixel 6"), "Pixel 6");
    }

    #[test]
    fn a_maker_joined_with_a_dash_is_split_and_written_as_a_name() {
        assert_eq!(n("SAMSUNG-SM-G930A"), "Samsung SM-G930A");
        assert_eq!(n("Samsung SM-G930A"), "Samsung SM-G930A");
    }

    #[test]
    fn cameras_keep_their_models_and_lose_company_words_and_repeats() {
        assert_eq!(n("Canon PowerShot SX70 HS"), "Canon PowerShot SX70 HS");
        assert_eq!(n("NIKON CORPORATION NIKON D90"), "Nikon D90");
        assert_eq!(n("Canon Canon EOS 5D"), "Canon EOS 5D");
        assert_eq!(n("COOLPIX B700"), "COOLPIX B700");
        assert_eq!(n("OLYMPUS IMAGING CORP. E-M10"), "Olympus E-M10");
    }

    #[test]
    fn spaces_and_underscores_are_tidied_and_different_cameras_stay_different() {
        assert_eq!(n("  Pixel   7 "), "Pixel 7");
        assert_eq!(n("Pixel_7"), "Pixel 7");
        assert_ne!(n("Pixel 7"), n("Pixel 7 Pro"));
        assert_eq!(n(""), "");
    }
}
