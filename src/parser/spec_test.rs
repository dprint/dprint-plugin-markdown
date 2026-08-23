// Runs the parser's spec tests, which pair markdown text with the json of the
// ast it should parse into.
//
// The files live under `tests/parser_specs` and hold any number of cases:
//
// ```
// [[test]] a name for the case
// # some markdown
// [[ast]]
// { ...the expected ast... }
// ```
//
// A case written as `[[test:crlf]]` has its line endings turned into `\r\n`
// before it's parsed, since the files themselves are kept with `\n` endings.
//
// Every case is also checked against the invariants an ast holds whatever the
// expectation says: see [`validate_spans`].
//
// Set `UPDATE=1` to rewrite the files with what the parser currently produces,
// which is how a new case is filled in. Always read the result before
// committing it.

use std::path::Path;
use std::path::PathBuf;

use super::ast::Node;
use super::ast::SourceFile;
use super::ast::Ranged;
use super::ast::Span;
use super::debug_json::to_json;

#[test]
fn spec_tests() {
  let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/parser_specs");
  let update = std::env::var("UPDATE").is_ok();
  let mut failures = Vec::new();
  let mut count = 0;

  let filter = std::env::var("SPEC_FILTER").unwrap_or_default();
  for path in spec_files(&directory) {
    if !filter.is_empty() && !path.to_string_lossy().contains(&filter) {
      continue;
    }
    let file_text = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
    let cases = parse_spec_file(&file_text, &path);
    let mut updated = String::new();

    for case in &cases {
      count += 1;
      if std::env::var("SPEC_TRACE").is_ok() {
        use std::io::Write;
        writeln!(std::io::stderr(), "{}: {}", path.display(), case.name).unwrap();
      }
      let source = case.source();
      let file = super::parse(&source);
      let children: Vec<&Node<'_>> = file.children.iter().collect();
      if let Err(message) = validate_spans(&children, file.span, &source) {
        failures.push(format!("{}\n  case: {}\n  {}", path.display(), case.name, message));
      }
      if let Err(message) = validate_text_coverage(&file, &source) {
        failures.push(format!("{}\n  case: {}\n  {}", path.display(), case.name, message));
      }
      let actual = to_json(&file, &source);
      updated.push_str(&format!("{}\n{}\n[[ast]]\n{}", case.header(), case.input, actual));
      if actual != case.expected {
        failures.push(format!(
          "{}\n  case: {}\n  --- expected ---\n{}\n  --- actual ---\n{}",
          path.display(),
          case.name,
          case.expected,
          actual,
        ));
      }
    }

    if update && !cases.is_empty() {
      std::fs::write(&path, updated).unwrap();
    }
  }

  if let Ok(corpus) = std::env::var("COVERAGE_CORPUS") {
    let mut files = Vec::new();
    collect_markdown(std::path::Path::new(&corpus), &mut files);
    for path in &files {
      let Ok(source) = std::fs::read_to_string(path) else {
        continue;
      };
      if let Err(message) = validate_text_coverage(&super::parse(&source), &source) {
        failures.push(format!("{}
  {}", path.display(), message));
      }
    }
    eprintln!("checked {} corpus files", files.len());
  }

  assert!(count > 0, "no parser spec tests were found");
  if update {
    return;
  }
  if !failures.is_empty() {
    panic!(
      "{} of {} parser spec tests failed:\n\n{}\n\nRe-run with UPDATE=1 to accept the new output.",
      failures.len(),
      count,
      failures.join("\n\n"),
    );
  }
}

struct SpecCase {
  name: String,
  input: String,
  expected: String,
  /// Whether the input is fed to the parser with `\r\n` line endings.
  crlf: bool,
}

impl SpecCase {
  fn source(&self) -> String {
    if self.crlf {
      self.input.replace('\n', "\r\n")
    } else {
      self.input.clone()
    }
  }

  fn header(&self) -> String {
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
/// Every character a node covers that none of its children do has to be part of
/// how the node itself is written, and nothing markdown is written with is
/// alphanumeric -- apart from the few places [`markup_spans`] lists. So an
/// alphanumeric character in the gap between two children is text that was read
/// and then dropped.
pub fn validate_text_coverage(file: &SourceFile<'_>, source: &str) -> Result<(), String> {
  let children: Vec<&Node<'_>> = file.children.iter().collect();
  check_gaps("SourceFile", file.span, &children, &[], source)?;
  for child in children {
    visit(child, source)?;
  }
  return Ok(());

  fn visit(node: &Node<'_>, source: &str) -> Result<(), String> {
    let children = child_nodes(node);
    if !children.is_empty() && !holds_its_own_text(node) {
      let label = format!("{:?}", node.kind());
      check_gaps(&label, node.span(), &children, &markup_spans(node), source)?;
    }
    for child in children {
      visit(child, source)?;
    }
    Ok(())
  }

  fn check_gaps(
    label: &str,
    span: Span,
    children: &[&Node<'_>],
    allowed: &[Span],
    source: &str,
  ) -> Result<(), String> {
    let mut position = span.start;
    for child in children {
      check_gap(label, span, position, child.span().start, allowed, source)?;
      position = child.span().end;
    }
    check_gap(label, span, position, span.end, allowed, source)
  }

  fn check_gap(
    label: &str,
    span: Span,
    from: usize,
    to: usize,
    allowed: &[Span],
    source: &str,
  ) -> Result<(), String> {
    if from >= to {
      return Ok(());
    }
    for (offset, character) in source[from..to].char_indices() {
      let position = from + offset;
      if !character.is_alphanumeric() || allowed.iter().any(|span| span.start <= position && position < span.end) {
        continue;
      }
      return Err(format!(
        "{} at {:?} drops the text at {}: {:?}",
        label,
        span,
        position,
        &source[from..to],
      ));
    }
    Ok(())
  }

  /// Whether the node keeps text of its own outside of its children, which
  /// leaves alphanumeric characters between them (ex. the url of an inline
  /// link, which follows the text it is written under).
  fn holds_its_own_text(node: &Node<'_>) -> bool {
    matches!(node, Node::InlineLink(_) | Node::ReferenceLink(_))
  }

  /// The places within a node where markdown is written with alphanumeric
  /// characters, which are markup rather than text the ast has to hold.
  fn markup_spans(node: &Node<'_>) -> Vec<Span> {
    match node {
      // a list item is marked with `1.` as much as with `-`, and either may be
      // followed by the `[x]` of a task list
      Node::Item(item) => {
        let mut spans = vec![item.marker_span];
        spans.extend(item.marker.as_ref().map(|marker| marker.span));
        spans
      }
      // a paragraph carries the task list marker of the item it opens
      Node::Paragraph(paragraph) => paragraph.marker.as_ref().map(|marker| marker.span).into_iter().collect(),
      // a footnote definition is written under the name it is referred to by,
      // which runs from its start up to the content that follows
      Node::FootnoteDefinition(definition) => match definition.children.first() {
        Some(child) => vec![Span::new(definition.span.start, child.span().start)],
        None => vec![definition.span],
      },
      _ => Vec::new(),
    }
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

fn collect_markdown(directory: &Path, files: &mut Vec<PathBuf>) {
  let Ok(entries) = std::fs::read_dir(directory) else {
    return;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      collect_markdown(&path, files);
    } else if path.extension().is_some_and(|extension| extension == "md") {
      files.push(path);
    }
  }
}

fn parse_spec_file(file_text: &str, path: &Path) -> Vec<SpecCase> {
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

fn spec_files(directory: &Path) -> Vec<PathBuf> {
  let mut files = Vec::new();
  let Ok(entries) = std::fs::read_dir(directory) else {
    return files;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      files.extend(spec_files(&path));
    } else if path.extension().is_some_and(|extension| extension == "txt") {
      files.push(path);
    }
  }
  files.sort();
  files
}
