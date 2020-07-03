use regex::Regex;
use lazy_static;

/// List characters shouldn't have a newline added before them
/// as we don't want to create a list out of a series of text.
pub fn is_list_char(character: char) -> bool {
    character == '*' || character == '-'
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
