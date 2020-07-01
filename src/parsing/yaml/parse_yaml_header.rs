use crate::parsing::common::{CharScanner, YamlHeader};

pub fn parse_yaml_header(text: &str) -> Option<YamlHeader> {
    let mut scanner = CharScanner::new(0, text);

    if !scanner.move_text("---") {
        return None;
    }

    if !scanner.move_new_line() {
        return None;
    }

    while scanner.has_next() {
        if scanner.is_next_text("---") {
            scanner.move_text("---");
            if scanner.move_new_line() {
                let range = std::ops::Range {
                    start: 0,
                    end: scanner.pos(),
                };
                let header_text = String::from(&text[range.start..range.end]);
                return Some(YamlHeader {
                    range,
                    text: header_text,
                });
            }
        }

        scanner.move_next_line();
    }

    return None;
}