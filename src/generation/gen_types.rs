use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use dprint_core::formatting::PrintItemPath;
use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;

use super::utils::*;
use crate::configuration::Configuration;
use crate::format_text;
use crate::format_text::FormatError;
use crate::parser::Span;

type FormatResult = Result<Option<String>, FormatError>;

/// The error the plugin that formats the code within a code block ran into,
/// along with where the code block was written.
///
/// The line it's reported on is worked out once the file has been generated,
/// where the text the file began with is known.
pub struct CodeBlockError {
  /// Where the code block starts within the text being generated.
  pub pos: usize,
  pub error: FormatError,
}

/// The line breaks that aren't written out, and the characters that end up
/// written against each other in their place.
///
/// Both are held by where the node beside the break begins or ends, in order,
/// so that one can be looked up by that.
#[derive(Default)]
pub struct DroppedBreaks {
  /// The last character written before the node beginning at each position.
  pub before: Vec<(usize, char)>,
  /// The first character written after the node ending at each position.
  pub after: Vec<(usize, char)>,
}

/// The runs of `*` and `_` within the block being written that could be read
/// as emphasis delimiters, which is what decides whether a text decoration can
/// be written with a character other than the one it was written with.
#[derive(Default)]
pub struct DelimiterRuns {
  /// Where each text decoration within the block sits, in order.
  pub decorations: Vec<Span>,
  /// Each run of a delimiter character within the text of the block that can
  /// open or close emphasis where it sits, with that character, in order.
  pub pairable: Vec<(Span, char)>,
}

impl DelimiterRuns {
  /// Whether a text decoration ends right where `start` is.
  pub fn has_decoration_ending_at(&self, start: usize) -> bool {
    self.decorations.iter().any(|span| span.end == start)
  }

  /// Whether a text decoration begins right where `end` is.
  pub fn has_decoration_starting_at(&self, end: usize) -> bool {
    self.decorations.iter().any(|span| span.start == end)
  }

  /// The decorations the one at `span` is written within, from outermost to
  /// innermost.
  pub fn decorations_around(&self, span: Span) -> impl Iterator<Item = Span> + '_ {
    self
      .decorations
      .iter()
      .copied()
      .filter(move |other| other.start < span.start && other.end > span.end)
  }

  /// Whether the text of the block holds a run of the character that can open
  /// or close emphasis, other than one written directly against the decoration
  /// at `span`.
  pub fn has_pairable_run_away_from(&self, span: Span, character: char) -> bool {
    self
      .pairable
      .iter()
      .any(|(run, c)| *c == character && run.end != span.start && run.start != span.end)
  }
}

/// What the paragraph being written has to be written with in order to be read
/// back as the paragraph it is.
pub struct ParagraphEscapes {
  /// The text the paragraph writes at the start of a line that would be read
  /// as the start of a block of its own.
  pub block_starts: Vec<BlockStartEscape>,
  /// Where each bit of text the paragraph writes at the start of a line
  /// starts.
  pub line_starts: LineStarts,
  /// Where the paragraph ends, which bounds how far the text of one of its
  /// lines can run.
  pub end: usize,
}

/// Where each bit of text a paragraph writes at the start of a line starts.
///
/// Only a hard break puts anything but a paragraph's first node at the start of
/// a line, so almost every paragraph has just the one. That one is held beside
/// the rest rather than in a vector of its own, which most paragraphs then
/// never need.
#[derive(Default)]
pub struct LineStarts {
  first: Option<usize>,
  rest: Vec<usize>,
}

impl LineStarts {
  pub fn push(&mut self, start: usize) {
    match self.first {
      None => self.first = Some(start),
      Some(_) => self.rest.push(start),
    }
  }

  pub fn contains(&self, start: usize) -> bool {
    self.first == Some(start) || self.rest.contains(&start)
  }
}

/// Where a backslash goes so that a bit of a paragraph's text isn't read as the
/// start of a block of its own.
pub struct BlockStartEscape {
  /// Where the text that would be read that way starts.
  pub text_start: usize,
  /// Where within that text the backslash goes.
  pub position: usize,
}

/// Where a node sits among the ones around it, which decides how some of them
/// have to be written.
#[derive(Default, Clone, Copy)]
pub struct NodePosition {
  /// The marker of the list item the node is written directly after, which
  /// takes the place of the indentation its first line would have.
  pub marker: Option<ListItemMarker>,
  /// Whether a list is written directly above the node, which would take the
  /// indentation of anything indented into its last item.
  pub after_list: bool,
  /// Whether a paragraph is written directly above the node with no blank line
  /// between, which a line of dashes below would turn into a heading.
  pub after_paragraph: bool,
}

/// The marker of a list item, which decides how what is written beside it may
/// be written.
#[derive(Clone, Copy)]
pub struct ListItemMarker {
  /// The character the item is marked with, where it is marked with one rather
  /// than numbered.
  pub char: Option<char>,
  /// Whether the indentation the item's content is written at lines up with
  /// the column its marker leaves the first line at. Where it doesn't, nothing
  /// that has to keep its own indentation can be written beside the marker.
  pub lines_up: bool,
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
  /// The current indentation level of what's being printed out.
  pub indent_level: u32,
  /// The current indentation level within the file being formatted.
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
  escaped_block_starts: Vec<BlockStartEscape>,
  /// Where each bit of text written at the start of a line starts.
  line_start_texts: LineStarts,
  /// Where the block being written ends, which bounds how far the text of one
  /// of its lines can run.
  block_end: Option<usize>,
  /// The line breaks between the nodes being written that aren't written out.
  dropped_breaks: DroppedBreaks,
  /// The delimiter runs of the block being written, once its content is being
  /// written.
  delimiter_runs: Option<DelimiterRuns>,
  /// The delimiter each text decoration is written with, by where it starts.
  decoration_delimiters: std::cell::RefCell<HashMap<usize, &'static str>>,
  /// The marker of the list item being generated.
  list_marker: ListItemMarker,
  /// Where the node generated next sits.
  next_position: NodePosition,
  pub format_code_block_text: Box<dyn for<'b> FnMut(&str, &'b str, u32) -> FormatResult + 'a>,
  /// The first error a code block's plugin ran into, which fails the file once
  /// it's been generated when `codeBlock.raiseSyntaxErrors` is on.
  ///
  /// It's held outside the context so that it's still there to be read once
  /// the context has been dropped.
  code_block_error: Rc<RefCell<Option<CodeBlockError>>>,
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
    code_block_error: Rc<RefCell<Option<CodeBlockError>>>,
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
      line_start_texts: LineStarts::default(),
      block_end: None,
      dropped_breaks: Default::default(),
      delimiter_runs: None,
      decoration_delimiters: std::cell::RefCell::new(HashMap::new()),
      list_marker: ListItemMarker {
        char: None,
        lines_up: true,
      },
      next_position: NodePosition::default(),
      format_code_block_text: Box::new(format_code_block_text),
      code_block_error,
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

  /// Generates the content of the list item the marker belongs to.
  pub fn mark_in_list_item<T>(&mut self, marker: ListItemMarker, func: impl FnOnce(&mut Context) -> T) -> T {
    let previous = std::mem::replace(&mut self.list_marker, marker);
    let result = func(self);
    self.list_marker = previous;
    result
  }

  /// Marks that a list item's marker was just written out, so that what comes
  /// directly after it can tell it will be sitting beside it.
  pub fn mark_marker_beside(&mut self) {
    self.next_position.marker = Some(self.list_marker);
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

  /// The indentation of what's being generated beyond the block quote it is
  /// within, which is what the list items inside that quote write.
  pub fn indent_within_block_quote(&self) -> u32 {
    self.indent_level - self.block_quote_base_indents.last().copied().unwrap_or(0)
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

  /// Generates the content of a paragraph, which `escapes` says has to be
  /// written with backslashes in order to be read back as one.
  pub fn with_paragraph_escapes<T>(&mut self, escapes: ParagraphEscapes, func: impl FnOnce(&mut Context) -> T) -> T {
    let previous_starts = std::mem::replace(&mut self.escaped_block_starts, escapes.block_starts);
    let previous_line_starts = std::mem::replace(&mut self.line_start_texts, escapes.line_starts);
    let previous_end = self.block_end.replace(escapes.end);
    let result = func(self);
    self.escaped_block_starts = previous_starts;
    self.line_start_texts = previous_line_starts;
    self.block_end = previous_end;
    result
  }

  /// The text from `start` through to the end of the block being written,
  /// which is how far a line of it can run on.
  pub fn text_from(&self, start: usize, fallback_end: usize) -> &str {
    let end = self.block_end.filter(|end| *end >= start).unwrap_or(fallback_end);
    &self.file_text[start..end]
  }

  /// Whether what starts here is written at the start of a line, with nothing
  /// before it on that line.
  pub fn is_line_start(&self, start: usize) -> bool {
    self.line_start_texts.contains(start)
  }

  /// Whether the position sits within the block being written, before the
  /// end of it.
  pub fn is_within_block(&self, position: usize) -> bool {
    self.block_end.is_some_and(|end| position < end)
  }

  /// Generates a run of nodes, `dropped` holding the line breaks within it and
  /// everything nested in it that aren't written out.
  pub fn with_dropped_breaks<T>(&mut self, dropped: DroppedBreaks, func: impl FnOnce(&mut Context) -> T) -> T {
    let previous = std::mem::replace(&mut self.dropped_breaks, dropped);
    let result = func(self);
    self.dropped_breaks = previous;
    result
  }

  /// Generates a run of nodes, `runs` holding the delimiter runs of the block
  /// they belong to -- or nothing, where the nodes are blocks of their own.
  pub fn with_delimiter_runs<T>(&mut self, runs: Option<DelimiterRuns>, func: impl FnOnce(&mut Context) -> T) -> T {
    let previous = std::mem::replace(&mut self.delimiter_runs, runs);
    let result = func(self);
    self.delimiter_runs = previous;
    result
  }

  /// The delimiter runs of the block being written, if its content is what's
  /// being written.
  pub fn delimiter_runs(&self) -> Option<&DelimiterRuns> {
    self.delimiter_runs.as_ref()
  }

  /// Whether the line break written before the node starting here is dropped
  /// rather than written out.
  pub fn drops_break_before(&self, start: usize) -> bool {
    self
      .dropped_breaks
      .before
      .binary_search_by_key(&start, |(at, _)| *at)
      .is_ok()
  }

  /// Whether the line break written after the node ending here is dropped
  /// rather than written out.
  pub fn drops_break_after(&self, end: usize) -> bool {
    self
      .dropped_breaks
      .after
      .binary_search_by_key(&end, |(at, _)| *at)
      .is_ok()
  }

  /// The character written directly before the node starting here, where the
  /// line break between them is dropped.
  pub fn char_written_before(&self, start: usize) -> Option<char> {
    self.written_char(&self.dropped_breaks.before, start)
  }

  /// The character written directly after the node ending here, where the line
  /// break between them is dropped.
  pub fn char_written_after(&self, end: usize) -> Option<char> {
    self.written_char(&self.dropped_breaks.after, end)
  }

  fn written_char(&self, at: &[(usize, char)], position: usize) -> Option<char> {
    // a break that is kept is still written out, so nothing moves against it
    if self.configuration.text_wrap.keeps_line_breaks() || !self.configuration.wrap_unspaced_scripts {
      return None;
    }
    at.binary_search_by_key(&position, |(at, _)| *at)
      .ok()
      .map(|index| at[index].1)
  }

  /// Where a backslash goes in the text starting here, when it is written
  /// where a block would start and has to be escaped.
  pub fn block_start_escape_at(&self, start: usize) -> Option<usize> {
    self
      .escaped_block_starts
      .iter()
      .find(|escape| escape.text_start == start)
      .map(|escape| escape.position)
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

  /// The delimiter the decoration starting at `start` is written with, if that
  /// has been worked out already.
  pub fn resolved_decoration_delimiter(&self, start: usize) -> Option<&'static str> {
    self.decoration_delimiters.borrow().get(&start).copied()
  }

  /// Whether the html is the comment that turns off formatting for the node
  /// written after it.
  pub fn is_ignore_comment(&self, html_text: &str) -> bool {
    is_ignore_comment(html_text, &self.configuration.ignore_directive)
  }

  /// Whether the html is the comment that turns off formatting until the
  /// matching end comment.
  pub fn is_ignore_start_comment(&self, html_text: &str) -> bool {
    is_ignore_comment(html_text, &self.configuration.ignore_start_directive)
  }

  /// Whether the html is the comment that turns formatting back on.
  pub fn is_ignore_end_comment(&self, html_text: &str) -> bool {
    is_ignore_comment(html_text, &self.configuration.ignore_end_directive)
  }

  /// Whether the html holds a comment that turns formatting off.
  ///
  /// A block of html is written out as it was when it holds one, because the
  /// text an ignore comment covers can be within the block rather than being
  /// a node of the document the block sits in.
  pub fn has_ignore_comment(&self, html_text: &str) -> bool {
    [
      &self.configuration.ignore_directive,
      &self.configuration.ignore_start_directive,
      &self.configuration.ignore_end_directive,
      &self.configuration.ignore_file_directive,
    ]
    .iter()
    .any(|directive| html_text.contains(directive.as_str()))
  }

  pub fn is_preserving_decorations(&self) -> bool {
    self.decorations_preserved_count > 0
  }

  pub fn is_text_wrap_disabled(&self) -> bool {
    self.text_wrap_disabled_count > 0
  }

  /// Keeps the error a code block's plugin ran into, when it's the first, so
  /// that it can fail the file once it's been generated.
  pub fn mark_code_block_error(&mut self, pos: usize, error: FormatError) {
    let mut code_block_error = self.code_block_error.borrow_mut();
    if code_block_error.is_none() {
      *code_block_error = Some(CodeBlockError { pos, error });
    }
  }

  pub fn format_text<'b>(&mut self, tag: &str, text: &'b str) -> FormatResult {
    let line_width = std::cmp::max(10, self.configuration.line_width as i32 - self.indent_level as i32) as u32;

    match tag.to_lowercase().as_str() {
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

    count_new_lines(&self.file_text[start..end])
  }
}
