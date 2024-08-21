use crate::generation::common::CharScanner;

pub fn strip_yaml_header(text: &str) -> &str {
  // todo(dsherret): use pulldown_cmark to parse this and support "+++" as well
  let mut scanner = CharScanner::new(0, text);

  if !scanner.move_text("---") {
    return text;
  }

  if !scanner.move_new_line() {
    return text;
  }

  while scanner.has_next() {
    if scanner.is_next_text("---") {
      scanner.move_text("---");
      if scanner.move_new_line() || scanner.pos() == scanner.end() {
        let range = std::ops::Range {
          start: 0,
          end: scanner.pos(),
        };
        return &text[range.end..];
      }
    }

    scanner.move_next_line();
  }

  text
}

#[cfg(test)]
mod test {
  use super::*;
  #[test]
  fn it_should_strip_yaml_header() {
    let result = strip_yaml_header(
      r#"---
a: b
---

Test"#,
    );

    assert_eq!(result, "\n\nTest");
  }
}
