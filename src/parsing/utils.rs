/// List characters shouldn't have a newline added before them
/// as we don't want to create a list out of a series of text.
pub fn is_list_char(character: char) -> bool {
    character == '*' || character == '-'
}
