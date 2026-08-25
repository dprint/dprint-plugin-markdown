//! The tag name tables the parser and printer look elements up in.
//!
//! Each table is written in the order `binary_search` needs and compared
//! without case, because a tag name may be written in any case at all.

/// Whether the element is written with no closing tag and can hold no content.
pub fn is_void(name: &str) -> bool {
  contains(&VOID_TAGS, name)
}

/// Whether the element's content is character data rather than markup, which
/// means it runs to the element's closing tag no matter what is written in it.
pub fn is_raw_text(name: &str) -> bool {
  contains(&RAW_TEXT_TAGS, name)
}

/// Whether the whitespace within the element is rendered as it was written,
/// which means the printer has to leave the content exactly as it is.
pub fn is_preformatted(name: &str) -> bool {
  contains(&PREFORMATTED_TAGS, name)
}

/// Whether the element is laid out as a block, where the whitespace at the
/// start and end of its content isn't rendered.
///
/// Everything else is treated as being laid out inline, which is both what the
/// spec says an unknown element is and the careful assumption to make: the
/// whitespace around an inline element is rendered, so the printer has to keep
/// it as it was written.
pub fn is_block(name: &str) -> bool {
  contains(&BLOCK_TAGS, name)
}

/// Whether the element's content is svg or mathml rather than html, where a
/// self-closing tag closes the element it's written on.
pub fn is_foreign_root(name: &str) -> bool {
  name.eq_ignore_ascii_case("svg") || name.eq_ignore_ascii_case("math")
}

fn contains(tags: &[&str], name: &str) -> bool {
  tags.binary_search_by(|tag| compare_tag_name(tag, name)).is_ok()
}

fn compare_tag_name(tag: &str, name: &str) -> std::cmp::Ordering {
  tag
    .bytes()
    .map(|b| b.to_ascii_lowercase())
    .cmp(name.bytes().map(|b| b.to_ascii_lowercase()))
}

const VOID_TAGS: [&str; 18] = [
  "area", "base", "basefont", "br", "col", "embed", "frame", "hr", "img", "input", "isindex", "keygen", "link", "meta",
  "param", "source", "track", "wbr",
];

const RAW_TEXT_TAGS: [&str; 4] = ["script", "style", "textarea", "title"];

const PREFORMATTED_TAGS: [&str; 4] = ["listing", "plaintext", "pre", "textarea"];

const BLOCK_TAGS: [&str; 58] = [
  "address",
  "article",
  "aside",
  "blockquote",
  "body",
  "caption",
  "center",
  "col",
  "colgroup",
  "dd",
  "details",
  "dialog",
  "dir",
  "div",
  "dl",
  "dt",
  "fieldset",
  "figcaption",
  "figure",
  "footer",
  "form",
  "frame",
  "frameset",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "head",
  "header",
  "hgroup",
  "hr",
  "html",
  "legend",
  "li",
  "main",
  "menu",
  "nav",
  "ol",
  "optgroup",
  "option",
  "p",
  "pre",
  "script",
  "search",
  "section",
  "style",
  "summary",
  "table",
  "tbody",
  "td",
  "tfoot",
  "th",
  "thead",
  "title",
  "tr",
  "ul",
];

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn tables_are_sorted_for_binary_search() {
    for table in [
      &VOID_TAGS[..],
      &RAW_TEXT_TAGS[..],
      &PREFORMATTED_TAGS[..],
      &BLOCK_TAGS[..],
    ] {
      let mut sorted = table.to_vec();
      sorted.sort();
      assert_eq!(table, sorted, "table is not in sorted order");
    }
  }

  #[test]
  fn looks_tags_up_without_case() {
    assert!(is_void("BR"));
    assert!(is_void("Img"));
    assert!(!is_void("div"));
    assert!(is_raw_text("SCRIPT"));
    assert!(is_block("DIV"));
    assert!(!is_block("span"));
    assert!(is_preformatted("Pre"));
  }

  #[test]
  fn treats_an_unknown_element_as_inline() {
    assert!(!is_block("my-widget"));
    assert!(!is_block("Foo"));
  }
}
