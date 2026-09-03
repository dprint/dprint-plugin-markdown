//! Runs the CommonMark spec examples through the parser, rendering its ast to
//! html the way the reference implementation does, and checks the html against
//! what the spec says.
//!
//! The same is then done with the formatter's output for each example, which
//! is how formatting is checked to keep what a document says. The formatter
//! decides the whitespace between words itself, so that check is made with the
//! whitespace outside of code collapsed.
//!
//! The examples live under `tests/commonmark`, along with the list of the ones
//! known to fail either check and why. The test fails on any other example
//! that does, and on a listed one that passes, so a fix takes its line out.
//!
//! Set `SPEC_REPORT=<file>` to write the failures out in full, `SPEC_STRICT=1`
//! to report a formatted example that only differs in whitespace, and
//! `SPEC_PRESERVE_CODE=1` to turn on the code block options that would
//! otherwise change what a code block holds.
//!
//! The spec's examples aren't the only markdown this reads: set
//! `COMMONMARK_SPEC=<file>` to a file of the same shape (`markdown`, `html`,
//! `example`, `section`) rendered by another implementation and it is reported
//! on the same way, without the known failures being checked. That is how the
//! parser is compared with another on markdown the spec has no example of.

#[path = "../src/parser/mod.rs"]
#[allow(unused_imports, dead_code)]
mod parser;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use parser::*;

#[derive(serde::Deserialize)]
struct Example {
  markdown: String,
  html: String,
  example: usize,
  section: String,
}

#[test]
fn checks_the_commonmark_spec() {
  let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/commonmark");
  // another file of examples is only reported on, since the known failures
  // are those of the spec's
  let other_examples = std::env::var("COMMONMARK_SPEC").ok().map(PathBuf::from);
  let spec_path = other_examples.clone().unwrap_or_else(|| data_dir.join("spec.json"));
  let examples: Vec<Example> = serde_json::from_str(&std::fs::read_to_string(spec_path).unwrap()).unwrap();
  let entities: HashMap<String, EntityValue> =
    serde_json::from_str(&std::fs::read_to_string(data_dir.join("entities.json")).unwrap()).unwrap();
  let entities: HashMap<String, String> = entities.into_iter().map(|(k, v)| (k, v.characters)).collect();
  let known = KnownFailures::read(&data_dir.join("known_failures.txt"));
  let mut builder = dprint_plugin_markdown::configuration::ConfigurationBuilder::new();
  // the code block options that change what a block holds are turned off
  // with `SPEC_PRESERVE_CODE`, which leaves only what the formatter does
  // whatever its configuration says
  if std::env::var("SPEC_PRESERVE_CODE").is_ok() {
    builder
      .code_block_preserve_indentation(true)
      .code_block_preserve_blank_lines(true);
  }
  let config = builder.build();

  let mut report = String::new();
  let mut sections: Vec<(String, usize, usize, usize, usize)> = Vec::new(); // name, total, parse ok, format ok, format loose ok
  let mut parse_failures = Vec::new();
  let mut format_failures = Vec::new();
  for example in &examples {
    let actual = render_document(&example.markdown, &entities);
    let parse_ok = actual == example.html;

    let formatted =
      std::panic::catch_unwind(|| dprint_plugin_markdown::format_text(&example.markdown, &config, |_, _, _| Ok(None)));
    let (formatted_text, format_note) = match formatted {
      Ok(Ok(Some(text))) => (text, None),
      Ok(Ok(None)) => (example.markdown.clone(), None),
      Ok(Err(err)) => (example.markdown.clone(), Some(format!("format error: {err}"))),
      Err(_) => (example.markdown.clone(), Some("format panic".to_string())),
    };
    let formatted_html = render_document(&formatted_text, &entities);
    let format_ok = format_note.is_none() && formatted_html == example.html;
    // the formatter decides the whitespace between words itself, so it is also
    // compared with the whitespace outside of code collapsed
    let format_loose_ok = format_note.is_none() && loosen(&formatted_html) == loosen(&example.html);

    let entry = match sections.iter_mut().find(|s| s.0 == example.section) {
      Some(entry) => entry,
      None => {
        sections.push((example.section.clone(), 0, 0, 0, 0));
        sections.last_mut().unwrap()
      }
    };
    entry.1 += 1;
    entry.2 += parse_ok as usize;
    entry.3 += format_ok as usize;
    entry.4 += format_loose_ok as usize;
    if !parse_ok {
      parse_failures.push(example.example);
    }
    if !format_loose_ok {
      format_failures.push(example.example);
    }

    let strict = std::env::var("SPEC_STRICT").is_ok();
    if !parse_ok || (if strict { !format_ok } else { !format_loose_ok }) {
      report.push_str(&format!(
        "==== example {} ({}) parse:{} format:{}\n--- markdown\n{}\n--- expected\n{}",
        example.example,
        example.section,
        if parse_ok { "ok" } else { "FAIL" },
        if format_ok { "ok" } else { "FAIL" },
        show(&example.markdown),
        example.html,
      ));
      if !parse_ok {
        report.push_str(&format!("--- parsed\n{}", actual));
      }
      if !format_ok {
        report.push_str(&format!(
          "--- formatted markdown\n{}\n--- formatted html\n{}",
          show(&formatted_text),
          formatted_html
        ));
        if let Some(note) = format_note {
          report.push_str(&format!("--- note\n{note}\n"));
        }
      }
      report.push('\n');
    }
  }

  let mut summary = String::new();
  for (name, total, parse_ok, format_ok, format_loose_ok) in &sections {
    summary.push_str(&format!(
      "{:<45} {:>5}/{:<5} parse  {:>5}/{:<5} format  {:>5}/{:<5} format (loose)\n",
      name, parse_ok, total, format_ok, total, format_loose_ok, total
    ));
  }
  println!("{summary}");
  if let Ok(path) = std::env::var("SPEC_REPORT") {
    std::fs::write(path, format!("{summary}\n{report}")).unwrap();
  }
  if other_examples.is_some() {
    return;
  }

  let mut problems = Vec::new();
  problems.extend(known.compare("parse", &parse_failures, &known.parse));
  // an example the parser doesn't read as the spec says won't format as it
  // says either, so it isn't listed twice
  let expected_format: Vec<usize> = known.format.iter().chain(&known.parse).copied().collect();
  problems.extend(known.compare("format", &format_failures, &expected_format));
  assert!(
    problems.is_empty(),
    "{}\n\nSet SPEC_REPORT=<file> to write out how each failing example differs.",
    problems.join("\n")
  );
}

#[derive(serde::Deserialize)]
struct EntityValue {
  characters: String,
}

/// The examples listed in `known_failures.txt` as not coming out as the spec
/// says.
struct KnownFailures {
  parse: Vec<usize>,
  format: Vec<usize>,
}

impl KnownFailures {
  fn read(path: &Path) -> KnownFailures {
    let mut known = KnownFailures {
      parse: Vec::new(),
      format: Vec::new(),
    };
    for line in std::fs::read_to_string(path).unwrap().lines() {
      let line = line.trim();
      if line.is_empty() || line.starts_with('#') {
        continue;
      }
      let mut words = line.splitn(3, ' ');
      let example: usize = words
        .next()
        .unwrap()
        .parse()
        .unwrap_or_else(|_| panic!("bad line: {line}"));
      match words.next() {
        Some("parse") => known.parse.push(example),
        Some("format") => known.format.push(example),
        _ => panic!("bad line: {line}"),
      }
    }
    known
  }

  /// What differs between the examples that failed a check and the ones that
  /// were expected to, as messages naming each.
  fn compare(&self, check: &str, failed: &[usize], expected: &[usize]) -> Vec<String> {
    let mut problems = Vec::new();
    for example in failed {
      if !expected.contains(example) {
        problems.push(format!("example {example} doesn't {check} as the spec says"));
      }
    }
    for example in expected {
      if !failed.contains(example) {
        problems.push(format!(
          "example {example} now {check}s as the spec says, so its line in known_failures.txt can go"
        ));
      }
    }
    problems
  }
}

/// The html with the whitespace outside of code collapsed, which is what the
/// formatter is free to change.
fn loosen(html: &str) -> String {
  let mut out = String::new();
  let mut rest = html;
  loop {
    let Some(start) = rest.find("<pre>") else {
      out.push_str(&collapse(rest));
      return out;
    };
    out.push_str(&collapse(&rest[..start]));
    let end = rest[start..]
      .find("</pre>")
      .map(|i| start + i + 6)
      .unwrap_or(rest.len());
    out.push_str(&rest[start..end]);
    rest = &rest[end..];
  }
}

fn collapse(html: &str) -> String {
  let mut out = String::new();
  let mut in_space = false;
  for c in html.chars() {
    if c.is_whitespace() {
      in_space = true;
      continue;
    }
    if in_space && !out.ends_with('>') && c != '<' {
      out.push(' ');
    }
    in_space = false;
    out.push(c);
  }
  out
}

fn show(text: &str) -> String {
  text.replace('\t', "→")
}

// ==== the renderer ====

fn render_document(source: &str, entities: &HashMap<String, String>) -> String {
  let file = match parser::parse(source) {
    Ok(file) => file,
    Err(err) => return format!("<!--parse error: {err}-->\n"),
  };
  let mut renderer = Renderer {
    source,
    entities,
    definitions: HashMap::new(),
    out: String::new(),
  };
  renderer.collect_definitions(&file.children);
  renderer.render_blocks(&file.children, false);
  renderer.out
}

struct Renderer<'a> {
  source: &'a str,
  entities: &'a HashMap<String, String>,
  /// The first definition of each normalized label: its destination and title
  /// as written.
  definitions: HashMap<String, (String, Option<String>)>,
  out: String,
}

impl<'a> Renderer<'a> {
  fn collect_definitions(&mut self, nodes: &[Node<'_>]) {
    for node in nodes {
      if let Node::LinkReference(reference) = node {
        let label = normalize_label(&reference.name);
        self.definitions.entry(label).or_insert_with(|| {
          (
            reference.link.to_string(),
            reference.title.as_ref().map(|t| t.to_string()),
          )
        });
      }
      self.collect_definitions(node.children());
      if let Node::Item(item) = node {
        self.collect_definitions(&item.sub_lists);
      }
    }
  }

  fn cr(&mut self) {
    if !self.out.is_empty() && !self.out.ends_with('\n') {
      self.out.push('\n');
    }
  }

  fn render_blocks(&mut self, nodes: &[Node<'_>], tight: bool) {
    for node in nodes {
      self.render_block(node, tight);
    }
  }

  fn render_block(&mut self, node: &Node<'_>, tight: bool) {
    match node {
      Node::Paragraph(paragraph) => {
        if tight {
          self.render_inlines(&paragraph.children);
        } else {
          self.cr();
          self.out.push_str("<p>");
          self.render_inlines(&paragraph.children);
          self.out.push_str("</p>\n");
        }
      }
      Node::Heading(heading) => {
        self.cr();
        self.out.push_str(&format!("<h{}>", heading.level));
        self.render_inlines(&heading.children);
        self.out.push_str(&format!("</h{}>\n", heading.level));
      }
      Node::BlockQuote(quote) => {
        self.cr();
        self.out.push_str("<blockquote>\n");
        self.render_blocks(&quote.children, false);
        self.cr();
        self.out.push_str("</blockquote>\n");
      }
      Node::List(list) => {
        let tight = self.is_tight(list);
        self.cr();
        let close = match list.start_index {
          Some(1) => {
            self.out.push_str("<ol>\n");
            "</ol>\n"
          }
          Some(start) => {
            self.out.push_str(&format!("<ol start=\"{start}\">\n"));
            "</ol>\n"
          }
          None => {
            self.out.push_str("<ul>\n");
            "</ul>\n"
          }
        };
        for item in &list.children {
          self.render_block(item, tight);
        }
        self.out.push_str(close);
      }
      Node::Item(item) => {
        self.cr();
        self.out.push_str("<li>");
        self.render_blocks(&item.children, tight);
        self.render_blocks(&item.sub_lists, tight);
        self.out.push_str("</li>\n");
      }
      Node::CodeBlock(block) => {
        self.cr();
        self.out.push_str("<pre><code");
        if let Some(info) = block.tag() {
          let language = info.split([' ', '\t']).next().unwrap_or("");
          let language = self.decode(language);
          self
            .out
            .push_str(&format!(" class=\"language-{}\"", escape_html(&language)));
        }
        self.out.push('>');
        self.out.push_str(&escape_html(&block.code));
        self.out.push_str("</code></pre>\n");
      }
      Node::Html(html) => {
        self.cr();
        self.out.push_str(&html.text);
        self.out.push('\n');
      }
      Node::HorizontalRule(_) => {
        self.cr();
        self.out.push_str("<hr />\n");
      }
      Node::LinkReference(_) => {}
      Node::MetadataBlock(_) => {
        self.cr();
        self.out.push_str("<!--extension:metadata-->\n");
      }
      Node::Table(_) | Node::DefinitionList(_) | Node::FootnoteDefinition(_) => {
        self.cr();
        self.out.push_str(&format!("<!--extension:{:?}-->\n", node.kind()));
      }
      _ => {
        self.cr();
        self
          .out
          .push_str(&format!("<!--inline at block level:{:?}-->\n", node.kind()));
      }
    }
  }

  /// Whether the list is tight: no blank line between its items, nor between
  /// the blocks directly within any of them.
  fn is_tight(&self, list: &List<'_>) -> bool {
    let items: Vec<&Item<'_>> = list
      .children
      .iter()
      .filter_map(|node| match node {
        Node::Item(item) => Some(item),
        _ => None,
      })
      .collect();
    for pair in items.windows(2) {
      if self.has_blank_between(pair[0].span.end, pair[1].span.start) {
        return false;
      }
    }
    for item in items {
      let blocks: Vec<&Node<'_>> = item.children.iter().chain(item.sub_lists.iter()).collect();
      for pair in blocks.windows(2) {
        if self.has_blank_between(pair[0].span().end, pair[1].span().start) {
          return false;
        }
      }
    }
    true
  }

  fn has_blank_between(&self, start: usize, end: usize) -> bool {
    self.source[start..end].matches('\n').count() >= 2
  }

  fn render_inlines(&mut self, nodes: &[Node<'_>]) {
    for (index, node) in nodes.iter().enumerate() {
      if index > 0 {
        self.render_gap(&nodes[index - 1], node);
      }
      self.render_inline(node);
    }
  }

  /// The whitespace between two inline nodes, which the ast leaves out.
  fn render_gap(&mut self, previous: &Node<'_>, next: &Node<'_>) {
    // the space before a backslash hard break is text, unlike the spaces a
    // two-space break is made of
    let before_backslash = matches!(next, Node::HardBreak(b) if self.source[b.span.start..].starts_with('\\'));
    if matches!(previous, Node::SoftBreak(_) | Node::HardBreak(_))
      || (matches!(next, Node::SoftBreak(_) | Node::HardBreak(_)) && !before_backslash)
    {
      return;
    }
    let gap = &self.source[previous.span().end..next.span().start];
    if gap.bytes().all(|b| matches!(b, b' ' | b'\t')) {
      self.out.push_str(gap);
    } else {
      self.out.push_str(&format!("<!--gap:{}-->", escape_html(gap)));
    }
  }

  fn render_inline(&mut self, node: &Node<'_>) {
    match node {
      Node::Text(text) => {
        let decoded = self.decode(text.text);
        self.out.push_str(&escape_html(&decoded));
      }
      Node::Code(code) => {
        self.out.push_str("<code>");
        self
          .out
          .push_str(&escape_html(&code.code.replace("\r\n", " ").replace('\n', " ")));
        self.out.push_str("</code>");
      }
      Node::TextDecoration(decoration) => {
        let tag = match decoration.kind {
          TextDecorationKind::Emphasis => "em",
          TextDecorationKind::Strong => "strong",
          TextDecorationKind::Strikethrough => "del",
        };
        self.out.push_str(&format!("<{tag}>"));
        self.render_inlines(&decoration.children);
        self.out.push_str(&format!("</{tag}>"));
      }
      Node::Html(html) => self.out.push_str(&html.text),
      Node::SoftBreak(_) => self.out.push('\n'),
      Node::HardBreak(_) => self.out.push_str("<br />\n"),
      Node::InlineLink(link) => {
        let (url, title) = (link.url.to_string(), link.title.as_ref().map(|t| t.to_string()));
        self.render_link_open(&url, title.as_deref());
        self.render_inlines(&link.children);
        self.out.push_str("</a>");
      }
      Node::ReferenceLink(link) => {
        let label = normalize_label(&link.reference);
        let Some((url, title)) = self.definitions.get(&label).cloned() else {
          self.out.push_str("<!--unresolved reference-->");
          return;
        };
        self.render_link_open(&url, title.as_deref());
        self.render_inlines(&link.children);
        self.out.push_str("</a>");
      }
      Node::ShortcutLink(link) => {
        let label = normalize_label(&self.source[link.span.start + 1..link.span.end - 1]);
        let Some((url, title)) = self.definitions.get(&label).cloned() else {
          self.out.push_str("<!--unresolved shortcut-->");
          return;
        };
        self.render_link_open(&url, title.as_deref());
        self.render_inlines(&link.children);
        self.out.push_str("</a>");
      }
      Node::AutoLink(link) => {
        let inner = &self.source[link.span.start + 1..link.span.end - 1];
        let href = if inner.contains(':') {
          inner.to_string()
        } else {
          format!("mailto:{inner}")
        };
        // an autolink holds no escapes, so its destination is read as written
        self.out.push_str(&format!("<a href=\"{}\">", escape_href(&href)));
        self.out.push_str(&escape_html(inner));
        self.out.push_str("</a>");
      }
      Node::InlineImage(image) => {
        let (url, title) = (image.url.to_string(), image.title.as_ref().map(|t| t.to_string()));
        let alt = image.text.to_string();
        self.render_image(&url, &alt, title.as_deref());
      }
      Node::ReferenceImage(image) => {
        let label = if image.reference.is_empty() {
          normalize_label(&image.text)
        } else {
          normalize_label(&image.reference)
        };
        let Some((url, title)) = self.definitions.get(&label).cloned() else {
          self.out.push_str("<!--unresolved image reference-->");
          return;
        };
        let alt = image.text.to_string();
        self.render_image(&url, &alt, title.as_deref());
      }
      Node::ShortcutImage(image) => {
        let label = normalize_label(&image.text);
        let Some((url, title)) = self.definitions.get(&label).cloned() else {
          self.out.push_str("<!--unresolved image shortcut-->");
          return;
        };
        let alt = image.text.to_string();
        self.render_image(&url, &alt, title.as_deref());
      }
      Node::FootnoteReference(_) | Node::InlineMath(_) | Node::DisplayMath(_) => {
        self.out.push_str(&format!("<!--extension:{:?}-->", node.kind()));
      }
      _ => {
        self
          .out
          .push_str(&format!("<!--block at inline level:{:?}-->", node.kind()));
      }
    }
  }

  fn render_link_open(&mut self, url: &str, title: Option<&str>) {
    let href = self.destination(url);
    self.out.push_str(&format!("<a href=\"{}\"", escape_href(&href)));
    if let Some(title) = title {
      let title = self.decode(title);
      self.out.push_str(&format!(" title=\"{}\"", escape_html(&title)));
    }
    self.out.push('>');
  }

  fn render_image(&mut self, url: &str, alt: &str, title: Option<&str>) {
    let src = self.destination(url);
    let alt = self.plain_text(alt);
    self.out.push_str(&format!(
      "<img src=\"{}\" alt=\"{}\"",
      escape_href(&src),
      escape_html(&alt)
    ));
    if let Some(title) = title {
      let title = self.decode(title);
      self.out.push_str(&format!(" title=\"{}\"", escape_html(&title)));
    }
    self.out.push_str(" />");
  }

  /// A destination as written, without the pointy brackets it may be wrapped
  /// in and with its escapes and entities read.
  fn destination(&self, url: &str) -> String {
    let url = url
      .strip_prefix('<')
      .and_then(|rest| rest.strip_suffix('>'))
      .unwrap_or(url);
    self.decode(url)
  }

  /// The alt text of an image, which is the plain text of what its brackets
  /// hold. The definitions of the document are appended so that references
  /// within it resolve the same way.
  fn plain_text(&self, alt: &str) -> String {
    let mut text = alt.to_string();
    for (label, (url, title)) in &self.definitions {
      text.push_str(&format!("\n\n[{label}]: {url}"));
      if let Some(title) = title {
        text.push_str(&format!(" \"{title}\""));
      }
    }
    let Ok(file) = parser::parse(&text) else {
      return String::new();
    };
    let mut out = String::new();
    let renderer = Renderer {
      source: &text,
      entities: self.entities,
      definitions: self.definitions.clone(),
      out: String::new(),
    };
    if let Some(Node::Paragraph(paragraph)) = file.children.first() {
      renderer.plain_inlines(&paragraph.children, &mut out);
    }
    out
  }

  fn plain_inlines(&self, nodes: &[Node<'_>], out: &mut String) {
    for (index, node) in nodes.iter().enumerate() {
      if index > 0 {
        let previous = &nodes[index - 1];
        if !matches!(previous, Node::SoftBreak(_) | Node::HardBreak(_))
          && !matches!(node, Node::SoftBreak(_) | Node::HardBreak(_))
        {
          out.push_str(&self.source[previous.span().end..node.span().start]);
        }
      }
      match node {
        Node::Text(text) => out.push_str(&self.decode(text.text)),
        Node::Code(code) => out.push_str(&code.code),
        Node::SoftBreak(_) | Node::HardBreak(_) => out.push(' '),
        Node::InlineImage(image) => out.push_str(&self.plain_text(&image.text)),
        Node::ReferenceImage(image) => out.push_str(&self.plain_text(&image.text)),
        Node::ShortcutImage(image) => out.push_str(&self.plain_text(&image.text)),
        Node::Html(_) => {}
        other => self.plain_inlines(other.children(), out),
      }
    }
  }

  /// Reads the backslash escapes and entity references of some text.
  fn decode(&self, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
      match bytes[index] {
        b'\\' if index + 1 < bytes.len() && bytes[index + 1].is_ascii_punctuation() => {
          out.push(bytes[index + 1] as char);
          index += 2;
        }
        b'&' => {
          if let Some((end, decoded)) = self.match_entity(&text[index..]) {
            out.push_str(&decoded);
            index += end;
          } else {
            out.push('&');
            index += 1;
          }
        }
        _ => {
          let c = text[index..].chars().next().unwrap();
          out.push(c);
          index += c.len_utf8();
        }
      }
    }
    out
  }

  fn match_entity(&self, text: &str) -> Option<(usize, String)> {
    let end = text.find(';')?;
    let body = &text[1..end];
    if let Some(number) = body.strip_prefix('#') {
      let code = if let Some(hex) = number.strip_prefix(['x', 'X']) {
        if hex.is_empty() || hex.len() > 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
          return None;
        }
        u32::from_str_radix(hex, 16).ok()?
      } else {
        if number.is_empty() || number.len() > 7 || !number.bytes().all(|b| b.is_ascii_digit()) {
          return None;
        }
        number.parse::<u32>().ok()?
      };
      let c = if code == 0 {
        '\u{FFFD}'
      } else {
        char::from_u32(code).unwrap_or('\u{FFFD}')
      };
      return Some((end + 1, c.to_string()));
    }
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_alphanumeric()) {
      return None;
    }
    let value = self.entities.get(&text[..end + 1])?;
    Some((end + 1, value.clone()))
  }
}

fn normalize_label(label: &str) -> String {
  let mut result = String::new();
  let mut had_whitespace = false;
  for c in label.trim_matches([' ', '\t', '\n', '\r']).chars() {
    if matches!(c, ' ' | '\t' | '\n' | '\r') {
      had_whitespace = true;
      continue;
    }
    if had_whitespace {
      result.push(' ');
      had_whitespace = false;
    }
    result.extend(c.to_lowercase());
  }
  result
}

fn escape_html(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  for c in text.chars() {
    match c {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      '"' => out.push_str("&quot;"),
      c => out.push(c),
    }
  }
  out
}

/// Percent-encodes a destination the way the reference implementation does.
fn escape_href(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  for byte in text.bytes() {
    let safe = byte.is_ascii_alphanumeric()
      || matches!(
        byte,
        b'!'
          | b'#'
          | b'$'
          | b'%'
          | b'('
          | b')'
          | b'*'
          | b'+'
          | b','
          | b'-'
          | b'.'
          | b'/'
          | b':'
          | b';'
          | b'='
          | b'?'
          | b'@'
          | b'_'
          | b'~'
      );
    if safe {
      out.push(byte as char);
    } else if byte == b'&' {
      out.push_str("&amp;");
    } else if byte == b'\'' {
      out.push_str("&#x27;");
    } else {
      out.push_str(&format!("%{byte:02X}"));
    }
  }
  out
}
