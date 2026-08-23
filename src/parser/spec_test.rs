//! The parser's spec tests, which pair markdown text with the json of the ast
//! it should parse into.
//!
//! The files live under `tests/parser_specs` and hold any number of cases:
//!
//! ```
//! [[test]] a name for the case
//! # some markdown
//! [[ast]]
//! { ...the expected ast... }
//! ```
//!
//! A case written as `[[test:crlf]]` has its line endings turned into `\r\n`
//! before it's parsed, since the files themselves are kept with `\n` endings.
//!
//! Every case is also checked against the invariants an ast holds whatever the
//! expectation says: that its nodes sit within one another in order (see
//! [`validate_spans`]) and that it holds every bit of the text they were parsed
//! from (see [`validate_text_coverage`]).
//!
//! `tests/parser_spec_test.rs` is what runs these; this is the reading and
//! checking of a case, which the fuzzer next door shares.
#![allow(dead_code)]

use std::path::Path;

use super::ast::Node;
use super::ast::Ranged;
use super::ast::SourceFile;
use super::ast::Span;
use super::ast::TableCell;
use super::ast::TaskListMarker;
use super::debug_json::to_json;

/// Reads the case through the parser, giving back the json of the ast it holds
/// along with whatever it fails of the invariants every ast holds.
pub fn run_case(case: &SpecCase) -> CaseOutcome {
  let source = case.source();
  let file = match super::parse(&source) {
    Ok(file) => file,
    Err(err) => {
      return CaseOutcome {
        actual: String::new(),
        failures: vec![err.to_string()],
      }
    }
  };
  let children: Vec<&Node<'_>> = file.children.iter().collect();
  let mut failures = Vec::new();
  if let Err(message) = validate_spans(&children, file.span, &source) {
    failures.push(message);
  }
  if let Err(message) = validate_text_coverage(&file, &source) {
    failures.push(message);
  }
  CaseOutcome {
    actual: to_json(&file, &source),
    failures,
  }
}

pub struct CaseOutcome {
  /// The json of the ast the case parsed into.
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

/// Checks what holds of every ast no matter what a case expects of it: a node
/// sits within its parent, siblings run in order without overlapping, and each
/// span falls on a character boundary of the source.
fn validate_spans(nodes: &[&Node<'_>], parent: Span, source: &str) -> Result<(), String> {
  let mut previous_end = parent.start;
  for node in nodes {
    let span = node.span();
    if !source.is_char_boundary(span.start) || !source.is_char_boundary(span.end) {
      return Err(format!(
        "{:?} has a span that splits a character: {:?}",
        node.kind(),
        span
      ));
    }
    if span.start < parent.start || span.end > parent.end {
      return Err(format!(
        "{:?} at {:?} reaches outside its parent at {:?}",
        node.kind(),
        span,
        parent
      ));
    }
    if span.start < previous_end {
      return Err(format!(
        "{:?} at {:?} overlaps the sibling that ended at {}",
        node.kind(),
        span,
        previous_end
      ));
    }
    previous_end = span.end;
    validate_spans(child_nodes(node).as_slice(), span, source)?;
  }
  Ok(())
}

/// Checks that the ast holds every bit of the source that isn't markup, which
/// is what makes formatting from it lossless.
///
/// Markdown is written with punctuation, so the letters and digits of a node's
/// span are its text and nothing else -- apart from a list item's number, a
/// task list marker, a fence's info string and a footnote's name, which the
/// nodes that are written with them report as their own. Reading the text back
/// out of a node has to give exactly what its span holds: anything missing was
/// read and then dropped, and anything extra was read twice.
pub fn validate_text_coverage(file: &SourceFile<'_>, source: &str) -> Result<(), String> {
  // the file's own span may sit within the source, but only whitespace can be
  // outside of it
  let outside = written_letters(&source[..file.span.start]) + &written_letters(&source[file.span.end..]);
  if !outside.is_empty() {
    return Err(format!(
      "SourceFile at {:?} leaves the text {:?} of {:?} outside the ast",
      file.span, outside, source,
    ));
  }
  let held = children_content(&file.children, source);
  compare("SourceFile", file.span, &held, source)?;
  for child in &file.children {
    visit(child, source)?;
  }
  return Ok(());

  fn visit(node: &Node<'_>, source: &str) -> Result<(), String> {
    compare(
      &format!("{:?}", node.kind()),
      node.span(),
      &content(node, source),
      source,
    )?;
    for child in child_nodes(node) {
      visit(child, source)?;
    }
    Ok(())
  }

  fn compare(label: &str, span: Span, held: &str, source: &str) -> Result<(), String> {
    let written = written_letters(&source[span.start..span.end]);
    if written == held {
      return Ok(());
    }
    Err(format!(
      "{} at {:?} holds {:?} of the {:?} written at {:?}",
      label,
      span,
      held,
      written,
      &source[span.start..span.end],
    ))
  }

  /// What a node holds of the source, as the letters and digits of it in the
  /// order they are written.
  fn content(node: &Node<'_>, source: &str) -> String {
    match node {
      Node::Text(text) => written_letters(text.text),
      Node::Code(code) => written_letters(&code.code),
      Node::Html(html) => written_letters(&html.text),
      Node::DisplayMath(math) => written_letters(&math.text),
      Node::InlineMath(math) => written_letters(&math.text),
      Node::MetadataBlock(block) => written_letters(block.text),
      // a fenced code block names the language it is written in
      Node::CodeBlock(block) => {
        let info = block.fence.as_ref().and_then(|fence| fence.info).unwrap_or("");
        written_letters(info) + &written_letters(&block.code)
      }
      // a footnote is written under the name it is referred to by
      Node::FootnoteReference(reference) => written_letters(reference.name),
      Node::FootnoteDefinition(definition) => {
        written_letters(definition.name) + &children_content(&definition.children, source)
      }
      // a link is written as the text it is under, then as where it points
      Node::InlineLink(link) => {
        children_content(&link.children, source) + &written_letters(&link.url) + &title_content(&link.title)
      }
      Node::ReferenceLink(link) => {
        children_content(&link.children, source) + &reference_content(&link.reference, link.span, source)
      }
      Node::LinkReference(reference) => {
        written_letters(&reference.name) + &written_letters(&reference.link) + &title_content(&reference.title)
      }
      Node::InlineImage(image) => {
        written_letters(&image.text) + &written_letters(&image.url) + &title_content(&image.title)
      }
      Node::ReferenceImage(image) => {
        written_letters(&image.text) + &reference_content(&image.reference, image.span, source)
      }
      Node::ShortcutImage(image) => written_letters(&image.text),
      // an item is written after a marker that may hold a number, and may open
      // with the marker of a task list
      Node::Item(item) => {
        written_letters(&source[item.marker_span.start..item.marker_span.end])
          + &marker_content(&item.marker)
          + &children_content(&item.children, source)
          + &children_content(&item.sub_lists, source)
      }
      Node::Paragraph(paragraph) => marker_content(&paragraph.marker) + &children_content(&paragraph.children, source),
      Node::TaskListMarker(marker) => checked_content(marker.is_checked),
      Node::Table(table) => {
        let cells = |cells: &[TableCell<'_>]| {
          cells
            .iter()
            .map(|cell| children_content(&cell.children, source))
            .collect::<String>()
        };
        cells(&table.header.cells) + &table.rows.iter().map(|row| cells(&row.cells)).collect::<String>()
      }
      node => children_content(node.children(), source),
    }
  }

  fn children_content(nodes: &[Node<'_>], source: &str) -> String {
    nodes.iter().map(|node| content(node, source)).collect()
  }

  fn title_content(title: &Option<std::borrow::Cow<'_, str>>) -> String {
    written_letters(title.as_deref().unwrap_or(""))
  }

  /// The label a reference link or image points at, which is only written out
  /// where it isn't the text the link is already under.
  fn reference_content(reference: &str, span: Span, source: &str) -> String {
    match has_empty_label(&source[span.start..span.end]) {
      true => String::new(),
      false => written_letters(reference),
    }
  }

  /// Whether the link or image is written with an empty label, which points it
  /// at the text it is already under.
  fn has_empty_label(text: &str) -> bool {
    let Some(rest) = text.strip_suffix("[]") else {
      return false;
    };
    // a label written as `[a\[]` ends with a bracket of its own rather than an
    // empty one
    let backslashes = rest.len() - rest.trim_end_matches('\\').len();
    backslashes % 2 == 0
  }

  /// A task list marker is written as `[x]` when it is checked off, and the `x`
  /// is the only letter markdown's markup is written with.
  fn marker_content(marker: &Option<TaskListMarker>) -> String {
    checked_content(marker.as_ref().is_some_and(|marker| marker.is_checked))
  }

  fn checked_content(is_checked: bool) -> String {
    match is_checked {
      true => String::from("x"),
      false => String::new(),
    }
  }

  /// The text of the source that is content rather than markup, in the order it
  /// is written.
  ///
  /// Markdown is written entirely in ascii punctuation, so a letter, a digit or
  /// anything outside of ascii is content -- including the spaces that only
  /// look like whitespace, such as a non-breaking one, which markdown reads as
  /// the text it is.
  fn written_letters(text: &str) -> String {
    text
      .chars()
      .filter(|c| c.is_alphanumeric() || !c.is_ascii())
      .flat_map(|c| c.to_lowercase())
      .collect()
  }
}

/// Every node held by the given one, including the ones its own children don't
/// cover (a table's cells, an item's trailing lists).
fn child_nodes<'a, 'b>(node: &'b Node<'a>) -> Vec<&'b Node<'a>> {
  let mut nodes: Vec<&Node<'a>> = node.children().iter().collect();
  match node {
    Node::Item(item) => nodes.extend(item.sub_lists.iter()),
    Node::Table(table) => {
      // the head and the rows aren't nodes of their own to the ast, so their
      // cells stand in for them here
      for cell in &table.header.cells {
        nodes.extend(cell.children.iter());
      }
      for row in &table.rows {
        for cell in &row.cells {
          nodes.extend(cell.children.iter());
        }
      }
    }
    _ => {}
  }
  nodes
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
