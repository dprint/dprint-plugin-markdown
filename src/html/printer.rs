//! Writes a parsed html fragment back out.
//!
//! The printer only ever changes whitespace that isn't rendered: the
//! indentation of an element's children, the runs of whitespace a browser
//! collapses to a single space, and the line the attributes of a long tag are
//! written on. Attribute values, character data, comments and the content of a
//! preformatted or raw text element are all written back byte for byte.
//!
//! Elements are laid out by whether the spec renders them as a block, because
//! that decides whether the whitespace at the edges of their content matters.
//! Everything the tables don't name is treated as inline, which is both what
//! the spec says an unknown element is and the careful assumption: the
//! whitespace around it has to be kept.

use unicode_width::UnicodeWidthStr;

use super::ast::*;
use super::parser;
use super::tags;

pub struct HtmlFormatOptions {
  pub line_width: u32,
  pub use_tabs: bool,
  pub indent_width: u8,
  /// Whether to write a space before the `/>` that closes a self-closing tag.
  pub self_closing_space: bool,
  /// Whether to write an element's content, or a tag's attributes, on the one
  /// line when they fit even where they were written across lines. Otherwise
  /// what was written across lines stays that way and is only reindented.
  pub prefer_single_line: bool,
}

/// Formats an html fragment.
///
/// Returns an error where the html isn't something the formatter can take
/// apart and put back together, which leaves it to be written out as it was.
pub fn format_html<'a>(text: &'a str, options: &HtmlFormatOptions) -> Result<String, ParseError<'a>> {
  let document = parser::parse(text)?;
  let mut printer = Printer {
    options,
    out: String::with_capacity(text.len()),
  };
  printer.print_items(&document.children, 0);
  let result = printer.out;

  // a blank line closes the html block it's written in, so one that wasn't
  // there before would split the block in two and leave the rest of it to be
  // read as markdown
  if has_blank_line(&result) && !has_blank_line(text) {
    return Err(ParseError::WouldSplitTheBlock);
  }
  Ok(result)
}

struct Printer<'a> {
  options: &'a HtmlFormatOptions,
  out: String,
}

impl Printer<'_> {
  /// Writes nodes one item to a line, which is how the children of a block
  /// element and the fragment itself are laid out.
  ///
  /// A block is written on a line of its own; everything between two of them
  /// is one run of text and inline elements, written on a line of its own in
  /// turn. The caller has already written the indentation of the first line.
  fn print_items(&mut self, children: &[Node], indent: usize) {
    let mut wrote_one = false;
    let mut run_start = 0;
    for (index, node) in children.iter().enumerate() {
      if !is_block_node(node) {
        continue;
      }
      self.print_run(&children[run_start..index], indent, &mut wrote_one);
      if wrote_one {
        self.newline(indent);
      }
      self.print_node(node, indent);
      wrote_one = true;
      run_start = index + 1;
    }
    self.print_run(&children[run_start..], indent, &mut wrote_one);
  }

  /// Writes a run of text and inline elements as an item of its own, unless
  /// there is nothing to it but the whitespace that separates two items.
  fn print_run(&mut self, run: &[Node], indent: usize, wrote_one: &mut bool) {
    let Some(run) = trimmed_run(run) else {
      return;
    };
    if *wrote_one {
      self.newline(indent);
    }
    self.print_inline_nodes(run, indent, true, true);
    *wrote_one = true;
  }

  fn print_node(&mut self, node: &Node, indent: usize) {
    match node {
      Node::Element(element) => self.print_element(element, indent),
      Node::Text(text) => self.out.push_str(text),
      Node::Comment(text) | Node::Doctype(text) | Node::ProcessingInstruction(text) | Node::CData(text) => {
        self.out.push_str(text)
      }
    }
  }

  fn print_element(&mut self, element: &Element, indent: usize) {
    self.print_open_tag(element, indent);
    if matches!(element.kind, ElementKind::Void | ElementKind::SelfClosing) {
      return;
    }

    // the content of these is character data, which is rendered as it was
    // written and so can't be touched at all
    if element.kind == ElementKind::RawText || tags::is_preformatted(element.name) {
      self.out.push_str(element.content);
      self.print_close_tag(element);
      return;
    }

    if element.children.is_empty() {
      self.print_close_tag(element);
      return;
    }

    // The content is measured rather than written out and looked at, so that
    // an element whose content doesn't fit isn't written twice. Measuring is
    // what keeps the cost of a deeply nested document from doubling with every
    // level of it.
    // A block's content that was written on lines of its own is kept there
    // unless a single line is preferred. The whitespace at the edges of an
    // inline element's content is rendered, so a line break written there is
    // one the content never fits on a line with anyway.
    let is_block = tags::is_block(element.name);
    let keeps_lines = is_block && !self.options.prefer_single_line && content_on_own_lines(&element.children);
    let fits = !keeps_lines
      && content_flat_width(&element.children, is_block, self.options).is_some_and(|width| {
        self.current_column() + width + close_tag_width(element) <= self.options.line_width as usize
      });
    if keeps_lines && trimmed_run(&element.children).is_none() {
      // nothing but whitespace was written between the tags, so the close tag
      // goes on the line below the open one rather than on one of its own
      // below a line that would hold only indentation
      self.newline(indent);
      self.print_close_tag(element);
      return;
    }
    if fits {
      // every part of the content fits along with the whole of it, so each of
      // them is written on this line in turn
      if is_block {
        self.print_items(&element.children, indent + 1);
      } else {
        self.print_inline_nodes(&element.children, indent + 1, false, false);
      }
      self.print_close_tag(element);
      return;
    }

    // Content that won't fit on the line its tags are on is written on lines
    // of its own, a level further in. The whitespace at the edges of a block's
    // content isn't rendered, so a block can always be opened up this way; an
    // inline element can only be broken where whitespace was already written,
    // because a line break added beside its content is a space that renders.
    let break_start = is_block || starts_with_whitespace(&element.children);
    let break_end = is_block || ends_with_whitespace(&element.children);

    if break_start {
      self.newline(indent + 1);
    }
    if is_block {
      self.print_items(&element.children, indent + 1);
    } else {
      self.print_inline_nodes(&element.children, indent + 1, break_start, break_end);
    }
    if break_end {
      self.newline(indent);
    }
    self.print_close_tag(element);
  }

  /// Writes a run of text and inline elements.
  ///
  /// Runs of whitespace are written as the single space a browser renders them
  /// as, except that one holding a line break is written as a line break so
  /// that the lines the author wrote are kept. When the run is an item of its
  /// own the whitespace at its edges isn't rendered, so it's dropped.
  fn print_inline_nodes(&mut self, nodes: &[Node], indent: usize, trim_start: bool, trim_end: bool) {
    for (index, node) in nodes.iter().enumerate() {
      match node {
        Node::Text(text) => {
          let trim_start = trim_start && index == 0;
          let trim_end = trim_end && index == nodes.len() - 1;
          self.print_collapsed_text(text, indent, trim_start, trim_end);
        }
        _ => self.print_node(node, indent),
      }
    }
  }

  fn print_collapsed_text(&mut self, text: &str, indent: usize, trim_start: bool, trim_end: bool) {
    let mut rest = text;
    if trim_start {
      rest = rest.trim_start_matches(is_html_whitespace);
    }
    if trim_end {
      rest = rest.trim_end_matches(is_html_whitespace);
    }
    let mut chars = rest.char_indices().peekable();
    let mut word_start = None;
    while let Some((index, ch)) = chars.next() {
      if !is_html_whitespace(ch) {
        word_start.get_or_insert(index);
        continue;
      }
      if let Some(start) = word_start.take() {
        self.out.push_str(&rest[start..index]);
      }
      // a run of whitespace renders as a single space, so only whether there
      // is one matters -- but a line break is kept as one to leave the lines
      // the author wrote where they were
      let mut has_newline = ch == '\n';
      while let Some((_, next)) = chars.peek() {
        if !is_html_whitespace(*next) {
          break;
        }
        has_newline |= *next == '\n';
        chars.next();
      }
      if has_newline {
        self.newline(indent);
      } else {
        self.out.push(' ');
      }
    }
    if let Some(start) = word_start {
      self.out.push_str(&rest[start..]);
    }
  }

  fn print_open_tag(&mut self, element: &Element, indent: usize) {
    // a tag written partway along a line isn't the place to start writing
    // attributes on lines of their own: the line it would break is one the
    // tags around it are sharing, and taking it apart reads worse than
    // leaving it long
    if !self.at_line_start() {
      self.write_open_tag(element);
      return;
    }
    // attributes the author wrote on lines of their own are kept there unless
    // a single line is preferred
    let keeps_lines = !self.options.prefer_single_line && attributes_on_own_lines(element);
    let fits = self.current_column() + open_tag_width(element, self.options) <= self.options.line_width as usize;
    // a lone attribute has nowhere better to go, so it stays where it was
    // written rather than being pushed onto a line of its own
    if !keeps_lines && (fits || element.attributes.len() < 2) {
      self.write_open_tag(element);
      return;
    }

    self.out.push('<');
    self.out.push_str(element.name);
    for attribute in &element.attributes {
      self.newline(indent + 1);
      self.write_attribute(attribute);
    }
    self.newline(indent);
    if element.self_closing_syntax {
      self.out.push_str("/>");
    } else {
      self.out.push('>');
    }
  }

  fn write_open_tag(&mut self, element: &Element) {
    self.out.push('<');
    self.out.push_str(element.name);
    for attribute in &element.attributes {
      self.out.push(' ');
      self.write_attribute(attribute);
    }
    if element.self_closing_syntax {
      if self.options.self_closing_space {
        self.out.push(' ');
      }
      self.out.push_str("/>");
    } else {
      self.out.push('>');
    }
  }

  /// Writes an attribute, quoting a value that was written without quotes.
  ///
  /// The value itself is never touched: whitespace within it is rendered for
  /// some attributes and is part of the value for the rest.
  fn write_attribute(&mut self, attribute: &Attribute) {
    self.out.push_str(attribute.name);
    let Some(value) = &attribute.value else {
      return;
    };
    let quote = attribute_quote(value);
    self.out.push('=');
    self.out.push(quote);
    self.out.push_str(value.text);
    self.out.push(quote);
  }

  fn print_close_tag(&mut self, element: &Element) {
    self.out.push_str("</");
    self.out.push_str(element.close_tag_name());
    self.out.push('>');
  }

  fn newline(&mut self, indent: usize) {
    self.out.push('\n');
    for _ in 0..indent {
      if self.options.use_tabs {
        self.out.push('\t');
      } else {
        for _ in 0..self.options.indent_width {
          self.out.push(' ');
        }
      }
    }
  }

  /// Whether nothing but indentation has been written on the line so far,
  /// which is what gives a tag the room to lay its attributes out.
  fn at_line_start(&self) -> bool {
    let line = match self.out.rfind('\n') {
      Some(index) => &self.out[index + 1..],
      None => &self.out[..],
    };
    line.chars().all(|ch| ch == ' ' || ch == '\t')
  }

  fn current_column(&self) -> usize {
    match self.out.rfind('\n') {
      Some(index) => self.out[index + 1..].width(),
      None => self.out.width(),
    }
  }
}

/// The run without the whitespace at its edges, or `None` where there is
/// nothing to it but that whitespace.
///
/// Text that is only whitespace separates one item from the next, so it is
/// dropped -- but only where it falls between items, because within a run the
/// space between two inline elements is rendered.
fn trimmed_run<'a, 'b>(run: &'b [Node<'a>]) -> Option<&'b [Node<'a>]> {
  let start = run.iter().position(|node| !is_blank_text(node))?;
  let end = run.iter().rposition(|node| !is_blank_text(node)).unwrap();
  Some(&run[start..=end])
}

/// Whether the whitespace an element's content begins with was written by the
/// author, which is where a line break can be put without adding one.
fn starts_with_whitespace(children: &[Node]) -> bool {
  matches!(children.first(), Some(Node::Text(text)) if text.starts_with(is_html_whitespace))
}

/// Whether the whitespace an element's content ends with was written by the
/// author, which is where a line break can be put without adding one.
fn ends_with_whitespace(children: &[Node]) -> bool {
  matches!(children.last(), Some(Node::Text(text)) if text.ends_with(is_html_whitespace))
}

/// Whether the author wrote an element's content on lines of its own, with a
/// line break directly after the open tag or directly before the close tag.
fn content_on_own_lines(children: &[Node]) -> bool {
  let starts_on_own_line = matches!(
    children.first(),
    Some(Node::Text(text)) if leading_whitespace(text).contains('\n')
  );
  let ends_on_own_line = matches!(
    children.last(),
    Some(Node::Text(text)) if trailing_whitespace(text).contains('\n')
  );
  starts_on_own_line || ends_on_own_line
}

fn leading_whitespace(text: &str) -> &str {
  &text[..text.len() - text.trim_start_matches(is_html_whitespace).len()]
}

fn trailing_whitespace(text: &str) -> &str {
  &text[text.trim_end_matches(is_html_whitespace).len()..]
}

/// Whether the author wrote a tag's attributes on lines of their own, with a
/// line break between the tag name and the first of them.
///
/// The whitespace within a tag isn't kept by the parser, so this reads it back
/// off the source the element was parsed out of, which the name and the
/// attributes borrow from.
fn attributes_on_own_lines(element: &Element) -> bool {
  let Some(first) = element.attributes.first() else {
    return false;
  };
  let source_start = element.source.as_ptr() as usize;
  let name_end = element.name.as_ptr() as usize + element.name.len() - source_start;
  let first_start = first.name.as_ptr() as usize - source_start;
  element.source[name_end..first_start].contains('\n')
}

fn is_block_node(node: &Node) -> bool {
  match node {
    Node::Element(element) => tags::is_block(element.name),
    Node::Doctype(_) => true,
    _ => false,
  }
}

fn is_blank_text(node: &Node) -> bool {
  matches!(node, Node::Text(text) if text.chars().all(is_html_whitespace))
}

/// The quote to write a value with, which is the one it was written with
/// unless it was written without any.
fn attribute_quote(value: &AttributeValue) -> char {
  match value.quote {
    Some(quote) => quote,
    // an unquoted value can always be written with quotes, so long as the
    // quote isn't one of the characters in it
    None if value.text.contains('"') => '\'',
    None => '"',
  }
}

/// The columns an element's content takes when it is written on one line, or
/// `None` when it can't be: it holds a block, or whitespace that has to stay a
/// line break, or text that is written back out with a line break in it.
///
/// This mirrors what the printer writes, so that measuring an element and
/// writing it out agree. Measuring is what keeps a deeply nested document from
/// costing twice as much for every level of it, which is what writing the
/// content out only to find it doesn't fit would do.
fn content_flat_width(children: &[Node], is_block: bool, options: &HtmlFormatOptions) -> Option<usize> {
  if children.iter().any(is_block_node) {
    return None;
  }
  if !is_block {
    // the whitespace at the edges of an inline element's content is rendered,
    // so it is measured rather than trimmed away
    return run_flat_width(children, false, false, options);
  }
  // with no block among them the children are the one run the printer writes,
  // without the whitespace at its edges
  match trimmed_run(children) {
    Some(run) => run_flat_width(run, true, true, options),
    None => Some(0),
  }
}

fn run_flat_width(nodes: &[Node], trim_start: bool, trim_end: bool, options: &HtmlFormatOptions) -> Option<usize> {
  let mut total = 0;
  for (index, node) in nodes.iter().enumerate() {
    total += match node {
      Node::Text(text) => collapsed_text_width(text, trim_start && index == 0, trim_end && index == nodes.len() - 1)?,
      Node::Element(element) => element_flat_width(element, options)?,
      Node::Comment(text) | Node::Doctype(text) | Node::ProcessingInstruction(text) | Node::CData(text) => {
        verbatim_width(text)?
      }
    };
  }
  Some(total)
}

fn element_flat_width(element: &Element, options: &HtmlFormatOptions) -> Option<usize> {
  // A value written across lines takes the tag with it. Attributes being kept
  // on lines of their own don't come into it: only a block is given a line of
  // its own to write them from, and a block is never measured as part of a
  // run, while an inline element sits within one and its attributes stay on
  // the line the run is on.
  if element
    .attributes
    .iter()
    .any(|attribute| attribute.value.as_ref().is_some_and(|value| value.text.contains('\n')))
  {
    return None;
  }
  let open = open_tag_width(element, options);
  if matches!(element.kind, ElementKind::Void | ElementKind::SelfClosing) {
    return Some(open);
  }
  if element.kind == ElementKind::RawText || tags::is_preformatted(element.name) {
    return Some(open + verbatim_width(element.content)? + close_tag_width(element));
  }
  let content = content_flat_width(&element.children, tags::is_block(element.name), options)?;
  Some(open + content + close_tag_width(element))
}

fn open_tag_width(element: &Element, options: &HtmlFormatOptions) -> usize {
  let mut width = 1 + element.name.width();
  for attribute in &element.attributes {
    width += 1 + attribute.name.width();
    if let Some(value) = &attribute.value {
      // the two quotes and the equals sign
      width += 3 + value.text.width();
    }
  }
  width += if element.self_closing_syntax {
    if options.self_closing_space {
      3
    } else {
      2
    }
  } else {
    1
  };
  width
}

/// The columns text written back out as it was takes, or `None` where it holds
/// a line break and so takes more than one line.
fn verbatim_width(text: &str) -> Option<usize> {
  (!text.contains('\n')).then(|| text.width())
}

/// The columns text takes once the runs of whitespace within it are collapsed,
/// or `None` where one of them holds a line break the printer keeps.
fn collapsed_text_width(text: &str, trim_start: bool, trim_end: bool) -> Option<usize> {
  let mut rest = text;
  if trim_start {
    rest = rest.trim_start_matches(is_html_whitespace);
  }
  if trim_end {
    rest = rest.trim_end_matches(is_html_whitespace);
  }
  let mut width = 0;
  let mut word_start = None;
  let mut chars = rest.char_indices().peekable();
  while let Some((index, ch)) = chars.next() {
    if !is_html_whitespace(ch) {
      word_start.get_or_insert(index);
      continue;
    }
    if let Some(start) = word_start.take() {
      width += rest[start..index].width();
    }
    if ch == '\n' {
      return None;
    }
    while let Some((_, next)) = chars.peek() {
      if !is_html_whitespace(*next) {
        break;
      }
      if *next == '\n' {
        return None;
      }
      chars.next();
    }
    // the run renders as the single space it is written back out as
    width += 1;
  }
  if let Some(start) = word_start {
    width += rest[start..].width();
  }
  Some(width)
}

fn close_tag_width(element: &Element) -> usize {
  element.name.width() + 3
}

fn is_html_whitespace(ch: char) -> bool {
  matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{000C}')
}

/// Whether any line of the text holds nothing but whitespace, which is what
/// closes an html block in markdown.
///
/// The empty line that text ending in a newline leaves behind isn't one of
/// them, because there is no line there at all.
fn has_blank_line(text: &str) -> bool {
  let mut lines: Vec<&str> = text.split('\n').collect();
  if lines.last() == Some(&"") {
    lines.pop();
  }
  lines
    .iter()
    .any(|line| line.trim_matches(is_html_whitespace).is_empty())
}

#[cfg(test)]
mod test {
  use super::*;

  #[track_caller]
  fn assert_format(text: &str, expected: &str) {
    let actual = format_with_width(text, 80);
    assert_eq!(actual, expected, "\n  input: {:?}", text);
    // formatting is only worth anything if it settles, which is why the spec
    // tests run every file through twice as well
    let again = format_with_width(&actual, 80);
    assert_eq!(again, actual, "formatting a second time changed the text");
  }

  #[track_caller]
  fn format_with_width(text: &str, line_width: u32) -> String {
    format_html(
      text,
      &HtmlFormatOptions {
        line_width,
        use_tabs: false,
        indent_width: 2,
        self_closing_space: true,
        prefer_single_line: true,
      },
    )
    .unwrap_or_else(|err| panic!("expected {:?} to format, but: {}", text, err))
  }

  /// Asserts the html is left to be written out as it was.
  #[track_caller]
  fn assert_kept(text: &str) {
    let result = format_html(
      text,
      &HtmlFormatOptions {
        line_width: 80,
        use_tabs: false,
        indent_width: 2,
        self_closing_space: true,
        prefer_single_line: true,
      },
    );
    assert!(result.is_err(), "expected {:?} to be kept, but got {:?}", text, result);
  }

  // ==== indentation ====

  #[test]
  fn indents_nested_blocks() {
    assert_format("<div><p>a</p></div>", "<div>\n  <p>a</p>\n</div>");
  }

  #[test]
  fn reindents_blocks_that_were_written_flat() {
    assert_format(
      "<table>\n<tr><td>a</td><td>b</td></tr>\n</table>",
      "<table>\n  <tr>\n    <td>a</td>\n    <td>b</td>\n  </tr>\n</table>",
    );
  }

  #[test]
  fn reindents_blocks_that_were_over_indented() {
    assert_format(
      "<div>\n        <p>a</p>\n            <p>b</p>\n</div>",
      "<div>\n  <p>a</p>\n  <p>b</p>\n</div>",
    );
  }

  #[test]
  fn keeps_a_block_with_short_inline_content_on_one_line() {
    assert_format("<p>a</p>", "<p>a</p>");
    assert_format("<div><span>a</span></div>", "<div><span>a</span></div>");
  }

  #[test]
  fn drops_the_whitespace_at_the_edges_of_block_content() {
    assert_format("<div>   <p>a</p>   </div>", "<div>\n  <p>a</p>\n</div>");
    assert_format("<p>   a   </p>", "<p>a</p>");
  }

  #[test]
  fn indents_deeply_nested_blocks() {
    assert_format(
      "<div><section><article><p>a</p></article></section></div>",
      "<div>\n  <section>\n    <article>\n      <p>a</p>\n    </article>\n  </section>\n</div>",
    );
  }

  // ==== whitespace ====

  #[test]
  fn keeps_the_whitespace_around_an_inline_element() {
    assert_format("<p>some <em>text</em> here</p>", "<p>some <em>text</em> here</p>");
    assert_format("<p>a<em>b</em>c</p>", "<p>a<em>b</em>c</p>");
  }

  #[test]
  fn collapses_a_run_of_whitespace_to_the_space_it_renders_as() {
    assert_format("<p>a     b</p>", "<p>a b</p>");
    assert_format("<p>a \t  b</p>", "<p>a b</p>");
  }

  #[test]
  fn keeps_a_line_break_the_author_wrote() {
    assert_format("<p>a\nb</p>", "<p>\n  a\n  b\n</p>");
  }

  #[test]
  fn keeps_the_whitespace_between_two_inline_elements() {
    assert_format(
      "<p><span>a</span> <span>b</span></p>",
      "<p><span>a</span> <span>b</span></p>",
    );
    assert_format(
      "<p><span>a</span><span>b</span></p>",
      "<p><span>a</span><span>b</span></p>",
    );
  }

  #[test]
  fn treats_an_unknown_element_as_inline() {
    assert_format(
      "<p>a <my-widget>b</my-widget> c</p>",
      "<p>a <my-widget>b</my-widget> c</p>",
    );
  }

  // ==== an element whose content spans lines ====

  #[test]
  fn lays_out_an_inline_element_that_spans_lines() {
    // the author wrote whitespace at both edges, so a line break can be put at
    // either without adding one that renders
    assert_format(
      "<div><a href=\"x\">\n<b>b</b>\ntext\n</a></div>",
      "<div>\n  <a href=\"x\">\n    <b>b</b>\n    text\n  </a>\n</div>",
    );
  }

  #[test]
  fn only_breaks_an_inline_edge_that_was_written_with_whitespace() {
    // no whitespace after the open tag, so the content stays beside it -- a
    // line break there would render as a space the author didn't write
    assert_format(
      "<div><a href=\"x\">Get started\n<b>b</b>\n</a></div>",
      "<div>\n  <a href=\"x\">Get started\n    <b>b</b>\n  </a>\n</div>",
    );
    // and none at either edge, so neither can be broken
    assert_format(
      "<div><a href=\"x\">Get started\n<b>b</b></a></div>",
      "<div>\n  <a href=\"x\">Get started\n    <b>b</b></a>\n</div>",
    );
  }

  #[test]
  fn indents_the_content_of_a_nested_inline_element() {
    assert_format(
      "<div><p>\n<a href=\"x\">\n<b>b</b>\n</a>\n</p></div>",
      "<div>\n  <p>\n    <a href=\"x\">\n      <b>b</b>\n    </a>\n  </p>\n</div>",
    );
  }

  #[test]
  fn does_not_break_an_inline_element_that_fits() {
    assert_format(
      "<div><p>a <em>b</em> c</p></div>",
      "<div>\n  <p>a <em>b</em> c</p>\n</div>",
    );
  }

  // ==== content that is kept byte for byte ====

  #[test]
  fn keeps_the_content_of_a_preformatted_element() {
    assert_format("<pre>  a   b\n   c  </pre>", "<pre>  a   b\n   c  </pre>");
    assert_format("<div><pre>  a  </pre></div>", "<div>\n  <pre>  a  </pre>\n</div>");
  }

  #[test]
  fn keeps_the_tags_written_within_a_preformatted_element() {
    let text = "<pre>
<a href=\"https://example.com\">a</a>
</pre>";
    assert_format(text, text);
    assert_format("<pre><b>a</b>  <i>b</i></pre>", "<pre><b>a</b>  <i>b</i></pre>");
  }

  #[test]
  fn keeps_the_content_of_a_raw_text_element() {
    assert_format("<script>const a = 1</script>", "<script>const a = 1</script>");
    assert_format(
      "<style>.a {   color: red }</style>",
      "<style>.a {   color: red }</style>",
    );
  }

  #[test]
  fn reads_markup_within_a_raw_text_element_as_text() {
    assert_format("<script>if (a < b) { }</script>", "<script>if (a < b) { }</script>");
    let quoted_close = "<script>const a = \"</div>\"</script>";
    assert_format(quoted_close, quoted_close);
  }

  #[test]
  fn keeps_a_comment_as_it_was_written() {
    assert_format("<!-- a comment -->", "<!-- a comment -->");
    assert_format(
      "<div>\n<!--   spaced   -->\n<p>a</p>\n</div>",
      "<div>\n  <!--   spaced   -->\n  <p>a</p>\n</div>",
    );
  }

  #[test]
  fn keeps_a_doctype_cdata_and_processing_instruction() {
    assert_format("<!DOCTYPE html>", "<!DOCTYPE html>");
    assert_format("<p><![CDATA[ a < b ]]></p>", "<p><![CDATA[ a < b ]]></p>");
    assert_format("<p><?php echo 1; ?></p>", "<p><?php echo 1; ?></p>");
  }

  // ==== tags ====

  #[test]
  fn keeps_the_case_a_tag_was_written_in() {
    assert_format("<DIV><P>a</P></DIV>", "<DIV>\n  <P>a</P>\n</DIV>");
  }

  #[test]
  fn writes_a_void_element_the_way_it_was_written() {
    assert_format("<br>", "<br>");
    assert_format("<br />", "<br />");
    assert_format("<img src=\"a.png\">", "<img src=\"a.png\">");
  }

  #[test]
  fn writes_the_space_before_a_self_closing_slash_the_way_the_option_says() {
    let without_space = |text: &str| {
      format_html(
        text,
        &HtmlFormatOptions {
          line_width: 80,
          use_tabs: false,
          indent_width: 2,
          self_closing_space: false,
          prefer_single_line: true,
        },
      )
      .unwrap()
    };
    // on by default, so a tag is written the one way however it was written
    assert_format("<br/>", "<br />");
    assert_format("<br />", "<br />");
    assert_format("<svg><circle cx=\"1\"/></svg>", "<svg><circle cx=\"1\" /></svg>");
    // and off, the other way
    assert_eq!(without_space("<br />"), "<br/>");
    assert_eq!(without_space("<br/>"), "<br/>");
    assert_eq!(
      without_space("<svg><circle cx=\"1\" /></svg>"),
      "<svg><circle cx=\"1\"/></svg>"
    );
  }

  #[test]
  fn reads_a_self_closing_tag_within_svg() {
    assert_format("<svg><circle cx=\"1\" /></svg>", "<svg><circle cx=\"1\" /></svg>");
  }

  #[test]
  fn keeps_an_empty_element_on_one_line() {
    assert_format("<div></div>", "<div></div>");
    assert_format("<div><p></p></div>", "<div>\n  <p></p>\n</div>");
  }

  // ==== attributes ====

  #[test]
  fn quotes_a_value_that_was_written_without_quotes() {
    assert_format("<a href=foo>a</a>", "<a href=\"foo\">a</a>");
  }

  #[test]
  fn keeps_the_quote_a_value_was_written_with() {
    assert_format("<a href='foo'>a</a>", "<a href='foo'>a</a>");
    assert_format("<a title='say \"hi\"'>a</a>", "<a title='say \"hi\"'>a</a>");
  }

  #[test]
  fn writes_an_attribute_that_has_no_value() {
    assert_format("<input disabled>", "<input disabled>");
    assert_format(
      "<button disabled type=\"submit\">a</button>",
      "<button disabled type=\"submit\">a</button>",
    );
  }

  #[test]
  fn keeps_an_attribute_value_byte_for_byte() {
    assert_format("<div title=\"a    b\">x</div>", "<div title=\"a    b\">x</div>");
  }

  #[test]
  fn breaks_a_long_list_of_attributes_onto_lines_of_their_own() {
    assert_format(
      "<div class=\"one two three four five six\" id=\"the-identifier\" data-value=\"something long\">a</div>",
      "<div\n  class=\"one two three four five six\"\n  id=\"the-identifier\"\n  data-value=\"something long\"\n>a</div>",
    );
  }

  #[test]
  fn only_lays_out_the_attributes_of_a_tag_that_starts_its_line() {
    // the svg is written partway along a line its siblings are sharing, so
    // taking its tag apart there would read worse than leaving the line long
    let span = "<span class=\"ic\"><svg viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\"></svg></span>";
    assert_eq!(
      format_with_width(&format!("<div>{}</div>", span), 40),
      format!("<div>\n  {}\n</div>", span),
    );
    // on a line of its own it has the room
    assert_eq!(
      format_with_width("<div viewBox=\"0 0 24 24\" fill=\"none\"><p>a</p></div>", 20),
      "<div\n  viewBox=\"0 0 24 24\"\n  fill=\"none\"\n>\n  <p>a</p>\n</div>",
    );
  }

  #[test]
  fn keeps_a_lone_attribute_on_the_line_its_tag_is_on() {
    // there is nowhere better for it to go, so a long list of classes is left
    // where it was written rather than being pushed onto a line of its own
    let long = "flex items-center justify-between rounded-lg border px-4 py-2 text-sm font-medium shadow";
    assert_format(
      &format!("<div class=\"{}\">a</div>", long),
      &format!("<div class=\"{}\">\n  a\n</div>", long),
    );
  }

  // ==== html that is left as it was written ====

  #[test]
  fn keeps_an_element_that_was_never_closed() {
    assert_kept("<div align=\"center\">");
    assert_kept("<div><p>a</p>");
  }

  #[test]
  fn keeps_a_closing_tag_that_closes_nothing() {
    assert_kept("</div>");
    assert_kept("<p>a</p></div>");
  }

  #[test]
  fn keeps_tags_that_dont_match() {
    assert_kept("<div><p>a</div></p>");
  }

  #[test]
  fn keeps_html_that_leaves_a_closing_tag_to_be_inferred() {
    assert_kept("<ul><li>a<li>b</ul>");
  }

  #[test]
  fn keeps_a_self_closing_tag_that_a_browser_reads_as_an_opening_one() {
    assert_kept("<div/>");
    assert_kept("<span/>a</span>");
  }

  #[test]
  fn keeps_markup_that_was_never_terminated() {
    assert_kept("<!-- a comment");
    assert_kept("<p title=\"a</p>");
  }

  // ==== text that only looks like markup ====

  #[test]
  fn reads_an_angle_bracket_that_opens_no_tag_as_text() {
    assert_format("<p>a < b</p>", "<p>a < b</p>");
    assert_format("<p>1 <2</p>", "<p>1 <2</p>");
  }

  // ==== line width ====

  #[test]
  fn breaks_content_that_runs_past_the_line_width() {
    assert_eq!(
      format_with_width(
        "<div><span>this content is long enough that it will not fit</span></div>",
        40
      ),
      "<div>\n  <span>this content is long enough that it will not fit</span>\n</div>",
    );
  }

  #[test]
  fn keeps_content_that_fits_on_the_line_it_is_on() {
    assert_eq!(
      format_with_width("<div><span>a</span></div>", 40),
      "<div><span>a</span></div>"
    );
  }

  // ==== text the printer must not touch ====

  #[test]
  fn keeps_a_character_entity_as_it_was_written() {
    assert_format("<p>a &amp; b &#38; c</p>", "<p>a &amp; b &#38; c</p>");
    assert_format("<p>&nbsp;&nbsp;</p>", "<p>&nbsp;&nbsp;</p>");
  }

  #[test]
  fn does_not_collapse_a_no_break_space() {
    // it isn't whitespace a browser collapses, so it is text like any other
    assert_format("<p>a\u{a0}\u{a0}b</p>", "<p>a\u{a0}\u{a0}b</p>");
  }

  #[test]
  fn keeps_an_attribute_value_written_across_lines() {
    assert_format(
      "<div class=\"one\n  two\" id=\"b\"><p>a</p></div>",
      "<div class=\"one\n  two\" id=\"b\">\n  <p>a</p>\n</div>",
    );
  }

  #[test]
  fn keeps_a_comment_holding_text_that_looks_like_markup() {
    assert_format("<!-- <div> not a tag -->", "<!-- <div> not a tag -->");
  }

  // ==== whitespace within a run ====

  #[test]
  fn collapses_a_tab_the_way_a_browser_does() {
    assert_format("<p>a\t\tb</p>", "<p>a b</p>");
  }

  #[test]
  fn keeps_a_void_element_within_a_run_of_text() {
    assert_format("<p>a<br>b</p>", "<p>a<br>b</p>");
    assert_format("<p>a <br> b</p>", "<p>a <br> b</p>");
  }

  #[test]
  fn lays_out_a_run_that_holds_a_block_beside_it() {
    assert_format(
      "<div>text before<p>a</p>text after</div>",
      "<div>\n  text before\n  <p>a</p>\n  text after\n</div>",
    );
  }

  // ==== measuring ====

  #[test]
  fn measures_a_wide_character_by_the_room_it_takes() {
    // each of these takes two columns, so the content is 40 columns wide and
    // doesn't fit beside the tags within a width of 30
    let wide = "\u{6f22}".repeat(20);
    assert_eq!(
      format_with_width(&format!("<div><span>{}</span></div>", wide), 30),
      format!("<div>\n  <span>{}</span>\n</div>", wide),
    );
  }

  #[test]
  fn lays_out_a_deeply_nested_tree() {
    assert_format(
      "<div><section><ul><li><p>a <em>b</em> c</p></li></ul></section></div>",
      "<div>\n  <section>\n    <ul>\n      <li>\n        <p>a <em>b</em> c</p>\n      </li>\n    </ul>\n  </section>\n</div>",
    );
  }
}
