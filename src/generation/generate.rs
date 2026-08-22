use dprint_core::formatting::condition_resolvers;
use dprint_core::formatting::conditions::*;
use dprint_core::formatting::ir_helpers::*;
use dprint_core::formatting::*;
use dprint_core_macros::sc;
use pulldown_cmark::MetadataBlockKind;
use std::borrow::Cow;
use std::rc::Rc;
use unicode_width::UnicodeWidthStr;

use super::common::*;
use super::gen_types::*;
use super::utils;
use crate::configuration::*;

pub fn generate(node: &Node, context: &mut Context) -> PrintItems {
  // eprintln!("Kind: {:?}", node.kind());
  // eprintln!("Text: {:?}", node.text(context));

  match node {
    Node::SourceFile(node) => gen_source_file(node, context),
    Node::Heading(node) => gen_heading(node, context),
    Node::Paragraph(node) => gen_paragraph(node, context),
    Node::BlockQuote(node) => gen_block_quote(node, context),
    Node::CodeBlock(node) => gen_code_block(node, context),
    Node::Code(node) => gen_code(node, context),
    Node::Text(node) => gen_text(node, context),
    Node::TextDecoration(node) => gen_text_decoration(node, context),
    Node::Html(node) => gen_html(node, context),
    Node::DisplayMath(node) => gen_display_math(node, context),
    Node::InlineMath(node) => gen_inline_math(node, context),
    Node::FootnoteReference(node) => gen_footnote_reference(node, context),
    Node::FootnoteDefinition(node) => gen_footnote_definition(node, context),
    Node::InlineLink(node) => gen_inline_link(node, context),
    Node::ReferenceLink(node) => gen_reference_link(node, context),
    Node::ShortcutLink(node) => gen_shortcut_link(node, context),
    Node::AutoLink(node) => gen_auto_link(node, context),
    Node::LinkReference(node) => gen_link_reference(node, context),
    Node::InlineImage(node) => gen_inline_image(node, context),
    Node::ReferenceImage(node) => gen_reference_image(node, context),
    Node::ShortcutImage(node) => gen_shortcut_image(node, context),
    Node::List(node) => gen_list(node, false, context),
    Node::Item(node) => gen_item(node, context),
    Node::TaskListMarker(_) => unreachable!("this should be handled by gen_paragraph"),
    Node::HorizontalRule(node) => gen_horizontal_rule(node, context),
    Node::SoftBreak(_) => PrintItems::new(),
    Node::HardBreak(_) => gen_hard_break(context),
    Node::Table(node) => gen_table(node, context),
    Node::TableHead(_) => unreachable!(),
    Node::TableRow(_) => unreachable!(),
    Node::TableCell(node) => gen_table_cell(node, context),
    Node::MetadataBlock(node) => gen_metadata_block(node, context),
    Node::NotImplemented(_) => ir_helpers::gen_from_raw_string(node.text(context)),
  }
}

fn gen_source_file(source_file: &SourceFile, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();

  items.extend(gen_nodes(&source_file.children, context));

  items.push_condition(if_true(
    "endOfFileNewLine",
    Rc::new(|context| Some(context.writer_info.column_number > 0 || context.writer_info.line_number > 0)),
    Signal::NewLine.into(),
  ));

  items
}

fn gen_nodes(nodes: &[Node], context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  if nodes.is_empty() {
    return items;
  }

  let mut last_node: Option<&Node> = None;
  let mut node_iterator = nodes.iter().filter(|n| !matches!(n, Node::SoftBreak(_)));

  while let Some(mut node) = node_iterator.next() {
    // handle alternate lists
    if let Some(Node::List(last_list)) = &last_node {
      if let Node::List(list) = &node {
        if last_list.start_index.is_some() == list.start_index.is_some() {
          items.extend(get_conditional_blank_line(node.range(), context));
          items.extend(gen_list(list, true, context));
          if let Some(current_node) = node_iterator.next() {
            last_node = Some(node);
            node = current_node;
          } else {
            break;
          }
        }
      }
    }

    // todo: this area needs to be thought out more
    if let Some(last_node) = last_node {
      if matches!(
        node,
        Node::Heading(_)
          | Node::Paragraph(_)
          | Node::CodeBlock(_)
          | Node::FootnoteDefinition(_)
          | Node::HorizontalRule(_)
          | Node::List(_)
          | Node::Table(_)
          | Node::BlockQuote(_)
      ) {
        items.extend(get_conditional_blank_line(node.range(), context));
      } else if !matches!(node, Node::HardBreak(_)) {
        match last_node {
          Node::Heading(_)
          | Node::Paragraph(_)
          | Node::CodeBlock(_)
          | Node::FootnoteDefinition(_)
          | Node::HorizontalRule(_)
          | Node::List(_)
          | Node::Table(_)
          | Node::MetadataBlock(_)
          | Node::BlockQuote(_)
          | Node::DisplayMath(_) => {
            items.extend(get_conditional_blank_line(node.range(), context));
          }
          Node::Code(_)
          | Node::SoftBreak(_)
          | Node::TextDecoration(_)
          | Node::FootnoteReference(_)
          | Node::InlineLink(_)
          | Node::ReferenceLink(_)
          | Node::ShortcutLink(_)
          | Node::AutoLink(_)
          | Node::Text(_)
          | Node::Html(_)
          | Node::InlineImage(_)
          | Node::ReferenceImage(_)
          | Node::ShortcutImage(_)
          | Node::InlineMath(_) => {
            let between_range = (last_node.range().end, node.range().start);
            let new_line_count = context.get_new_lines_in_range(between_range.0, between_range.1);

            if new_line_count == 1 {
              // Callout example:
              // > [!NOTE]
              // > Some note.
              let is_callout = if context.is_in_block_quote() {
                if let Node::Text(text) = last_node {
                  is_callout_text(&text.text)
                } else {
                  false
                }
              } else {
                false
              };
              if is_callout && !context.is_text_wrap_disabled() {
                items.push_signal(Signal::NewLine); // force a newline
              } else if matches!(node, Node::Html(_)) {
                items.push_signal(Signal::NewLine);
              } else {
                items.extend(get_newline_wrapping_based_on_config(context));
              }
            } else if new_line_count > 1 {
              items.push_signal(Signal::NewLine);
              items.push_signal(Signal::NewLine);
            } else {
              let needs_space = if let Node::Html(_) = last_node {
                node.has_preceding_space(context.file_text)
              } else if matches!(last_node, Node::Text(_)) || matches!(node, Node::Text(_)) {
                node.has_preceding_space(context.file_text)
                  || !last_node.ends_with_punctuation(context.file_text)
                    && !node.starts_with_punctuation(context.file_text)
              } else if let Node::FootnoteReference(_) = node {
                false
              } else if let Node::Html(_) = node {
                node.has_preceding_space(context.file_text)
              } else {
                // ex. two images beside each other shouldn't be separated
                node.has_preceding_whitespace(context.file_text)
              };

              if needs_space {
                if node.starts_with_list_word() {
                  items.push_space();
                } else {
                  items.extend(get_space_or_newline_based_on_config(context));
                }
              }
            }
          }
          Node::LinkReference(_) => {
            let needs_newline = matches!(node, Node::LinkReference(_));
            if needs_newline {
              items.push_signal(Signal::NewLine);
            }
          }
          Node::NotImplemented(_)
          | Node::SourceFile(_)
          | Node::Item(_)
          | Node::TaskListMarker(_)
          | Node::HardBreak(_)
          | Node::TableHead(_)
          | Node::TableRow(_)
          | Node::TableCell(_) => {}
        }
      }
    }

    items.extend(generate(node, context));
    last_node = Some(node);

    // check for ignore comment
    if let Node::Html(html) = node {
      let html_text = &context.file_text[html.range.clone()];
      if context.ignore_regex.is_match(html_text) {
        items.push_signal(Signal::NewLine);
        if let Some(node) = node_iterator.next() {
          if context.has_leading_blankline(node.range().start) {
            items.push_signal(Signal::NewLine);
          }

          // include the leading indent
          let range = node.range();
          let text_start = utils::get_leading_non_space_tab_byte_pos(context.file_text, range.start);
          items.extend(ir_helpers::gen_from_raw_string(
            context.file_text[text_start..range.end].trim_end(),
          ));

          last_node = Some(node);
        }
      } else if context.ignore_start_regex.is_match(html_text) {
        let mut range: Option<Range> = None;
        let mut end_comment = None;
        let start = html.range().end;
        for node in node_iterator.by_ref() {
          last_node = Some(node);

          if let Node::Html(html) = node {
            let html_text = &context.file_text[html.range.clone()];
            if context.ignore_end_regex.is_match(html_text) {
              end_comment = Some(html);
              break;
            }
          }

          let node_range = node.range();
          range = Some(Range {
            start: range.map(|r| r.start).unwrap_or(node_range.start),
            end: node_range.end,
          });
        }

        let end = end_comment
          .map(|c| c.range().start)
          .unwrap_or_else(|| last_node.unwrap().range().end);
        let ignore_text = &context.file_text[start..end];
        if let Some(end_comment) = end_comment {
          items.extend(ir_helpers::gen_from_raw_string(ignore_text));
          items.extend(gen_html(end_comment, context));
        } else {
          items.extend(ir_helpers::gen_from_raw_string(ignore_text.trim_end()));
        }
      }
    }
  }

  return items;

  fn get_conditional_blank_line(range: &Range, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    if !context.is_in_list() || context.has_leading_blankline(range.start) {
      items.push_signal(Signal::NewLine);
    }
    items.push_signal(Signal::NewLine);
    items
  }
}

fn gen_heading(heading: &Heading, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();

  if heading.level < 3 && context.configuration.heading_kind == HeadingKind::Setext {
    // setext headings only apply to level 1 and level 2.
    let heading_children = gen_nodes(&heading.children, context);
    let (heading_children, cloned_children) = clone_items(heading_children);
    items.extend(heading_children);
    items.push_item(PrintItem::Signal(Signal::NewLine));

    // render the heading text with the actual line width so wrapping is
    // applied, then measure the longest line for the underline width.
    let underline_width = measure_longest_line_width(cloned_children, context.configuration.line_width);
    let underline_char = if heading.level == 1 { "=" } else { "-" };
    items.push_string(underline_char.repeat(underline_width));
  } else {
    // atx headings apply to all levels.
    items.push_string(format!("{} ", "#".repeat(heading.level as usize)));
    items.extend(with_no_new_lines(gen_nodes(&heading.children, context)));
  }

  items
}

fn gen_paragraph(paragraph: &Paragraph, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();

  if let Some(marker) = &paragraph.marker {
    items.extend(gen_task_list_marker(marker, context));
    if !paragraph.children.is_empty() {
      items.push_space();
    }
  }

  items.extend(gen_task_list_marker_children(
    &paragraph.children,
    paragraph.marker.as_ref(),
    context,
  ));
  items
}

fn gen_block_quote(block_quote: &BlockQuote, context: &mut Context) -> PrintItems {
  context.mark_in_block_quotes(|context, base_indents| {
    let mut items = PrintItems::new();

    // add a > for any string that is on the start of a line
    // Note: This is extremely hacky
    let mut indent_level = 0;
    // the opening `>` cannot rely on being at the start of a line, because a block
    // quote may begin mid-line -- for example directly after a list item marker.
    let mut needs_opening_marker = true;
    let children = gen_nodes(&block_quote.children, context);
    for print_item in get_content_print_items(children, context) {
      match print_item {
        PrintItem::String(text) if needs_opening_marker => {
          // at the beginning of a block quote, '>' is necessary
          // even if it is not at the start of a line i.e. the start of a list item.
          needs_opening_marker = false;
          items.push_optional_path(context.get_memoized_rc_path(MemoizedRcPathKind::FinishIndent(indent_level)));
          items.push_sc(sc!(">"));
          // avoid inserting space in nested block quote markers (`> > foo`).
          if text.text != ">" {
            items.push_space();
          }
          items
            .push_optional_path(context.get_memoized_rc_path(MemoizedRcPathKind::StartWithSingleIndent(indent_level)));
          items.push_item(PrintItem::String(text));
        }
        PrintItem::String(text) => {
          // avoid inserting space in nested block quote markers (`> > foo`).
          let trailing = MarkersTrailing::Text {
            space: text.text != ">",
          };
          items.push_condition(if_true(
            "angleBracketIfStartOfLine",
            condition_resolvers::is_start_of_line(),
            gen_block_quote_markers(&base_indents, indent_level, trailing, context),
          ));
          items.push_item(PrintItem::String(text));
        }
        PrintItem::Signal(Signal::NewLine) => {
          needs_opening_marker = false;
          items.push_condition(if_true(
            "angleBracketIfStartOfLine",
            condition_resolvers::is_start_of_line(),
            gen_block_quote_markers(&base_indents, indent_level, MarkersTrailing::BlankLine, context),
          ));
          items.push_signal(Signal::NewLine);
        }
        PrintItem::Signal(Signal::StartIndent | Signal::QueueStartIndent) => {
          indent_level += 1;
          items.push_item(print_item)
        }
        PrintItem::Signal(Signal::FinishIndent) => {
          indent_level -= 1;
          items.push_item(print_item)
        }
        _ => items.push_item(print_item),
      }
    }

    // an empty block quote generates no print items, so there is nothing above to
    // hang the marker off of. emit it here rather than dropping the block quote.
    if needs_opening_marker {
      items.push_sc(sc!(">"));
    }

    items
  })
}

/// Gets the print items of a block quote's content, stepping into the paths
/// that hold generated content so the markers can be added to the lines within
/// them (ex. the text of a link, which is shared in a path in order to measure
/// it). The paths holding the markers of a nested block quote are left alone,
/// since the indentation they contain isn't the content's.
fn get_content_print_items(items: PrintItems, context: &Context) -> Vec<PrintItem> {
  let mut result = Vec::new();
  let mut iterators = vec![items.iter()];

  while let Some(mut iterator) = iterators.pop() {
    while let Some(print_item) = iterator.next() {
      match print_item {
        PrintItem::RcPath(path) if !context.is_memoized_rc_path(path) => {
          iterators.push(iterator);
          iterators.push(PrintItemsIterator::new(path));
          break;
        }
        _ => result.push(print_item),
      }
    }
  }

  result
}

enum MarkersTrailing {
  /// Text follows the markers on the same line.
  Text { space: bool },
  /// Nothing follows the markers on the line.
  BlankLine,
}

/// Generates the `>` markers that prefix a line within a block quote, keeping the
/// indentation that occurs between the block quote levels (ex. `> - > text`, where
/// a list within the outer block quote indents the inner block quote).
fn gen_block_quote_markers(
  base_indents: &[u32],
  inner_indent_level: u32,
  trailing: MarkersTrailing,
  context: &mut Context,
) -> PrintItems {
  let mut items = PrintItems::new();
  let outermost_indent = *base_indents.first().unwrap();
  let current_indent = *base_indents.last().unwrap() + inner_indent_level;

  // go back to where the outermost marker belongs, then write each
  // marker at the indentation its block quote started at
  items.push_optional_path(
    context.get_memoized_rc_path(MemoizedRcPathKind::FinishIndent(current_indent - outermost_indent)),
  );
  let mut written_indent = outermost_indent;
  for base_indent in base_indents {
    if *base_indent > written_indent {
      items.push_space();
      items.push_optional_path(
        context.get_memoized_rc_path(MemoizedRcPathKind::StartWithSingleIndent(base_indent - written_indent)),
      );
      written_indent = *base_indent;
    }
    items.push_sc(sc!(">"));
  }

  let remaining_indent = current_indent - written_indent;
  match trailing {
    MarkersTrailing::Text { space } => {
      if space {
        items.push_space();
      }
      items
        .push_optional_path(context.get_memoized_rc_path(MemoizedRcPathKind::StartWithSingleIndent(remaining_indent)));
    }
    MarkersTrailing::BlankLine => {
      items.push_optional_path(context.get_memoized_rc_path(MemoizedRcPathKind::StartIndent(remaining_indent)));
    }
  }

  items
}

fn gen_code_block(code_block: &CodeBlock, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  let code_text = get_code_text(code_block, context);
  let code_text = code_text.trim_end();
  let code_text = if context.configuration.unindent_code_blocks {
    utils::unindent(code_text)
  } else {
    Cow::Borrowed(code_text)
  };
  let backtick_text = "`".repeat(get_backtick_count(&code_text));
  let indent_level = if code_block.is_fenced { 0 } else { 4 };

  // header
  if code_block.is_fenced {
    items.push_str(&backtick_text);
    if let Some(tag) = &code_block.tag {
      items.push_str(tag);
    }
    items.push_signal(Signal::NewLine);
  }

  // body
  if !code_text.is_empty() {
    items.extend(ir_helpers::gen_from_string(&code_text));
  }

  // footer
  if code_block.is_fenced {
    if !code_text.is_empty() {
      items.push_signal(Signal::NewLine);
    }
    items.push_string(backtick_text);
  }

  return with_indent_times(items, indent_level);

  fn get_code_text<'a>(code_block: &'a CodeBlock, context: &mut Context) -> Cow<'a, str> {
    let code = &code_block.code;
    if code.trim().is_empty() {
      return Cow::Borrowed("");
    }
    let start_pos = get_code_block_start_pos(code);
    let code = code[start_pos..].trim_end();
    if let Some(tag) = &code_block.tag {
      // allow situations like ```rust,ignore
      let tag = tag.chars().take_while(|&c| c != ' ' && c != ',').collect::<String>();
      if let Ok(Some(text)) = context.format_text(&tag, code) {
        return Cow::Owned(text);
      }
    }
    Cow::Borrowed(code)
  }

  fn get_code_block_start_pos(text: &str) -> usize {
    let mut start_pos = 0;
    for (i, c) in text.char_indices() {
      if c == '\n' {
        start_pos = i + 1;
      } else if !c.is_whitespace() {
        break;
      }
    }
    start_pos
  }

  fn get_backtick_count(text: &str) -> usize {
    // need to count how many consecutive backticks there are in the text
    let mut count = 0;
    let mut max_count = 0;
    for c in text.chars() {
      match c {
        '`' => {
          count += 1;
          max_count = std::cmp::max(count, max_count);
        }
        _ => count = 0,
      }
    }
    std::cmp::max(2, max_count) + 1
  }
}

fn gen_code(code: &Code, context: &mut Context) -> PrintItems {
  let text = code.code.trim();
  let backtick_text = "`".repeat(get_backtick_count(text));
  let separator = if text.starts_with('`') || text.ends_with('`') {
    " "
  } else {
    ""
  };

  // only the text is run through the text builder so that the backticks
  // always stay attached to it when the text is wrapped
  let mut items = PrintItems::new();
  items.push_string(format!("{}{}", backtick_text, separator));
  items.extend(gen_code_str(text, context));
  items.push_string(format!("{}{}", separator, backtick_text));
  return items;

  /// A code span ends at the first run of backticks with the same length as
  /// the one that opened it, so the delimiter must be a length that doesn't
  /// appear in the text.
  fn get_backtick_count(text: &str) -> usize {
    let mut text_counts = Vec::new();
    let mut current_count = 0;
    for c in text.chars() {
      if c == '`' {
        current_count += 1;
      } else {
        if current_count > 0 {
          text_counts.push(current_count);
        }
        current_count = 0;
      }
    }
    if current_count > 0 {
      text_counts.push(current_count);
    }

    let mut count = 1;
    while text_counts.contains(&count) {
      count += 1;
    }
    count
  }
}

/// Generates the text of a code span.
///
/// This can't use `gen_str` because a code span renders its content verbatim
/// apart from line endings, which render as a single space. So a run of a
/// single space or line ending is a place the text may be wrapped, but any
/// longer run of whitespace has to be kept at its original width.
fn gen_code_str(text: &str, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  let mut current_word = String::new();
  let mut was_last_newline = false;
  let mut chars = text.chars().peekable();

  while let Some(c) = chars.next() {
    if c != ' ' && c != '\n' {
      current_word.push(c);
      continue;
    }

    let mut whitespace_count = 1;
    while chars.peek().map(|c| *c == ' ' || *c == '\n').unwrap_or(false) {
      whitespace_count += 1;
      chars.next();
    }

    if whitespace_count == 1 && chars.peek().is_some() {
      push_word(&mut items, &mut current_word, was_last_newline, context);
      was_last_newline = c == '\n' && context.configuration.text_wrap == TextWrap::Maintain;
    } else {
      // keep the width the same because a line ending renders as a space
      current_word.push_str(&" ".repeat(whitespace_count));
    }
  }

  push_word(&mut items, &mut current_word, was_last_newline, context);

  return items;

  fn push_word(items: &mut PrintItems, word: &mut String, was_last_newline: bool, context: &Context) {
    if word.is_empty() {
      return;
    }
    if !items.is_empty() {
      // a chunk may have significant whitespace merged into it, so only the
      // leading token decides whether it could start a block
      let leading_token = word.split(' ').next().unwrap_or(word);
      if utils::is_block_start_word(leading_token) {
        items.push_space();
      } else if was_last_newline {
        items.push_signal(Signal::NewLine);
      } else {
        items.extend(get_space_or_newline_based_on_config(context));
      }
    }
    items.push_string(std::mem::take(word));
  }
}

fn gen_text(text: &Text, context: &mut Context) -> PrintItems {
  gen_str(&text.text, context)
}

fn is_callout_text(text: &str) -> bool {
  // ex. [!NOTE]
  text.starts_with("[!") && text.ends_with("]") && text[2..text.len() - 1].chars().all(|c| c.is_ascii_uppercase())
}

fn gen_str(text: &str, context: &mut Context) -> PrintItems {
  let mut text_builder = TextBuilder::new(context);

  for c in text.chars() {
    text_builder.add_char(c);
  }

  return text_builder.build();

  struct TextBuilder<'a> {
    items: PrintItems,
    was_last_newline: bool,
    current_word: Option<String>,
    context: &'a Context<'a>,
  }

  impl<'a> TextBuilder<'a> {
    pub fn new(context: &'a Context) -> TextBuilder<'a> {
      TextBuilder {
        items: PrintItems::new(),
        was_last_newline: false,
        current_word: None,
        context,
      }
    }

    pub fn build(mut self) -> PrintItems {
      self.flush_current_word();
      self.items
    }

    pub fn add_char(&mut self, character: char) {
      if character == '\n' || character == ' ' {
        if self.context.configuration.text_wrap == TextWrap::Maintain && character == '\n' {
          self.newline();
        } else {
          self.space_or_newline();
        }
        return;
      }

      if let Some(current_word) = self.current_word.as_mut() {
        current_word.push(character);
      } else {
        let mut text = String::new();
        text.push(character);
        self.current_word = Some(text);
      }
    }

    fn space_or_newline(&mut self) {
      self.flush_current_word();
    }

    fn newline(&mut self) {
      self.flush_current_word();
      self.was_last_newline = true;
    }

    fn flush_current_word(&mut self) {
      if let Some(current_word) = self.current_word.take() {
        if !self.items.is_empty() {
          if utils::is_block_start_word(&current_word) {
            self.items.push_space();
          } else if self.was_last_newline {
            self.items.push_signal(Signal::NewLine)
          } else {
            self.items.extend(get_space_or_newline_based_on_config(self.context));
          }
        }

        self.items.push_string(current_word);
        self.was_last_newline = false;
      }
    }
  }
}

fn gen_text_decoration(text: &TextDecoration, context: &mut Context) -> PrintItems {
  /// GitHub doesn't make `_` and `__` as being a text decoration when the character
  /// after the underscore is alphanumeric. For example: `__word__something`. Due
  /// to this, we need to keep the asterisk when configured for underscores
  /// in order to ensure the text keeps its meaning on GitHub.
  fn keep_asterisk(pos: usize, context: &Context) -> bool {
    &context.file_text[pos - 1..pos] == "*"
      && context.file_text[pos..]
        .chars()
        .next()
        .map(|c| c.is_alphanumeric())
        .unwrap_or(false)
  }

  let mut items = PrintItems::new();
  let decoration_text = match &text.kind {
    TextDecorationKind::Emphasis => match context.configuration.emphasis_kind {
      EmphasisKind::Asterisks => sc!("*"),
      EmphasisKind::Underscores => {
        if keep_asterisk(text.range.end, context) {
          sc!("*")
        } else {
          sc!("_")
        }
      }
    },
    TextDecorationKind::Strong => match context.configuration.strong_kind {
      StrongKind::Asterisks => sc!("**"),
      StrongKind::Underscores => {
        if keep_asterisk(text.range.end, context) {
          sc!("**")
        } else {
          sc!("__")
        }
      }
    },
    TextDecorationKind::Strikethrough => sc!("~~"),
  };

  items.push_sc(decoration_text);
  items.extend(gen_nodes(&text.children, context));
  items.push_sc(decoration_text);

  items
}

fn gen_html(node: &Html, ctx: &mut Context) -> PrintItems {
  gen_range(node.range.clone(), ctx)
}

fn gen_display_math(node: &DisplayMath, ctx: &mut Context) -> PrintItems {
  gen_range(node.range.clone(), ctx)
}

fn gen_inline_math(node: &InlineMath, ctx: &mut Context) -> PrintItems {
  gen_range(node.range.clone(), ctx)
}

fn gen_range(range: Range, ctx: &mut Context) -> PrintItems {
  let text = ctx.file_text[range].trim_end();
  if text.is_empty() {
    return PrintItems::new();
  }
  let mut items = PrintItems::new();
  items.push_sc(sc!("")); // force first line indentation
  items.extend(ir_helpers::gen_from_raw_string_trim_line_ends(text));
  items
}

fn gen_footnote_reference(footnote_reference: &FootnoteReference, _: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_string(format!("[^{}]", footnote_reference.name.trim()));
  ir_helpers::with_no_new_lines(items)
}

fn gen_footnote_definition(footnote_definition: &FootnoteDefinition, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_string(format!("[^{}]: ", footnote_definition.name.trim()));
  items.extend(with_indent_times(gen_nodes(&footnote_definition.children, context), 4));
  items
}

fn gen_inline_link(link: &InlineLink, context: &mut Context) -> PrintItems {
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    let generated_children = gen_nodes(&link.children, context);
    items.push_sc(sc!("["));

    // force the text to be on a single line in some scenarios
    let (generated_children, generated_children_clone) = clone_items(generated_children);
    let single_line_text = get_items_text(ir_helpers::with_no_new_lines(generated_children_clone));
    if single_line_text.len() < (context.configuration.line_width / 2) as usize {
      // printing the children back out to text flattens any tab signal
      // they had, so they need breaking up again
      items.extend(gen_text_with_tabs(single_line_text));
    } else {
      items.extend(generated_children);
    }

    items.push_sc(sc!("]"));
    items.push_sc(sc!("("));
    // the parser resolves the escapes in an inline link's url, so render it
    // back out with the escapes it needs and no others
    items.extend(gen_link_destination_text(format_link_destination(link.url.trim())));
    if let Some(title) = &link.title {
      items.extend(gen_title(title, context));
    }
    items.push_sc(sc!(")"));

    ir_helpers::new_line_group(items)
  })
}

/// Writes out a rendered link destination, handling the characters the printer
/// can't be handed as part of a string.
fn gen_link_destination_text(text: String) -> PrintItems {
  // a destination can't contain a line ending in either of its forms, so keep
  // one as the character reference it could only have come from
  let text = if text.contains(['\n', '\r']) {
    text.replace('\r', "&#13;").replace('\n', "&#10;")
  } else {
    text
  };
  gen_text_with_tabs(text)
}

/// Generates an image's alt text, which is the raw text from the file.
fn gen_image_alt_text(text: &str, context: &Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_sc(sc!("!["));
  items.extend(gen_raw_text(text.trim(), context));
  items.push_sc(sc!("]"));
  items
}

/// Generates the label of a reference image or link.
fn gen_reference_label(reference: &str, context: &Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_sc(sc!("["));
  items.extend(gen_raw_text(reference.trim(), context));
  items.push_sc(sc!("]"));
  items
}

/// Generates the title that follows an inline image or link's destination.
///
/// A title continued onto another line loses that line's indentation, since
/// there's no telling apart the indentation that's part of the title from the
/// indentation the enclosing list item or block quote gave it.
fn gen_title(title: &str, context: &Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_sc(sc!(" \""));
  items.extend(gen_raw_text(title.trim(), context));
  items.push_sc(sc!("\""));
  items
}

/// Generates raw text from the file that may span multiple lines.
///
/// The printer requires the strings it's given to be a single line, so the
/// line breaks the text contains need to be sent as print items instead.
fn gen_raw_text(text: &str, context: &Context) -> PrintItems {
  let mut items = PrintItems::new();
  // a line ending may be in either of its forms, so split on both characters
  // and skip the empty text a carriage return and line feed pair leaves behind
  let mut lines = text.split(['\r', '\n']).filter(|line| !line.is_empty());
  if let Some(line) = lines.next() {
    items.extend(gen_text_with_tabs(line.trim_end().to_string()));
  }
  for line in lines {
    items.extend(get_newline_wrapping_based_on_config(context));
    // the printer provides the indentation and block quote markers of a
    // continued line, so drop the ones this picked up from the file
    let line = strip_block_quote_markers(line, context);
    items.extend(gen_text_with_tabs(line.trim_end().to_string()));
  }
  items
}

/// Strips the markers a line of raw text picked up by continuing within a
/// block quote.
fn strip_block_quote_markers<'a>(line: &'a str, context: &Context) -> &'a str {
  let mut line = line.trim_start();
  if context.is_in_block_quote() {
    while let Some(rest) = line.strip_prefix('>') {
      line = rest.trim_start();
    }
  }
  line
}

/// Writes out text, sending any tab it has as a signal, since the printer
/// can't be handed one as part of a string.
fn gen_text_with_tabs(text: String) -> PrintItems {
  let mut items = PrintItems::new();
  if !text.contains('\t') {
    items.push_string(text);
    return items;
  }

  for (i, part) in text.split('\t').enumerate() {
    if i > 0 {
      items.push_signal(Signal::Tab);
    }
    if !part.is_empty() {
      items.push_str(part);
    }
  }
  items
}

/// Renders an unescaped link destination, enclosing it in pointy brackets and
/// escaping characters only where necessary for it to round trip.
fn format_link_destination(destination: &str) -> String {
  let escaped = escape_link_destination(destination);
  if link_destination_needs_pointy_brackets(destination) {
    format!("<{}>", escaped.replace('<', r"\<").replace('>', r"\>"))
  } else {
    escaped
  }
}

/// Returns `true` if the destination can't be written without pointy brackets,
/// which is when it starts with `<`, contains a space or ascii control
/// character, or has unbalanced parentheses.
fn link_destination_needs_pointy_brackets(destination: &str) -> bool {
  if destination.starts_with('<') {
    return true;
  }

  let mut parentheses_depth = 0;
  for c in destination.chars() {
    match c {
      c if c == ' ' || c.is_ascii_control() => return true,
      '(' => parentheses_depth += 1,
      ')' => parentheses_depth -= 1,
      _ => (),
    }
    if parentheses_depth < 0 {
      return true;
    }
  }
  parentheses_depth != 0
}

/// Escapes the characters that would otherwise take on a different meaning
/// when the destination gets parsed again.
fn escape_link_destination(destination: &str) -> String {
  let mut text = String::with_capacity(destination.len());
  for (index, c) in destination.char_indices() {
    let rest = &destination[index + c.len_utf8()..];
    match c {
      // a trailing backslash would escape the delimiter written after the
      // destination, so it needs escaping too
      '\\' if rest.chars().next().is_none_or(|c| c.is_ascii_punctuation()) => text.push('\\'),
      '&' if starts_character_reference(rest) => text.push('\\'),
      _ => (),
    }
    text.push(c);
  }
  text
}

/// Returns `true` if the text following an ampersand makes it a character
/// reference (ex. `amp;`, `#41;` or `#x29;`).
fn starts_character_reference(text: &str) -> bool {
  let Some((body, _)) = text.split_once(';') else {
    return false;
  };
  match body.strip_prefix('#') {
    Some(number) => match number.strip_prefix(['x', 'X']) {
      Some(number) => !number.is_empty() && number.chars().all(|c| c.is_ascii_hexdigit()),
      None => !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()),
    },
    None => !body.is_empty() && body.chars().all(|c| c.is_ascii_alphanumeric()),
  }
}

fn gen_reference_link(link: &ReferenceLink, context: &mut Context) -> PrintItems {
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    items.push_sc(sc!("["));
    items.extend(gen_nodes(&link.children, context));
    items.push_sc(sc!("]"));
    items.extend(gen_reference_label(&link.reference, context));
    ir_helpers::new_line_group(items)
  })
}

fn gen_shortcut_link(link: &ShortcutLink, context: &mut Context) -> PrintItems {
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    items.push_sc(sc!("["));
    items.extend(gen_nodes(&link.children, context));
    items.push_sc(sc!("]"));
    ir_helpers::new_line_group(items)
  })
}

fn gen_auto_link(link: &AutoLink, context: &mut Context) -> PrintItems {
  // auto-links can't contain spaces, but do this anyway just in case
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    items.push_sc(sc!("<"));
    items.extend(gen_nodes(&link.children, context));
    items.push_sc(sc!(">"));
    ir_helpers::new_line_group(items)
  })
}

fn gen_link_reference(link_ref: &LinkReference, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.extend(gen_reference_label(&link_ref.name, context));
  items.push_sc(sc!(": "));

  let url = format_raw_link_destination(link_ref.link.trim());
  if url.is_empty() {
    // unlike an inline link, a link reference definition can't have an
    // empty destination without these
    items.push_sc(sc!("<>"));
  } else {
    items.extend(gen_link_destination_text(url));
  }

  if let Some(title) = &link_ref.title {
    items.extend(gen_title(title, context));
  }
  ir_helpers::new_line_group(items)
}

/// Renders a link destination that's still in the raw form it has in the file.
///
/// The escapes the author wrote are kept as they are, because resolving them
/// would also resolve any character reference, and there's no way to tell a
/// resolved one apart from text that merely looks like one.
fn format_raw_link_destination(destination: &str) -> String {
  let destination = destination
    .strip_prefix('<')
    .and_then(|destination| destination.strip_suffix('>'))
    .unwrap_or(destination);
  let needs_pointy_brackets = link_destination_needs_pointy_brackets(&unescape_link_destination(destination));
  let mut text = String::with_capacity(destination.len() + 2);
  if needs_pointy_brackets {
    text.push('<');
  }
  let mut chars = destination.chars();
  while let Some(c) = chars.next() {
    match c {
      // pointy brackets only need escaping when they're within pointy brackets
      '<' | '>' if needs_pointy_brackets => {
        text.push('\\');
        text.push(c);
      }
      '\\' => match chars.next() {
        // parentheses are escaped for the sake of the surrounding form, which
        // has been decided here, so drop the escapes and let the form decide
        Some(escaped @ ('(' | ')')) => text.push(escaped),
        Some(escaped) => {
          text.push('\\');
          text.push(escaped);
        }
        // a trailing backslash would escape whatever comes after the destination
        None => text.push_str(r"\\"),
      },
      _ => text.push(c),
    }
  }
  if needs_pointy_brackets {
    text.push('>');
  }
  text
}

/// Resolves the backslash escapes in a raw link destination.
///
/// This is only good enough to decide how the destination has to be written,
/// since it leaves any character reference alone.
fn unescape_link_destination(destination: &str) -> String {
  let mut text = String::with_capacity(destination.len());
  let mut chars = destination.chars().peekable();
  while let Some(c) = chars.next() {
    match chars.peek() {
      Some(next) if c == '\\' && next.is_ascii_punctuation() => {
        text.push(*next);
        chars.next();
      }
      _ => text.push(c),
    }
  }
  text
}

fn gen_inline_image(image: &InlineImage, context: &mut Context) -> PrintItems {
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    items.extend(gen_image_alt_text(&image.text, context));
    items.push_sc(sc!("("));
    // like a link reference definition, this is the raw text from the file
    items.extend(gen_link_destination_text(format_raw_link_destination(image.url.trim())));
    if let Some(title) = &image.title {
      items.extend(gen_title(title, context));
    }
    items.push_sc(sc!(")"));
    ir_helpers::new_line_group(items)
  })
}

fn gen_reference_image(image: &ReferenceImage, context: &mut Context) -> PrintItems {
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    items.extend(gen_image_alt_text(&image.text, context));
    items.extend(gen_reference_label(&image.reference, context));
    ir_helpers::new_line_group(items)
  })
}

fn gen_shortcut_image(image: &ShortcutImage, context: &mut Context) -> PrintItems {
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    items.extend(gen_image_alt_text(&image.text, context));
    ir_helpers::new_line_group(items)
  })
}

fn gen_list(list: &List, is_alternate: bool, context: &mut Context) -> PrintItems {
  context.mark_in_list(|context| {
    let mut items = PrintItems::new();

    // generate items
    for (index, child) in list.children.iter().enumerate() {
      if index > 0 {
        items.push_signal(Signal::NewLine);
        if context.has_leading_blankline(child.range().start) {
          items.push_signal(Signal::NewLine);
        }
      }
      let prefix_text = if let Some(start_index) = list.start_index {
        let end_char = if is_alternate { ")" } else { "." };
        let display_index = if is_all_ones_list(list, context) {
          1
        } else {
          start_index + index as u64
        };
        format!("{}{}", display_index, end_char)
      } else {
        String::from(context.configuration.unordered_list_kind.list_char(is_alternate))
      };
      let indent_increment = match context.configuration.list_indent_kind {
        crate::configuration::ListIndentKind::CommonMark => (prefix_text.chars().count() + 1) as u32,
        crate::configuration::ListIndentKind::PythonMarkdown => {
          std::cmp::max(prefix_text.chars().count() as u32 + 1, 4)
        }
      };
      context.indent_level += indent_increment;
      items.push_string(prefix_text);
      let after_child = LineAndColumn::new("afterChild");
      items.push_condition(if_true(
        "spaceIfHasChild",
        Rc::new(move |context| Some(!condition_helpers::is_at_same_position(context, after_child)?)),
        Signal::SpaceIfNotTrailing.into(),
      ));
      items.extend(with_indent_times(generate(child, context), indent_increment));
      items.push_line_and_column(after_child);
      context.indent_level -= indent_increment;
    }

    items
  })
}

fn gen_item(item: &Item, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();

  if let Some(marker) = &item.marker {
    items.extend(gen_task_list_marker(marker, context));
    if !item.children.is_empty() {
      items.push_space();
    }
  }

  items.extend(gen_task_list_marker_children(
    &item.children,
    item.marker.as_ref(),
    context,
  ));

  if !item.sub_lists.is_empty() {
    items.push_signal(Signal::NewLine);
    if context.has_leading_blankline(item.sub_lists.first().unwrap().range().start) {
      items.push_signal(Signal::NewLine);
    }
    items.extend(gen_nodes(&item.sub_lists, context));
  }

  items
}

fn gen_task_list_marker_children(
  children: &[Node],
  marker: Option<&TaskListMarker>,
  context: &mut Context,
) -> PrintItems {
  let mut items = PrintItems::new();
  // indent the children to beyond the task list marker
  let marker_indent = if marker.is_some() { 4 } else { 0 };
  context.raw_indent_level += marker_indent;
  let indent_child_index_end = children
    .iter()
    .position(|c| {
      matches!(
        c,
        Node::List(_) | Node::CodeBlock(_) | Node::BlockQuote(_) | Node::Heading(_) | Node::Table(_)
      ) || context.has_leading_blankline(c.range().start)
    })
    .unwrap_or(children.len());
  items.extend(with_indent_times(
    gen_nodes(&children[..indent_child_index_end], context),
    marker_indent,
  ));
  context.raw_indent_level -= marker_indent;

  // insert the remaining children without indent
  if indent_child_index_end > 0 && indent_child_index_end != children.len() {
    items.push_signal(Signal::NewLine);
    if context.has_leading_blankline(children[indent_child_index_end].range().start) {
      items.push_signal(Signal::NewLine);
    }
  }
  items.extend(gen_nodes(&children[indent_child_index_end..], context));
  items
}

fn gen_task_list_marker(marker: &TaskListMarker, _: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  if marker.is_checked {
    items.push_sc(sc!("[x]"));
  } else {
    items.push_sc(sc!("[ ]"));
  }

  items
}

fn gen_horizontal_rule(_: &HorizontalRule, _: &mut Context) -> PrintItems {
  "---".into()
}

fn gen_hard_break(_: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_sc(sc!("\\"));
  items.push_signal(Signal::NewLine);
  items
}

fn gen_table(table: &Table, context: &mut Context) -> PrintItems {
  let header = table
    .header
    .cells
    .iter()
    .map(|cell| get_cell_items_and_width(cell, context))
    .collect::<Vec<_>>();
  let rows = table
    .rows
    .iter()
    .map(|row| {
      row
        .cells
        .iter()
        .map(|cell| get_cell_items_and_width(cell, context))
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();
  let column_widths = get_column_widths(&header, &rows, &table.column_alignment);
  let mut items = PrintItems::new();

  items.extend(get_row_items(header, &column_widths, &table.column_alignment));
  items.push_signal(Signal::NewLine);
  items.extend(get_divider_row(&column_widths, &table.column_alignment));

  for row in rows {
    items.push_signal(Signal::NewLine);
    items.extend(get_row_items(row, &column_widths, &table.column_alignment));
  }

  return items;

  fn get_divider_row(column_widths: &[usize], column_alignments: &[ColumnAlignment]) -> PrintItems {
    let mut items = PrintItems::new();
    for (i, column_width) in column_widths.iter().enumerate() {
      let column_alignment = column_alignments.get(i).copied().unwrap_or(ColumnAlignment::None);
      if i == 0 {
        items.push_sc(sc!("| "));
      } else {
        items.push_space();
      }

      let column_alignment_props = get_column_alignment_properties(column_alignment);
      let dashes_count = column_width - column_alignment_props.count();

      if column_alignment_props.has_left_colon {
        items.push_sc(sc!(":"));
      }
      items.push_string("-".repeat(dashes_count));
      if column_alignment_props.has_right_colon {
        items.push_sc(sc!(":"));
      }

      items.push_sc(sc!(" |"));
    }

    ir_helpers::with_no_new_lines(items)
  }

  fn get_row_items(
    row_cells: Vec<(PrintItems, usize)>,
    column_widths: &[usize],
    column_alignments: &[ColumnAlignment],
  ) -> PrintItems {
    let mut items = PrintItems::new();
    for (i, (cell_items, cell_width)) in row_cells.into_iter().enumerate() {
      let column_alignment = column_alignments.get(i).copied().unwrap_or(ColumnAlignment::None);
      let column_max_width = *column_widths.get(i).unwrap();
      let difference = column_max_width - cell_width;
      if i == 0 {
        items.push_sc(sc!("| "))
      } else {
        items.push_space();
      }

      if difference > 0 {
        match column_alignment {
          ColumnAlignment::None | ColumnAlignment::Left => {}
          ColumnAlignment::Center => {
            if difference > 1 {
              items.push_string(" ".repeat((difference as f32 / 2_f32).floor() as usize))
            }
          }
          ColumnAlignment::Right => {
            items.push_string(" ".repeat(difference));
          }
        }
      }

      items.extend(cell_items);

      if difference > 0 {
        match column_alignment {
          ColumnAlignment::None | ColumnAlignment::Left => {
            items.push_string(" ".repeat(difference));
          }
          ColumnAlignment::Center => items.push_string(" ".repeat((difference as f32 / 2_f32).ceil() as usize)),
          ColumnAlignment::Right => {}
        }
      }

      items.push_sc(sc!(" |"));
    }

    ir_helpers::with_no_new_lines(items)
  }

  fn get_column_widths(
    header: &[(PrintItems, usize)],
    rows: &[Vec<(PrintItems, usize)>],
    column_alignments: &[ColumnAlignment],
  ) -> Vec<usize> {
    let mut column_widths = Vec::new();
    for i in 0.. {
      let mut had_column = false;
      let mut max_width = 0;

      // get header width
      if let Some((_, width)) = header.get(i) {
        max_width = *width;
        had_column = true;
      }

      // check column alignment row width
      if let Some(column_alignment) = column_alignments.get(i) {
        // + 1 in order to have at least one dash
        max_width = std::cmp::max(
          max_width,
          get_column_alignment_properties(*column_alignment).count() + 1,
        );
        had_column = true;
      }

      // check each row width
      for row in rows.iter() {
        if let Some((_, width)) = row.get(i) {
          max_width = std::cmp::max(max_width, *width);
          had_column = true;
        }
      }

      if had_column {
        column_widths.push(max_width);
      } else {
        break;
      }
    }
    column_widths
  }

  struct ColumnAlignmentProperties {
    has_left_colon: bool,
    has_right_colon: bool,
  }

  impl ColumnAlignmentProperties {
    pub fn count(&self) -> usize {
      (if self.has_left_colon { 1 } else { 0 }) + (if self.has_right_colon { 1 } else { 0 })
    }
  }

  fn get_column_alignment_properties(column_alignment: ColumnAlignment) -> ColumnAlignmentProperties {
    let has_left_colon = column_alignment == ColumnAlignment::Left || column_alignment == ColumnAlignment::Center;
    let has_right_colon = column_alignment == ColumnAlignment::Right || column_alignment == ColumnAlignment::Center;
    ColumnAlignmentProperties {
      has_left_colon,
      has_right_colon,
    }
  }

  fn get_cell_items_and_width(cell: &TableCell, context: &mut Context) -> (PrintItems, usize) {
    let items = gen_table_cell(cell, context);
    get_items_single_line_width(items)
  }
}

fn gen_table_cell(table_cell: &TableCell, context: &mut Context) -> PrintItems {
  gen_nodes(&table_cell.children, context)
}

fn gen_metadata_block(node: &MetadataBlock, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();

  let delimiter = match node.kind {
    MetadataBlockKind::YamlStyle => sc!("---"),
    MetadataBlockKind::PlusesStyle => sc!("+++"),
  };

  items.push_sc(delimiter);
  items.push_signal(Signal::NewLine);
  match node.kind {
    MetadataBlockKind::YamlStyle => {
      let text = context
        .format_text("yaml", &node.text)
        .ok()
        .flatten()
        .map(Cow::from)
        .unwrap_or_else(|| Cow::from(&node.text));
      items.extend(ir_helpers::gen_from_string_trim_line_ends(text.trim_end()));
    }
    MetadataBlockKind::PlusesStyle => {
      items.extend(ir_helpers::gen_from_raw_string_trim_line_ends(node.text.trim_end()));
    }
  }
  items.push_signal(Signal::NewLine);
  items.push_sc(delimiter);

  items
}

fn get_items_single_line_width(items: PrintItems) -> (PrintItems, usize) {
  let (items, cloned_items) = clone_items(items);
  let width = measure_single_line_width(cloned_items);
  (items, width)
}

fn clone_items(items: PrintItems) -> (PrintItems, PrintItems) {
  // todo: something in the core library? This is weird
  let rc_path = items.into_rc_path();
  let mut items1 = PrintItems::new();
  let mut items2 = PrintItems::new();
  items1.push_optional_path(rc_path);
  items2.push_optional_path(rc_path);
  (items1, items2)
}

fn measure_single_line_width(items: PrintItems) -> usize {
  UnicodeWidthStr::width(get_items_text(items).as_str())
}

fn get_items_text(items: PrintItems) -> String {
  print(
    ir_helpers::with_no_new_lines(items),
    PrintOptions {
      indent_width: 0,
      max_width: u32::MAX,
      use_tabs: false,
      new_line_text: "",
    },
  )
}

fn measure_longest_line_width(items: PrintItems, max_width: u32) -> usize {
  let rendered = print(
    items,
    PrintOptions {
      indent_width: 0,
      max_width,
      use_tabs: false,
      new_line_text: "\n",
    },
  );
  rendered.lines().map(UnicodeWidthStr::width).max().unwrap_or(0)
}

fn get_space_or_newline_based_on_config(context: &Context) -> PrintItems {
  if context.is_text_wrap_disabled() {
    return space();
  }
  match context.configuration.text_wrap {
    TextWrap::Always => Signal::SpaceOrNewLine.into(),
    TextWrap::Never | TextWrap::Maintain => space(),
  }
}

fn space() -> PrintItems {
  let mut items = PrintItems::new();
  items.push_space();
  items
}

fn get_newline_wrapping_based_on_config(context: &Context) -> PrintItems {
  match context.configuration.text_wrap {
    TextWrap::Always => Signal::SpaceOrNewLine.into(),
    TextWrap::Never => space(),
    TextWrap::Maintain => {
      if context.is_text_wrap_disabled() {
        if_true_or(
          "newLineOrSpaceIfNewlinesDisabled",
          condition_resolvers::is_forcing_no_newlines(),
          space(),
          Signal::NewLine.into(),
        )
        .into()
      } else {
        Signal::NewLine.into()
      }
    }
  }
}

/// If the list's first items are both 1s
fn is_all_ones_list(list: &List, context: &Context) -> bool {
  list.children.len() > 1 && list.start_index.unwrap_or(0) == 1 && {
    let text = list.children.get(1).unwrap().text(context).trim();
    text.starts_with("1.") || text.starts_with("1)")
  }
}

#[cfg(test)]
mod tests {
  use super::format_link_destination;
  use super::format_raw_link_destination;
  use super::unescape_link_destination;

  #[test]
  fn formats_link_destination_without_pointy_brackets_when_possible() {
    assert_eq!(format_link_destination(""), "");
    assert_eq!(format_link_destination("foo%20bar"), "foo%20bar");
    assert_eq!(format_link_destination("foo(bar)baz"), "foo(bar)baz");
    assert_eq!(format_link_destination("a>b"), "a>b");
  }

  #[test]
  fn formats_link_destination_with_pointy_brackets_when_necessary() {
    assert_eq!(format_link_destination("foo bar"), "<foo bar>");
    assert_eq!(format_link_destination("foo(bar"), "<foo(bar>");
    assert_eq!(format_link_destination("foo)(bar"), "<foo)(bar>");
    // can't start with a pointy bracket without them
    assert_eq!(format_link_destination("<foo"), r"<\<foo>");
    // ascii control characters aren't allowed without them
    assert_eq!(format_link_destination("foo\u{1}bar"), "<foo\u{1}bar>");
  }

  #[test]
  fn escapes_link_destination_characters() {
    // pointy brackets need escaping within pointy brackets
    assert_eq!(format_link_destination("a> b"), r"<a\> b>");
    assert_eq!(format_link_destination("a< b"), r"<a\< b>");
    // a backslash needs escaping when it would escape what follows it
    assert_eq!(format_link_destination(r"foo\_bar"), r"foo\\_bar");
    assert_eq!(format_link_destination(r"foo\bar"), r"foo\bar");
    assert_eq!(format_link_destination(r"foo\<bar baz"), r"<foo\\\<bar baz>");
    // ...including the delimiter that follows the destination
    assert_eq!(format_link_destination(r"foo\"), r"foo\\");
    assert_eq!(format_link_destination("foo bar\\"), r"<foo bar\\>");
  }

  #[test]
  fn escapes_ampersands_that_would_become_character_references() {
    assert_eq!(format_link_destination("&amp;"), r"\&amp;");
    assert_eq!(format_link_destination("&#41;"), r"\&#41;");
    assert_eq!(format_link_destination("&#x29;"), r"\&#x29;");
    // these can't be mistaken for a character reference
    assert_eq!(format_link_destination("x?a=1&b=2"), "x?a=1&b=2");
    assert_eq!(format_link_destination("x?a=1&b=2;c=3"), "x?a=1&b=2;c=3");
    assert_eq!(format_link_destination("&;"), "&;");
    assert_eq!(format_link_destination("&#;"), "&#;");
  }

  #[test]
  fn unescapes_link_destination() {
    assert_eq!(unescape_link_destination(r"foo\(bar"), "foo(bar");
    assert_eq!(unescape_link_destination(r"foo\\_bar"), r"foo\_bar");
    assert_eq!(unescape_link_destination("foo\\"), "foo\\");
    // only ascii punctuation is escapable
    assert_eq!(unescape_link_destination(r"foo\bar"), r"foo\bar");
    // a character reference is left alone, since it isn't an escape
    assert_eq!(unescape_link_destination("&amp;"), "&amp;");
  }

  #[test]
  fn formats_raw_link_destination_keeping_its_escapes() {
    // the caller decides what an empty destination gets written as
    assert_eq!(format_raw_link_destination("<>"), "");
    assert_eq!(format_raw_link_destination(""), "");
    assert_eq!(format_raw_link_destination("<foo bar>"), "<foo bar>");
    assert_eq!(format_raw_link_destination(r"foo\_bar"), r"foo\_bar");
    assert_eq!(format_raw_link_destination("x?a=1&amp;b=2"), "x?a=1&amp;b=2");
    assert_eq!(format_raw_link_destination(r"x?a=1\&amp;b=2"), r"x?a=1\&amp;b=2");
    // the parentheses escapes follow the form that gets written here
    assert_eq!(format_raw_link_destination(r"foo\(bar"), "<foo(bar>");
    assert_eq!(format_raw_link_destination(r"a\(b)c"), "a(b)c");
    assert_eq!(format_raw_link_destination(r"foo\(bar\)baz"), "foo(bar)baz");
    assert_eq!(format_raw_link_destination(r"<foo\\(bar>"), r"<foo\\(bar>");
    // ...but pointy brackets do
    assert_eq!(format_raw_link_destination(r"a>b\(c"), r"<a\>b(c>");
    assert_eq!(format_raw_link_destination(r"<a\> b>"), r"<a\> b>");
    // a trailing backslash would escape the closing pointy bracket
    assert_eq!(format_raw_link_destination("<foo bar\\\\>"), r"<foo bar\\>");
    assert_eq!(format_raw_link_destination("foo bar\\"), r"<foo bar\\>");
  }
}
