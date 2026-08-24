use std::collections::HashMap;

use dprint_core::formatting::PrintItemPath;
use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;
use regex::Regex;

use super::utils::*;
use crate::configuration::Configuration;
use crate::format_text;
use crate::format_text::FormatError;
use crate::parser::Span;

type FormatResult = Result<Option<String>, FormatError>;

/// Where a node sits among the ones around it, which decides how some of them
/// have to be written.
#[derive(Default, Clone, Copy)]
pub struct NodePosition {
  /// Whether the node is written directly after a list item's marker, which
  /// takes the place of the indentation its first line would have.
  pub beside_marker: bool,
  /// The character marking the list item the node is written within, where it
  /// is written within one.
  pub marker_char: Option<char>,
  /// Whether the indentation the list item's content is written at lines up
  /// with the column its marker leaves the first line at. Where it doesn't,
  /// nothing that has to keep its own indentation can be written beside the
  /// marker.
  pub marker_lines_up: bool,
  /// Whether a list is written directly above the node, which would take the
  /// indentation of anything indented into its last item.
  pub after_list: bool,
  /// Whether a paragraph is written directly above the node with no blank line
  /// between, which a line of dashes below would turn into a heading.
  pub after_paragraph: bool,
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoizedRcPathKind {
  StartIndent(u32),
  StartWithSingleIndent(u32),
  FinishIndent(u32),
}

pub struct Context<'a> {
  pub file_text: &'a str,
  pub configuration: &'a Configuration,
  /** The current indentation level of what's being printed out. */
  pub indent_level: u32,
  /** The current indentation level within the file being formatted. */
  pub raw_indent_level: u32,
  is_in_list_count: u32,
  /// The indentation level each surrounding block quote started at, from outermost to innermost.
  block_quote_base_indents: Vec<u32>,
  /// The start position of the first child of each surrounding block quote.
  block_quote_content_starts: Vec<Option<usize>>,
  text_wrap_disabled_count: u32,
  decorations_preserved_count: u32,
  enclosing_decoration: Option<Span>,
  /// Where the text that ends an atx heading starts, when the hashes it ends
  /// with have to be escaped so that they aren't read as the heading's closing
  /// sequence.
  escaped_closing_hashes: Option<usize>,
  /// Where each bit of text a paragraph writes at the start of a line starts,
  /// when it has to be escaped so that it isn't read as the start of a block
  /// of its own.
  escaped_block_starts: Vec<(usize, usize)>,
  /// The delimiter each text decoration is written with, by where it starts.
  decoration_delimiters: std::cell::RefCell<HashMap<usize, &'static str>>,
  /// The character the list item being generated is marked with.
  list_marker_char: Option<char>,
  /// Whether the indentation of the list item being generated lines up with
  /// the column its marker leaves the first line at.
  list_marker_lines_up: bool,
  /// Where the node generated next sits.
  next_position: NodePosition,
  pub format_code_block_text: Box<dyn for<'b> FnMut(&str, &'b str, u32) -> FormatResult + 'a>,
  pub ignore_regex: Regex,
  pub ignore_start_regex: Regex,
  pub ignore_end_regex: Regex,
  memoized_rc_paths: HashMap<MemoizedRcPathKind, Option<PrintItemPath>>,
  /// How much each of the paths above indents by, keyed by address, so that
  /// they can be told apart from the paths that hold generated content and
  /// what they do to the indentation is known without stepping into them.
  memoized_rc_path_indents: HashMap<usize, i32>,
}

impl MemoizedRcPathKind {
  /// How much the path made for this indents by.
  fn indent_delta(&self) -> i32 {
    match self {
      MemoizedRcPathKind::StartIndent(times) | MemoizedRcPathKind::StartWithSingleIndent(times) => *times as i32,
      MemoizedRcPathKind::FinishIndent(times) => -(*times as i32),
    }
  }
}

impl<'a> Context<'a> {
  pub fn new(
    file_text: &'a str,
    configuration: &'a Configuration,
    format_code_block_text: impl for<'b> FnMut(&str, &'b str, u32) -> FormatResult + 'a,
  ) -> Context<'a> {
    Context {
      file_text,
      configuration,
      indent_level: 0,
      raw_indent_level: 0,
      is_in_list_count: 0,
      block_quote_base_indents: Vec::new(),
      block_quote_content_starts: Vec::new(),
      text_wrap_disabled_count: 0,
      decorations_preserved_count: 0,
      enclosing_decoration: None,
      escaped_closing_hashes: None,
      escaped_block_starts: Vec::new(),
      decoration_delimiters: std::cell::RefCell::new(HashMap::new()),
      list_marker_char: None,
      list_marker_lines_up: true,
      next_position: NodePosition::default(),
      format_code_block_text: Box::new(format_code_block_text),
      ignore_regex: get_ignore_comment_regex(&configuration.ignore_directive),
      ignore_start_regex: get_ignore_comment_regex(&configuration.ignore_start_directive),
      ignore_end_regex: get_ignore_comment_regex(&configuration.ignore_end_directive),
      memoized_rc_paths: HashMap::new(),
      memoized_rc_path_indents: HashMap::new(),
    }
  }

  pub fn get_memoized_rc_path(&mut self, kind: MemoizedRcPathKind) -> Option<PrintItemPath> {
    if let Some(path) = self.memoized_rc_paths.get(&kind) {
      *path
    } else {
      let mut items = PrintItems::new();
      match kind {
        MemoizedRcPathKind::StartIndent(times) => {
          for _ in 0..times {
            items.push_signal(Signal::StartIndent);
          }
        }
        MemoizedRcPathKind::StartWithSingleIndent(times) => {
          items.push_optional_path(self.get_memoized_rc_path(MemoizedRcPathKind::StartIndent(times)));
          for _ in 0..times {
            items.push_signal(Signal::SingleIndent);
          }
        }
        MemoizedRcPathKind::FinishIndent(times) => {
          for _ in 0..times {
            items.push_signal(Signal::FinishIndent);
          }
        }
      }
      let path = items.into_rc_path();
      self.memoized_rc_paths.insert(kind, path);
      if let Some(path) = path {
        self
          .memoized_rc_path_indents
          .insert(path as *const _ as usize, kind.indent_delta());
      }
      path
    }
  }

  /// Whether the path is one of the memoized paths handed out above, rather
  /// than a path holding generated content.
  pub fn is_memoized_rc_path(&self, path: PrintItemPath) -> bool {
    self.memoized_path_indent_delta(path).is_some()
  }

  /// How much the memoized path indents by, or `None` where the path holds
  /// generated content rather than indentation.
  pub fn memoized_path_indent_delta(&self, path: PrintItemPath) -> Option<i32> {
    self.memoized_rc_path_indents.get(&(path as *const _ as usize)).copied()
  }

  /// Marks being within a block quote whose content starts at `content_start`,
  /// providing the indentation level of every surrounding block quote (from
  /// outermost to innermost) to the provided function.
  pub fn mark_in_block_quotes<T>(
    &mut self,
    content_start: Option<usize>,
    func: impl FnOnce(&mut Context, Vec<u32>) -> T,
  ) -> T {
    let original_is_in_list_count = self.is_in_list_count;
    self.is_in_list_count = 0;
    self.block_quote_base_indents.push(self.indent_level);
    self.block_quote_content_starts.push(content_start);
    let base_indents = self.block_quote_base_indents.clone();
    let items = func(self, base_indents);
    self.block_quote_content_starts.pop();
    self.block_quote_base_indents.pop();
    self.is_in_list_count = original_is_in_list_count;
    items
  }

  pub fn mark_in_list<T>(&mut self, func: impl FnOnce(&mut Context) -> T) -> T {
    self.is_in_list_count += 1;
    let items = func(self);
    self.is_in_list_count -= 1;
    items
  }

  pub fn is_in_list(&self) -> bool {
    self.is_in_list_count > 0
  }

  pub fn is_in_block_quote(&self) -> bool {
    !self.block_quote_base_indents.is_empty()
  }

  /// Whether the position is where the innermost block quote's content starts.
  pub fn is_block_quote_content_start(&self, pos: usize) -> bool {
    self.block_quote_content_starts.last().copied().flatten() == Some(pos)
  }

  /// Whether the position at `index` is preceded by a blank line, accounting for
  /// the block quote markers that prefix a blank line inside a block quote.
  pub fn has_leading_blankline(&self, index: usize) -> bool {
    self.get_leading_blank_lines(index) > 0
  }

  /// The number of blank lines that precede the position at `index`, limited to
  /// the configured maximum.
  pub fn get_leading_blank_lines(&self, index: usize) -> u32 {
    get_leading_blank_lines(
      index,
      self.file_text,
      self.is_in_block_quote(),
      self.configuration.max_blank_lines,
    )
  }

  pub fn with_no_text_wrap<T>(&mut self, func: impl FnOnce(&mut Context) -> T) -> T {
    self.text_wrap_disabled_count += 1;
    let items = func(self);
    self.text_wrap_disabled_count -= 1;
    items
  }

  /// Generates the content of a list item marked with the given character,
  /// which is indented to line up with the column after its marker where
  /// `lines_up` holds.
  pub fn mark_in_list_item<T>(
    &mut self,
    marker: Option<char>,
    lines_up: bool,
    func: impl FnOnce(&mut Context) -> T,
  ) -> T {
    let previous_marker = std::mem::replace(&mut self.list_marker_char, marker);
    let previous_lines_up = std::mem::replace(&mut self.list_marker_lines_up, lines_up);
    let result = func(self);
    self.list_marker_char = previous_marker;
    self.list_marker_lines_up = previous_lines_up;
    result
  }

  /// Marks that a list item's marker was just written out, so that what comes
  /// directly after it can tell it will be sitting beside it.
  pub fn mark_marker_beside(&mut self) {
    self.next_position.beside_marker = true;
    self.next_position.marker_char = self.list_marker_char;
    self.next_position.marker_lines_up = self.list_marker_lines_up;
  }

  /// Marks that a paragraph was just written out with nothing between it and
  /// what comes next, which a line of dashes would underline into a heading.
  pub fn mark_after_paragraph(&mut self) {
    self.next_position.after_paragraph = true;
  }

  /// Marks that a list was just written out, which would take the indentation
  /// of whatever follows it for its own content.
  pub fn mark_after_list(&mut self) {
    self.next_position.after_list = true;
  }

  /// Where the node generated next sits, clearing it so that only that node
  /// sees it.
  pub fn take_position(&mut self) -> NodePosition {
    std::mem::take(&mut self.next_position)
  }

  /// How many block quotes surround what's being generated.
  pub fn block_quote_depth(&self) -> usize {
    self.block_quote_base_indents.len()
  }

  /// Writes out the text decorations within with the characters they were
  /// written with, which the name of a reference depends on.
  pub fn with_preserved_decorations<T>(&mut self, func: impl FnOnce(&mut Context) -> T) -> T {
    self.decorations_preserved_count += 1;
    let result = func(self);
    self.decorations_preserved_count -= 1;
    result
  }

  /// Runs within the content of a text decoration, so that the ones nested in
  /// it know where its own delimiters sit beside them.
  pub fn with_enclosing_decoration<T>(&mut self, content: Option<Span>, func: impl FnOnce(&mut Context) -> T) -> T {
    let previous = std::mem::replace(&mut self.enclosing_decoration, content);
    let result = func(self);
    self.enclosing_decoration = previous;
    result
  }

  /// Generates the content of an atx heading, whose text at `start` ends with
  /// hashes that would otherwise close the heading.
  pub fn with_escaped_closing_hashes<T>(&mut self, start: Option<usize>, func: impl FnOnce(&mut Context) -> T) -> T {
    let previous = std::mem::replace(&mut self.escaped_closing_hashes, start);
    let result = func(self);
    self.escaped_closing_hashes = previous;
    result
  }

  /// Whether the text starting here ends an atx heading with hashes that have
  /// to be escaped.
  pub fn is_escaping_closing_hashes(&self, start: usize) -> bool {
    self.escaped_closing_hashes == Some(start)
  }

  /// Generates the content of a paragraph, whose text at each of `starts` would
  /// be read as the start of a block of its own.
  pub fn with_escaped_block_starts<T>(
    &mut self,
    starts: Vec<(usize, usize)>,
    func: impl FnOnce(&mut Context) -> T,
  ) -> T {
    let previous = std::mem::replace(&mut self.escaped_block_starts, starts);
    let result = func(self);
    self.escaped_block_starts = previous;
    result
  }

  /// Where a backslash goes in the text starting here, when it is written
  /// where a block would start and has to be escaped.
  pub fn block_start_escape_at(&self, start: usize) -> Option<usize> {
    self
      .escaped_block_starts
      .iter()
      .find(|(text_start, _)| *text_start == start)
      .map(|(_, position)| *position)
  }

  pub fn enclosing_decoration(&self) -> Option<Span> {
    self.enclosing_decoration
  }

  /// The delimiter the decoration starting at `start` is written with, worked
  /// out once and kept.
  ///
  /// What one decoration is written with depends on what the ones nested in it
  /// are, so this is what keeps a deeply nested run of them from being worked
  /// out over and over.
  pub fn decoration_delimiter(&self, start: usize, resolve: impl FnOnce() -> &'static str) -> &'static str {
    if let Some(delimiter) = self.decoration_delimiters.borrow().get(&start) {
      return delimiter;
    }
    let delimiter = resolve();
    self.decoration_delimiters.borrow_mut().insert(start, delimiter);
    delimiter
  }

  pub fn is_preserving_decorations(&self) -> bool {
    self.decorations_preserved_count > 0
  }

  pub fn is_text_wrap_disabled(&self) -> bool {
    self.text_wrap_disabled_count > 0
  }

  pub fn format_text<'b>(&mut self, tag: &str, text: &'b str) -> FormatResult {
    let line_width = std::cmp::max(10, self.configuration.line_width as i32 - self.indent_level as i32) as u32;

    match tag {
      "markdown" | "md" => format_text(text, self.configuration, |tag, file_text, line_width| {
        (self.format_code_block_text)(tag, file_text, line_width)
      }),
      _ => (self.format_code_block_text)(tag, text, line_width),
    }
  }

  pub fn get_new_lines_in_range(&self, start: usize, end: usize) -> u32 {
    if end < start {
      return 0;
    } // ignore

    // a line ends at a newline, at a carriage return written on its own, and
    // at a carriage return and the newline that follows it, which is one line
    // ending rather than two
    let file_bytes = self.file_text.as_bytes();
    let mut count = 0;
    for (index, byte) in file_bytes[start..end].iter().enumerate() {
      match byte {
        b'\n' => count += 1,
        b'\r' if file_bytes.get(start + index + 1) != Some(&b'\n') => count += 1,
        _ => {}
      }
    }
    count
  }
}
