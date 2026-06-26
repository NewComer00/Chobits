/// Parse a 1-based session index from interactive input.
pub fn parse_session_choice(input: &str, count: usize) -> Option<usize> {
    input
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|&n| n >= 1 && n <= count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_one_based_index() {
        assert_eq!(parse_session_choice("1", 3), Some(1));
        assert_eq!(parse_session_choice(" 2 ", 3), Some(2));
        assert_eq!(parse_session_choice("3", 3), Some(3));
    }

    #[test]
    fn rejects_out_of_range_or_invalid() {
        assert_eq!(parse_session_choice("0", 3), None);
        assert_eq!(parse_session_choice("4", 3), None);
        assert_eq!(parse_session_choice("abc", 3), None);
        assert_eq!(parse_session_choice("", 3), None);
    }
}
