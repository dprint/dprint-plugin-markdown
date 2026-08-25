//! The html parser's spec tests, which pair html text with the json of the ast
//! it should parse into.
//!
//! The files live under `tests/html_parser_specs` and hold any number of
//! cases, written the same way the markdown parser's are:
//!
//! ```
//! [[test]] a name for the case
//! <p>some html</p>
//! [[ast]]
//! { ...the expected ast... }
//! ```
//!
//! A case written as `[[test:crlf]]` has its line endings turned into `\r\n`
//! before it's parsed, since the files themselves are kept with `\n` endings.
//! A case whose html the parser refuses is written with the reason in place of
//! the ast, which is how the html that is left as it was gets its own cases.
//!
//! Every case is also checked against the invariants an ast holds whatever the
//! expectation says: that each node was parsed from within its parent, and
//! that the children of a node account for every byte of it (see
//! [`validate_coverage`]). Nothing may be read and then dropped, because the
//! formatter has to be able to write back out everything that was read.
//!
//! `tests/html_parser_spec_test.rs` is what runs these.
#![allow(dead_code)]

use std::path::Path;

use super::ast::Document;
use super::ast::Node;
use super::debug_json::offset_in;
use super::debug_json::to_json;

/// Reads the case through the parser, giving back the json of the ast it holds
/// along with whatever it fails of the invariants every ast holds.
pub fn run_case(case: &SpecCase) -> CaseOutcome {
  let source = case.source();
  let document = match super::parser::parse(&source) {
    Ok(document) => document,
    // html the parser refuses isn't a failure: an html block in markdown is
    // very often a fragment, and refusing it is what leaves it as it was
    Err(err) => {
      return CaseOutcome {
        actual: format!("[[refused]] {}\n", err),
        failures: Vec::new(),
      }
    }
  };
  let mut failures = Vec::new();
  if let Err(message) = validate_coverage(&document, &source) {
    failures.push(message);
  }
  CaseOutcome {
    actual: to_json(&document, &source),
    failures,
  }
}

pub struct CaseOutcome {
  /// The json of the ast the case parsed into, or why the parser refused it.
  pub actual: String,
  /// What the ast failed of the invariants every one of them holds.
  pub failures: Vec<String>,
}

pub struct SpecCase {
  pub name: String,
  pub input: String,
  pub expected: String,
  /// Whether the input is fed to the parser with `\r\n` line endings.
  pub crlf: bool,
}

impl SpecCase {
  pub fn source(&self) -> String {
    if self.crlf {
      self.input.replace('\n', "\r\n")
    } else {
      self.input.clone()
    }
  }

  pub fn header(&self) -> String {
    let marker = if self.crlf { "[[test:crlf]]" } else { "[[test]]" };
    format!("{} {}", marker, self.name)
  }
}

/// Checks that the ast accounts for every byte of the html it was parsed from.
///
/// The children of a node run in order and butt up against one another, from
/// the first byte of what holds them to the last, so there is nowhere for text
/// to have been read and then dropped. That is what makes the ast something
/// the printer can write back out without losing anything.
pub fn validate_coverage(document: &Document<'_>, source: &str) -> Result<(), String> {
  tile("the document", &document.children, source, 0, source.len(), source)
}

fn tile(
  within: &str,
  children: &[Node<'_>],
  content: &str,
  start: usize,
  end: usize,
  source: &str,
) -> Result<(), String> {
  if children.is_empty() {
    return if start == end {
      Ok(())
    } else {
      Err(format!("{} holds no children but covers {:?}", within, content))
    };
  }

  let mut position = start;
  for child in children {
    let text = child.source();
    let child_start = offset_in(text, source);
    if child_start != position {
      return Err(format!(
        "{}: the {} at {} doesn't start where the one before it ended, at {}",
        within,
        child.kind(),
        child_start,
        position,
      ));
    }
    if !source.is_char_boundary(child_start) || !source.is_char_boundary(child_start + text.len()) {
      return Err(format!(
        "{}: the {} at {} splits a character",
        within,
        child.kind(),
        child_start
      ));
    }
    position = child_start + text.len();

    if let Node::Element(element) = child {
      if element.content.is_empty() && element.children.is_empty() {
        continue;
      }
      let content_start = offset_in(element.content, source);
      let name = format!("`<{}>` at {}", element.name, child_start);
      tile(
        &name,
        &element.children,
        element.content,
        content_start,
        content_start + element.content.len(),
        source,
      )?;
    }
  }

  if position != end {
    return Err(format!(
      "{}: the children stop at {} but what holds them runs to {}",
      within, position, end
    ));
  }
  Ok(())
}

pub fn parse_spec_file(file_text: &str, path: &Path) -> Vec<SpecCase> {
  let mut cases = Vec::new();
  let mut lines = file_text.lines().peekable();

  while let Some(line) = lines.next() {
    let (crlf, name) = match line.strip_prefix("[[test:crlf]]") {
      Some(name) => (true, Some(name)),
      None => (false, line.strip_prefix("[[test]]")),
    };
    let Some(name) = name else {
      assert!(
        line.trim().is_empty(),
        "unexpected text outside of a test case in {}: {}",
        path.display(),
        line
      );
      continue;
    };

    let mut input = String::new();
    let mut found_ast = false;
    for line in lines.by_ref() {
      if line == "[[ast]]" {
        found_ast = true;
        break;
      }
      input.push_str(line);
      input.push('\n');
    }
    assert!(found_ast, "missing [[ast]] in {} for {}", path.display(), name.trim());
    // the newline before the `[[ast]]` marker isn't part of the input
    input.pop();

    let mut expected = String::new();
    while let Some(line) = lines.peek() {
      if line.starts_with("[[test]]") || line.starts_with("[[test:") {
        break;
      }
      expected.push_str(lines.next().unwrap());
      expected.push('\n');
    }

    cases.push(SpecCase {
      name: name.trim().to_string(),
      input,
      expected,
      crlf,
    });
  }

  cases
}
