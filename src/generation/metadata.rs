/// The text after the file's front matter, which is where the formatter looks
/// for an ignore file directive.
pub fn strip_metadata_header(text: &str) -> &str {
  let delimiter = if text.starts_with("---") {
    "---"
  } else if text.starts_with("+++") {
    "+++"
  } else {
    return text;
  };

  let Some(first_line_end) = text.find('\n') else {
    return text;
  };
  if text[..first_line_end].trim_end() != delimiter {
    return text;
  }

  let mut pos = first_line_end + 1;
  while pos < text.len() {
    let line_end = text[pos..].find('\n').map(|index| pos + index).unwrap_or(text.len());
    if text[pos..line_end].trim_end() == delimiter {
      return &text[line_end..];
    }
    pos = line_end + 1;
  }

  text
}

#[cfg(test)]
mod test {
  use super::*;
  #[test]
  fn it_should_strip_yaml_header() {
    let result = strip_metadata_header(
      r#"---
a: b
---

Test"#,
    );

    assert_eq!(result, "\n\nTest");
  }

  #[test]
  fn it_should_strip_plus_plus_plus_header() {
    let result = strip_metadata_header(
      r#"+++
a: b
+++

Test"#,
    );

    assert_eq!(result, "\n\nTest");
  }
}
