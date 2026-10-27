//! Email size limits: how big a set of attachments counts as, and how to split it into messages that each fit.

/// The longest side, in pixels, of a picture made smaller for email, and the quality of its JPEG (1 to 100).
pub const SMALLER_EDGE: u32 = 2048;
pub const SMALLER_QUALITY: u8 = 85;

/// What a file counts as against an email service's limit: attachments grow by about a third when they are sent
/// (they are written as text), so a limit of 20 MB holds about 15 MB of files.
pub fn sent_size(bytes: u64) -> u64 {
    bytes.saturating_add(bytes / 3).saturating_add(bytes / 50)
}

/// Pack items, given by their sizes in the order they are to go, into messages that each fit under `limit_bytes` once sent.
/// Each item stays whole; one that is too big even alone gets a message to itself. Returns the item numbers of each
/// message, in order.
pub fn pack(sizes: &[u64], limit_bytes: u64) -> Vec<Vec<usize>> {
    let mut messages: Vec<Vec<usize>> = Vec::new();
    let mut used = 0u64;
    for (i, &size) in sizes.iter().enumerate() {
        let counts = sent_size(size);
        match messages.last_mut() {
            Some(last) if used.saturating_add(counts) <= limit_bytes => {
                last.push(i);
                used += counts;
            }
            _ => {
                messages.push(vec![i]);
                used = counts;
            }
        }
    }
    messages
}

/// The subject of message `part` (from 1) of `of`: "Photos: 5 items (2 of 3)". A single message keeps its subject.
pub fn part_subject(subject: &str, part: usize, of: usize) -> String {
    if of <= 1 {
        subject.to_string()
    } else {
        format!("{subject} ({part} of {of})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    #[test]
    fn sending_makes_a_file_about_a_third_bigger() {
        assert!(sent_size(15 * MB) > 19 * MB && sent_size(15 * MB) < 21 * MB);
        assert_eq!(sent_size(0), 0);
    }

    #[test]
    fn items_are_packed_in_order_into_messages_that_fit() {
        // six photos of 6 MB each (8 MB once sent) with a 20 MB limit: two to a message
        let sizes = vec![6 * MB; 6];
        let m = pack(&sizes, 20 * MB);
        assert_eq!(m, vec![vec![0, 1], vec![2, 3], vec![4, 5]]);
    }

    #[test]
    fn an_item_too_big_alone_gets_a_message_to_itself_and_the_rest_carry_on() {
        let sizes = vec![2 * MB, 40 * MB, 2 * MB, 2 * MB];
        let m = pack(&sizes, 20 * MB);
        assert_eq!(m, vec![vec![0], vec![1], vec![2, 3]]);
    }

    #[test]
    fn everything_that_fits_is_one_message_and_nothing_is_none() {
        assert_eq!(pack(&[MB, MB, MB], 20 * MB), vec![vec![0, 1, 2]]);
        assert!(pack(&[], 20 * MB).is_empty());
    }

    #[test]
    fn only_a_split_message_says_which_part_it_is() {
        assert_eq!(part_subject("Photos: 5 items", 1, 1), "Photos: 5 items");
        assert_eq!(part_subject("Photos: 5 items", 2, 3), "Photos: 5 items (2 of 3)");
    }
}
