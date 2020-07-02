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