use regex::Regex;
use lazy_static;

/// Checks if the provided word is a word that could be a list.
/// Assumes the provided string is one word and doesn't have whitespace.
pub fn is_list_word(word: &str) -> bool {
    debug_assert!(!word.chars().any(|c| c.is_whitespace()));

    if word == "*" || word == "-" || word == "+" {
        true
    } else {
        let mut had_number = false;
        let mut had_end_char = false;
        for c in word.chars() {
            if had_end_char {
                return false;
            }

            if !had_number {
                if c.is_numeric() {
                    had_number = true;
                } else {
                    return false;
                }
            } else {
                if !c.is_numeric() {
                    if c == '.' || c == ')' {
                        had_end_char = true;
                    } else {
                        return false;
                    }
                }
            }
        }

        had_end_char
    }
}

pub fn has_leading_blankline(index: usize, text: &str) -> bool {
    let mut newline_count = 0;
    for c in text[0..index].chars().rev() {
        if c == '\n' {
            newline_count += 1;
            if newline_count >= 2 {
                return true;
            }
        } else if !c.is_whitespace() {
            break;
        }
    }
    false
}

pub fn is_ignore_comment(text: &str) -> bool {
    lazy_static! {
        static ref IS_IGNORE_REGEX: Regex = Regex::new(r"\s*<!\-\-\s*dprint-ignore\s*\-\->\s*").unwrap();
    }
    IS_IGNORE_REGEX.is_match(text)
}

pub fn is_ignore_start_comment(text: &str) -> bool {
    lazy_static! {
        static ref IS_IGNORE_START_REGEX: Regex = Regex::new(r"\s*<!\-\-\s*dprint-ignore-start\s*\-\->\s*").unwrap();
    }
    IS_IGNORE_START_REGEX.is_match(text)
}

pub fn is_ignore_end_comment(text: &str) -> bool {
    lazy_static! {
        static ref IS_IGNORE_END_REGEX: Regex = Regex::new(r"\s*<!\-\-\s*dprint-ignore-end\s*\-\->\s*").unwrap();
    }
    IS_IGNORE_END_REGEX.is_match(text)
}

pub fn safe_subtract_to_zero(a: u32, b: u32) -> u32 {
    if a < b {
        0
    } else {
        a - b
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_should_find_list_words() {
        assert_eq!(is_list_word("test"), false);
        assert_eq!(is_list_word("*"), true);
        assert_eq!(is_list_word("+"), true);
        assert_eq!(is_list_word("-"), true);
        assert_eq!(is_list_word("1."), true);
        assert_eq!(is_list_word("99."), true);
        assert_eq!(is_list_word("10)"), true);
        assert_eq!(is_list_word("9999)"), true);
        assert_eq!(is_list_word("9999)."), false);
    }
}