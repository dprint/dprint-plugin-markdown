use dprint_core::formatting::condition_resolvers;
use dprint_core::formatting::conditions::*;
use dprint_core::formatting::ir_helpers::*;
use dprint_core::formatting::*;
use dprint_core_macros::sc;
use std::borrow::Cow;
use std::rc::Rc;
use unicode_width::UnicodeWidthStr;

use super::common::*;
use super::gen_types::*;
use super::utils;
use crate::configuration::*;

pub fn generate(node: &Node, context: &mut Context) -> PrintItems {
  let position = context.take_position();
  match node {
    Node::SourceFile(node) => gen_source_file(node, context),
    Node::Heading(node) => gen_heading(node, context),
    Node::Paragraph(node) => gen_paragraph(node, context),
    Node::BlockQuote(node) => gen_block_quote(node, context),
    Node::CodeBlock(node) => gen_code_block(node, position, context),
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
    Node::DefinitionList(node) => gen_definition_list(node, context),
    Node::DefinitionListTitle(_) => unreachable!("this should be handled by gen_definition_list"),
    Node::DefinitionListDefinition(_) => unreachable!("this should be handled by gen_definition_list"),
    Node::TaskListMarker(_) => unreachable!("this should be handled by gen_paragraph"),
    Node::HorizontalRule(node) => gen_horizontal_rule(node, position),
    Node::SoftBreak(_) => PrintItems::new(),
    Node::HardBreak(_) => gen_hard_break(context),
    Node::Table(node) => gen_table(node, context),
    Node::TableHead(_) => unreachable!(),
    Node::TableRow(_) => unreachable!(),
    Node::TableCell(node) => gen_table_cell(node, context),
    Node::MetadataBlock(node) => gen_metadata_block(node, context),
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
          items.extend(get_conditional_blank_line(node.span(), context));
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
          | Node::DefinitionList(_)
          | Node::Table(_)
          | Node::BlockQuote(_)
      ) {
        items.extend(get_conditional_blank_line(node.span(), context));
      } else if !matches!(node, Node::HardBreak(_)) {
        match last_node {
          Node::Heading(_)
          | Node::Paragraph(_)
          | Node::CodeBlock(_)
          | Node::FootnoteDefinition(_)
          | Node::HorizontalRule(_)
          | Node::List(_)
          | Node::DefinitionList(_)
          | Node::Table(_)
          | Node::MetadataBlock(_)
          | Node::BlockQuote(_)
          | Node::DisplayMath(_) => {
            items.extend(get_conditional_blank_line(node.span(), context));
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
            let between_range = (last_node.span().end, node.span().start);
            let new_line_count = context.get_new_lines_in_range(between_range.0, between_range.1);

            if new_line_count == 1 {
              // Callout example:
              // > [!NOTE]
              // > Some note.
              if is_callout_node(last_node, context) && !context.is_text_wrap_disabled() {
                items.push_signal(Signal::NewLine); // force a newline
              } else if node.starts_block_at_line_start(text_can_be_wrapped_away(context)) {
                // text that would start a block can't be moved to the start of
                // a line without changing what it means
                items.push_space();
              } else if matches!(node, Node::Html(_)) {
                items.push_signal(Signal::NewLine);
              } else if last_node.ends_with_unspaced_script(context.file_text)
                && node.starts_with_unspaced_script(context.file_text)
              {
                items.extend(get_unspaced_script_newline_wrapping(context));
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
                if node.starts_block_at_line_start(text_can_be_wrapped_away(context)) || node.starts_with_list_word() {
                  // the node would start a block of its own at the start of a
                  // line, so it has to be kept off one
                  items.push_space();
                } else {
                  items.extend(get_space_or_newline_based_on_config(context));
                }
              }
            }
          }
          Node::LinkReference(_) => {
            if matches!(node, Node::LinkReference(_)) {
              // definitions are kept on their own lines, with whatever blank
              // line separated them in the file
              if context.get_new_lines_in_range(last_node.span().end, node.span().start) > 1 {
                items.push_signal(Signal::NewLine);
              }
              items.push_signal(Signal::NewLine);
            } else {
              items.extend(get_conditional_blank_line(node.span(), context));
            }
          }
          Node::SourceFile(_)
          | Node::Item(_)
          | Node::DefinitionListTitle(_)
          | Node::DefinitionListDefinition(_)
          | Node::TaskListMarker(_)
          | Node::HardBreak(_)
          | Node::TableHead(_)
          | Node::TableRow(_)
          | Node::TableCell(_) => {}
        }
      }
    }

    // a hard break after a callout header (ex. `> [!NOTE]` with trailing
    // spaces) would stop it from being recognized as a callout, so only newline
    if matches!(node, Node::HardBreak(_))
      && !context.is_text_wrap_disabled()
      && last_node.map(|n| is_callout_node(n, context)).unwrap_or(false)
    {
      items.push_signal(Signal::NewLine);
      last_node = Some(node);
      continue;
    }

    // a list takes the indentation of whatever follows it for its own content,
    // so what comes next has to be written without any
    if matches!(last_node, Some(Node::List(_) | Node::DefinitionList(_))) {
      context.mark_after_list();
    }
    // a line of dashes directly below a paragraph underlines it into a heading.
    // everywhere but within a list a blank line is written between the two,
    // which leaves nothing for the dashes to underline
    if matches!(last_node, Some(Node::Paragraph(_)))
      && context.is_in_list()
      && !context.has_leading_blankline(node.span().start)
    {
      context.mark_after_paragraph();
    }
    items.extend(generate(node, context));
    last_node = Some(node);

    // check for ignore comment
    if let Node::Html(html) = node {
      let html_text = html.span.text(context.file_text);
      if context.ignore_regex.is_match(html_text) {
        items.push_signal(Signal::NewLine);
        if let Some(node) = node_iterator.next() {
          if context.has_leading_blankline(node.span().start) {
            items.push_signal(Signal::NewLine);
          }

          // include the leading indent
          let node_span = node.span();
          let text_start = utils::get_leading_non_space_tab_byte_pos(context.file_text, node_span.start);
          items.extend(ir_helpers::gen_from_raw_string(
            context.file_text[text_start..node_span.end].trim_end_matches(WHITESPACE),
          ));

          last_node = Some(node);
        }
      } else if context.ignore_start_regex.is_match(html_text) {
        let mut end_comment = None;
        let start = html.span().end;
        for node in node_iterator.by_ref() {
          last_node = Some(node);

          if let Node::Html(html) = node {
            let html_text = html.span.text(context.file_text);
            if context.ignore_end_regex.is_match(html_text) {
              end_comment = Some(html);
              break;
            }
          }
        }

        let end = end_comment
          .map(|c| c.span().start)
          .unwrap_or_else(|| last_node.unwrap().span().end);
        let ignore_text = &context.file_text[start..end];
        if let Some(end_comment) = end_comment {
          items.extend(ir_helpers::gen_from_raw_string(ignore_text));
          items.extend(gen_html(end_comment, context));
        } else {
          items.extend(ir_helpers::gen_from_raw_string(
            ignore_text.trim_end_matches(WHITESPACE),
          ));
        }
      }
    }
  }

  return items;

  fn get_conditional_blank_line(span: Span, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    if !context.is_in_list() || context.has_leading_blankline(span.start) {
      items.push_signal(Signal::NewLine);
    }
    items.push_signal(Signal::NewLine);
    items
  }
}

fn gen_heading(heading: &Heading, context: &mut Context) -> PrintItems {
  // setext headings only apply to level 1 and level 2, and only where the text
  // they underline reads as the paragraph a heading is made of -- an html
  // comment at the start of a line is a block of its own, which nothing can
  // underline
  if heading.level < 3
    && context.configuration.heading_kind == HeadingKind::Setext
    && !heading
      .children
      .first()
      .is_some_and(|child| child.starts_block_in_paragraph())
  {
    let children = gen_nodes(&heading.children, context);
    let (children, cloned_children) = clone_items(children);

    // render the heading text with the actual line width so wrapping is
    // applied, then measure the longest line for the underline width.
    let underline_width = measure_longest_line_width(cloned_children, context.configuration.line_width);

    // an underline with no text above it isn't a heading at all, so an
    // empty heading can only be written as atx
    if underline_width > 0 {
      let mut items = PrintItems::new();
      items.extend(children);
      items.push_signal(Signal::NewLine);
      let underline_char = if heading.level == 1 { "=" } else { "-" };
      items.push_string(underline_char.repeat(underline_width));
      return items;
    }

    return gen_atx_heading(heading.level, children);
  }

  // atx headings apply to all levels.
  let escaped = closing_hashes_start(heading);
  let children = context.with_escaped_closing_hashes(escaped, |context| gen_nodes(&heading.children, context));
  return gen_atx_heading(heading.level, children);

  /// Where the text an atx heading ends with starts, when it ends with hashes
  /// that would be read as the sequence that closes the heading rather than as
  /// the text they are.
  ///
  /// A closing sequence is a run of hashes at the end of the line with only
  /// spaces after it, and either nothing or a space before it.
  fn closing_hashes_start(heading: &Heading) -> Option<usize> {
    let Node::Text(text) = heading.children.last()? else {
      return None;
    };
    let before = text.text.trim_end_matches('#');
    if before.len() == text.text.len() {
      return None; // it doesn't end with hashes at all
    }
    // hashes written against the text before them are read as part of it,
    // unless there is no text before them for them to be part of
    let is_closing = before.is_empty() || before.ends_with([' ', '\t']);
    is_closing.then_some(text.span.start)
  }
}

fn gen_atx_heading(level: u8, children: PrintItems) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_string("#".repeat(level as usize));
  // an empty heading is just the number signs, so don't leave a trailing space
  items.push_signal(Signal::SpaceIfNotTrailing);
  items.extend(with_no_new_lines(children));
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

  // a paragraph begins where a block does, and a hard break puts what follows
  // it at the start of a line too, so neither can be left to read as the start
  // of a block of its own
  let escaped = escaped_block_starts(&paragraph.children);
  items.extend(context.with_escaped_block_starts(escaped, |context| {
    gen_task_list_marker_children(&paragraph.children, paragraph.marker.as_ref(), context)
  }));
  return items;

  /// Where each bit of the paragraph's text that is written at the start of a
  /// line starts, when it would be read as the start of a block.
  fn escaped_block_starts(children: &[Node]) -> Vec<(usize, usize)> {
    let mut starts = Vec::new();
    // the first line of a paragraph has nothing above it to be read together
    // with, so less of what it holds is markup than on the lines after it
    let mut escape: fn(&str) -> Option<usize> = block_start_escape;
    let mut at_line_start = true;
    for child in children {
      if at_line_start {
        if let Node::Text(text) = child {
          if let Some(position) = escape(text.text) {
            starts.push((text.span.start, position));
          }
        }
      }
      at_line_start = matches!(child, Node::HardBreak(_));
      if at_line_start {
        escape = line_start_escape;
      }
    }
    starts
  }
}

fn gen_block_quote(block_quote: &BlockQuote, context: &mut Context) -> PrintItems {
  let content_start = block_quote.children.first().map(|child| child.span().start);
  context.mark_in_block_quotes(content_start, |context, base_indents| {
    let mut items = PrintItems::new();

    // add a > for any string that is on the start of a line
    // Note: This is extremely hacky
    let mut indent_level = 0;
    // the opening `>` cannot rely on being at the start of a line, because a block
    // quote may begin mid-line -- for example directly after a list item marker.
    let mut needs_opening_marker = true;
    let children = gen_nodes(&block_quote.children, context);
    for print_item in get_content_print_items(children, context) {
      // a tab begins a line just as text does, so the markers belong before
      // either of them
      let is_content = matches!(&print_item, PrintItem::String(_) | PrintItem::Signal(Signal::Tab));
      // avoid inserting space in nested block quote markers (`> > foo`).
      let needs_space = !matches!(&print_item, PrintItem::String(text) if text.text == ">");
      if is_content {
        if needs_opening_marker {
          // at the beginning of a block quote, '>' is necessary
          // even if it is not at the start of a line i.e. the start of a list item.
          needs_opening_marker = false;
          items.push_optional_path(context.get_memoized_rc_path(MemoizedRcPathKind::FinishIndent(indent_level)));
          items.push_sc(sc!(">"));
          if needs_space {
            items.push_space();
          }
          items
            .push_optional_path(context.get_memoized_rc_path(MemoizedRcPathKind::StartWithSingleIndent(indent_level)));
        } else {
          items.push_condition(if_true(
            "angleBracketIfStartOfLine",
            condition_resolvers::is_start_of_line(),
            gen_block_quote_markers(
              &base_indents,
              indent_level,
              MarkersTrailing::Text { space: needs_space },
              context,
            ),
          ));
        }
        items.push_item(print_item);
        continue;
      }

      match print_item {
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
        // a nested block quote's markers indent from within a path of their
        // own, which counts towards the indentation the markers here go before
        PrintItem::RcPath(path) => {
          if let Some(delta) = context.memoized_path_indent_delta(path) {
            indent_level = indent_level.saturating_add_signed(delta);
          }
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

fn gen_code_block(code_block: &CodeBlock, position: NodePosition, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  let code_text = get_code_text(code_block, context);
  let code_text = if code_block.is_fenced() && context.configuration.code_block_preserve_blank_lines {
    code_text.as_ref()
  } else {
    code_text.trim_end_matches(WHITESPACE)
  };
  let code_text = if context.configuration.code_block_preserve_indentation {
    Cow::Borrowed(code_text)
  } else {
    utils::unindent(code_text)
  };
  // a backtick fence's info string can't hold a backtick, so a tag with one in
  // it has to be fenced with tildes
  let fence_char = match code_block.tag() {
    Some(tag) if tag.contains('`') => '~',
    _ => '`',
  };
  let fence_text = fence_char.to_string().repeat(get_fence_count(&code_text, fence_char));
  // an indented code block can only be written with indentation where nothing
  // above it would take that indentation for its own content, and where the
  // indentation of the lines after the first can match the first line's
  let lines_up = !position.beside_marker || position.marker_lines_up;
  let is_fenced = code_block.is_fenced() || position.after_list || !lines_up;
  let indent_level = if is_fenced { 0 } else { 4 };

  // header
  if is_fenced {
    items.push_str(&fence_text);
    if let Some(tag) = code_block.tag() {
      // an info string that starts with the fence character would otherwise
      // read as more of the fence
      if tag.starts_with(fence_char) {
        items.push_space();
      }
      // the info string may hold a tab, which the printer measures itself
      if tag.contains('\t') {
        items.extend(gen_text_with_tabs(tag.to_string()));
      } else {
        items.push_str(tag);
      }
    }
    items.push_signal(Signal::NewLine);
  }

  // body
  if !is_fenced && position.beside_marker {
    // the item's marker took the place of the first line's indentation, which
    // the printer only writes out for the lines after it
    items.push_sc(sc!("    "));
  }
  if !code_text.is_empty() {
    items.extend(ir_helpers::gen_from_string(&code_text));
  }

  // footer
  if is_fenced {
    if !code_text.is_empty() {
      items.push_signal(Signal::NewLine);
    }
    items.push_string(fence_text);
  }

  return with_indent_times(items, indent_level);

  fn get_code_text<'a>(code_block: &'a CodeBlock, context: &mut Context) -> Cow<'a, str> {
    let code = &code_block.code;
    let code = if code_block.is_fenced() && context.configuration.code_block_preserve_blank_lines {
      let code = code.strip_suffix('\n').unwrap_or(code);
      code.strip_suffix('\r').unwrap_or(code)
    } else {
      if code.trim_matches(WHITESPACE).is_empty() {
        return Cow::Borrowed("");
      }
      let start_pos = get_code_block_start_pos(code);
      code[start_pos..].trim_end_matches(WHITESPACE)
    };
    if !context.configuration.code_block_skip_format {
      if let Some(tag) = code_block.tag() {
        // allow situations like ```rust,ignore
        let tag = tag.chars().take_while(|&c| c != ' ' && c != ',').collect::<String>();
        if let Ok(Some(text)) = context.format_text(&tag, code) {
          // Formatters produce a string with a trailing newline, which must be removed.
          let text = text.strip_suffix("\n").unwrap_or(&text);
          return Cow::Owned(text.strip_suffix("\r").unwrap_or(text).to_owned());
        }
      }
    }
    Cow::Borrowed(code)
  }

  fn get_code_block_start_pos(text: &str) -> usize {
    let mut start_pos = 0;
    for (i, c) in text.char_indices() {
      if c == '\n' {
        start_pos = i + 1;
      } else if !WHITESPACE.contains(&c) {
        break;
      }
    }
    start_pos
  }

  /// How long the fence has to be to hold the code, which is longer than any
  /// run of the fence character within it.
  fn get_fence_count(text: &str, fence_char: char) -> usize {
    let mut count = 0;
    let mut max_count = 0;
    for c in text.chars() {
      if c == fence_char {
        count += 1;
        max_count = std::cmp::max(count, max_count);
      } else {
        count = 0;
      }
    }
    std::cmp::max(2, max_count) + 1
  }
}

fn gen_code(code: &Code, context: &mut Context) -> PrintItems {
  // a code span holds its text as it is, so the only choice here is how to
  // write the delimiters so that the text is read back out of them
  let text = code.code.as_ref();
  let backtick_text = "`".repeat(get_backtick_count(text));
  // a reader takes one space off each end of a span that has one at both, and
  // a backtick written against the delimiter would make the delimiter longer.
  // A space on each end handles either: the reader takes it straight back off
  let is_padded = text.starts_with([' ', '\n', '\r'])
    && text.ends_with([' ', '\n', '\r'])
    && !text.trim_matches([' ', '\n', '\r']).is_empty();
  let separator = if text.starts_with('`') || text.ends_with('`') || is_padded {
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

    // whitespace is only somewhere the span can be broken when there is a word
    // on either side of it -- the space a span opens with is text of its own
    let is_leading = current_word.is_empty() && items.is_empty();
    if whitespace_count == 1 && chars.peek().is_some() && !is_leading {
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
      // only the chunk is looked at, since a code span's text is written out
      // in chunks rather than a line at a time
      if utils::wrapping_word_starts_block(word, text_can_be_wrapped_away(context)) {
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
  if let Some(position) = context.block_start_escape_at(text.span.start) {
    let escaped = format!("{}\\{}", &text.text[..position], &text.text[position..]);
    return gen_str(&escaped, context);
  }
  if context.is_escaping_closing_hashes(text.span.start) {
    // the hashes are escaped where they start, which is enough to keep the
    // rest of the run from being read as a closing sequence of its own
    let hashes = text.text.len() - text.text.trim_end_matches('#').len();
    let escaped = format!(
      "{}\\{}",
      &text.text[..text.text.len() - hashes],
      &text.text[text.text.len() - hashes..]
    );
    return gen_str(&escaped, context);
  }
  gen_str(text.text, context)
}

/// Whether the node is a callout header (ex. `[!NOTE]`), which is only
/// recognized as one when it's the very first thing in a block quote.
fn is_callout_node(node: &Node, context: &Context) -> bool {
  match node {
    Node::Text(text) => context.is_block_quote_content_start(text.span.start) && is_callout_text(text.text),
    _ => false,
  }
}

fn is_callout_text(text: &str) -> bool {
  // ex. [!NOTE]
  let Some(kind) = text.strip_prefix("[!").and_then(|text| text.strip_suffix("]")) else {
    return false;
  };
  !kind.is_empty() && kind.chars().all(|c| c.is_ascii_uppercase())
}

fn gen_str(text: &str, context: &mut Context) -> PrintItems {
  let mut text_builder = TextBuilder::new(text, context);

  for (index, c) in text.char_indices() {
    text_builder.add_char(index, c);
  }

  return text_builder.build();

  struct TextBuilder<'a> {
    items: PrintItems,
    /// The text being written, which the words are read back out of in order
    /// to work out what a line would begin with.
    text: &'a str,
    /// The last character written, which decides how the space that follows it
    /// reads.
    last_char: Option<char>,
    current_word: Option<(usize, String)>,
    context: &'a Context<'a>,
  }

  impl<'a> TextBuilder<'a> {
    pub fn new(text: &'a str, context: &'a Context) -> TextBuilder<'a> {
      TextBuilder {
        items: PrintItems::new(),
        text,
        last_char: None,
        current_word: None,
        context,
      }
    }

    pub fn build(mut self) -> PrintItems {
      self.flush_current_word();
      self.items
    }

    pub fn add_char(&mut self, index: usize, character: char) {
      // a line break within a block reaches the formatter as a soft break of
      // its own, so a space is the only whitespace that gets here
      if character == ' ' {
        self.flush_current_word();
        return;
      }

      if let Some((_, current_word)) = self.current_word.as_mut() {
        current_word.push(character);
      } else {
        let mut word = String::new();
        word.push(character);
        self.current_word = Some((index, word));
      }
    }

    fn flush_current_word(&mut self) {
      if let Some((start, current_word)) = self.current_word.take() {
        if !self.items.is_empty() {
          self.items.extend(self.space_items(start, &current_word));
        }

        self.last_char = current_word.chars().last();
        self
          .items
          .extend(gen_word_with_unspaced_script_breaks(current_word, self.context));
      }
    }

    /// What to write in place of the space that ran up to the next word.
    fn space_items(&self, next_word_start: usize, next_word: &str) -> PrintItems {
      if utils::wrapping_word_starts_block(&self.text[next_word_start..], text_can_be_wrapped_away(self.context)) {
        // the text would start a block of its own at the start of a line, so
        // it has to be kept off one
        return space();
      }
      if self.is_between_unspaced_scripts(next_word) {
        // a space between two characters of a script written without spaces is
        // rendered, so it's kept as one rather than being written as the line
        // break that would read as nothing
        return space();
      }
      get_space_or_newline_based_on_config(self.context)
    }

    /// Whether the space sits between two characters of a script written
    /// without spaces between its words.
    fn is_between_unspaced_scripts(&self, next_word: &str) -> bool {
      matches!((self.last_char, next_word.chars().next()),
        (Some(last), Some(next)) if utils::is_unspaced_script(last) && utils::is_unspaced_script(next))
    }
  }
}

fn gen_text_decoration(text: &TextDecoration, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  let decoration_text = match decoration_delimiter(text, context.enclosing_decoration(), context) {
    "*" => sc!("*"),
    "_" => sc!("_"),
    "**" => sc!("**"),
    "__" => sc!("__"),
    "~" => sc!("~"),
    _ => sc!("~~"),
  };

  items.push_sc(decoration_text);
  let content = content_span(&text.children);
  items.extend(context.with_enclosing_decoration(content, |context| gen_nodes(&text.children, context)));
  items.push_sc(decoration_text);

  items
}

/// The delimiter a text decoration is written with.
///
/// The configured character is used unless the content it wraps begins or ends
/// with that character, where the two would run together into a longer
/// delimiter that means something else (ex. `_` around `_foo_` reads as
/// strong).
fn decoration_delimiter(decoration: &TextDecoration, parent_content: Option<Span>, context: &Context) -> &'static str {
  // the text of a reference link is the name it's matched by, so the
  // decorations within it are written with the characters they were written
  // with rather than the configured ones
  if context.is_preserving_decorations() {
    return written_delimiter(decoration, context);
  }
  // what this one is written with is read off the ones nested in it, so working
  // it out again for each of them would take twice as long at every level
  return context.decoration_delimiter(decoration.span.start, || resolve(decoration, parent_content, context));

  fn resolve(decoration: &TextDecoration, parent_content: Option<Span>, context: &Context) -> &'static str {
    let (asterisks, underscores, prefers_underscores) = match decoration.kind {
      TextDecorationKind::Emphasis => (
        "*",
        "_",
        context.configuration.emphasis_kind == EmphasisKind::Underscores,
      ),
      TextDecorationKind::Strong => ("**", "__", context.configuration.strong_kind == StrongKind::Underscores),
      TextDecorationKind::Strikethrough => return "~~",
    };
    // an underscore isn't read as a delimiter at all within a word, so only
    // asterisks will do there
    let (before, after) = surrounding_chars(decoration, parent_content, context);
    let within_word = is_word(before) || is_word(after);
    let prefers_underscores = prefers_underscores && !within_word;
    let (preferred, other) = if prefers_underscores {
      (underscores, asterisks)
    } else {
      (asterisks, underscores)
    };

    // the delimiter of a decoration written directly against this one is read by
    // what sits beside it, so changing this one's character would change theirs.
    // What the file already had parsed as this decoration, so it is safe.
    if within_word || is_delimiter(before) || is_delimiter(after) {
      return written_delimiter(decoration, context);
    }

    for candidate in [preferred, other] {
      if delimits(decoration, candidate, before, after, context) && !collides(decoration, candidate, context) {
        return candidate;
      }
    }
    // neither character can be written without running into the content, so keep
    // the one the file had
    written_delimiter(decoration, context)
  }

  /// The characters the decoration is written between, leaving out the
  /// delimiters of the decoration it is nested directly within -- those are
  /// written out here too, so they are chosen to sit beside this one rather
  /// than being something this one has to avoid.
  fn surrounding_chars(
    decoration: &TextDecoration,
    parent_content: Option<Span>,
    context: &Context,
  ) -> (Option<char>, Option<char>) {
    let span = decoration.span;
    let before = match parent_content {
      Some(content) if content.start == span.start => None,
      _ => context.file_text[..span.start].chars().next_back(),
    };
    let after = match parent_content {
      Some(content) if content.end == span.end => None,
      _ => context.file_text[span.end..].chars().next(),
    };
    (before, after)
  }

  fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric())
  }

  fn is_delimiter(c: Option<char>) -> bool {
    matches!(c, Some('*') | Some('_'))
  }

  /// Whether the character reads as a delimiter at all where the decoration is
  /// written, which the text on either side of it decides -- an underscore
  /// between two letters is one of them rather than a delimiter.
  fn delimits(
    decoration: &TextDecoration,
    delimiter: &str,
    before: Option<char>,
    after: Option<char>,
    context: &Context,
  ) -> bool {
    let character = delimiter.chars().next().unwrap();
    let content = content_span(&decoration.children);
    let first = written_first_char(decoration.children.first(), content, context);
    let last = written_last_char(decoration.children.last(), content, context);
    run_can_open(before, first, character) && run_can_close(last, after, character)
  }

  /// Whether the delimiter would run into the content it is written around,
  /// making a longer delimiter that means something else.
  fn collides(decoration: &TextDecoration, delimiter: &str, context: &Context) -> bool {
    let character = delimiter.chars().next().unwrap();
    let content = content_span(&decoration.children);
    written_first_char(decoration.children.first(), content, context) == Some(character)
      || written_last_char(decoration.children.last(), content, context) == Some(character)
      || text_can_pair(&decoration.children, character)
  }
}

/// The extent of what a decoration wraps, which is what its delimiters are
/// written directly against.
fn content_span(children: &[Node]) -> Option<Span> {
  Some(Span::new(children.first()?.span().start, children.last()?.span().end))
}

/// The delimiter the decoration was written with in the file.
fn written_delimiter(decoration: &TextDecoration, context: &Context) -> &'static str {
  let written = context.file_text.as_bytes().get(decoration.span.start).copied();
  match (decoration.kind, written) {
    (TextDecorationKind::Emphasis, Some(b'_')) => "_",
    (TextDecorationKind::Emphasis, _) => "*",
    (TextDecorationKind::Strong, Some(b'_')) => "__",
    (TextDecorationKind::Strong, _) => "**",
    (TextDecorationKind::Strikethrough, _) => {
      if context.file_text[decoration.span.start..].starts_with("~~") {
        "~~"
      } else {
        "~"
      }
    }
  }
}

/// The character the text ends with, unless a backslash escapes it and so
/// keeps it from merging with whatever follows.
fn last_unescaped_char(text: &str) -> Option<char> {
  let last = text.chars().next_back()?;
  let backslashes = text[..text.len() - last.len_utf8()]
    .chars()
    .rev()
    .take_while(|c| *c == '\\')
    .count();
  (backslashes % 2 == 0).then_some(last)
}

/// Whether any text within the nodes holds a run of the character that could
/// pair with a delimiter of that character written around them, which would
/// split that delimiter apart.
fn text_can_pair(nodes: &[Node], delimiter: char) -> bool {
  nodes.iter().any(|node| match node {
    Node::Text(text) => can_pair_with_delimiter(text.text, delimiter),
    // a decoration's own delimiters pair with each other rather than reaching
    // out, but the text it wraps is still text within this one
    node => text_can_pair(node.children(), delimiter),
  })
}

/// Whether the text holds a run of the character that could open or close
/// emphasis, and so pair with a delimiter of that character around it.
fn can_pair_with_delimiter(text: &str, delimiter: char) -> bool {
  let mut chars = text.char_indices().peekable();
  let mut is_escaped = false;
  while let Some((start, c)) = chars.next() {
    // an escaped character is text of its own and pairs with nothing
    if std::mem::take(&mut is_escaped) {
      continue;
    }
    if c == '\\' {
      is_escaped = true;
      continue;
    }
    if c != delimiter {
      continue;
    }
    let mut end = start + c.len_utf8();
    while chars.next_if(|(_, next)| *next == delimiter).is_some() {
      end += delimiter.len_utf8();
    }
    // what sits outside the text is the delimiter this could pair with, or
    // another node's punctuation -- either way, not whitespace
    let before = text[..start].chars().next_back().unwrap_or(delimiter);
    let after = text[end..].chars().next().unwrap_or(delimiter);
    if run_can_pair(Some(before), Some(after), delimiter) {
      return true;
    }
  }
  false
}

/// Whether a run of the character surrounded by those two can open or close
/// emphasis, which is the "flanking" rule of the CommonMark spec.
fn run_can_pair(before: Option<char>, after: Option<char>, delimiter: char) -> bool {
  run_can_open(before, after, delimiter) || run_can_close(before, after, delimiter)
}

/// Whether a run of the character surrounded by those two opens emphasis.
fn run_can_open(before: Option<char>, after: Option<char>, delimiter: char) -> bool {
  let flanking = Flanking::new(before, after);
  if delimiter == '_' {
    // `_` can't be used within a word
    flanking.left && (!flanking.right || flanking.before_punctuation)
  } else {
    flanking.left
  }
}

/// Whether a run of the character surrounded by those two closes emphasis.
fn run_can_close(before: Option<char>, after: Option<char>, delimiter: char) -> bool {
  let flanking = Flanking::new(before, after);
  if delimiter == '_' {
    flanking.right && (!flanking.left || flanking.after_punctuation)
  } else {
    flanking.right
  }
}

/// Which sides of a run of delimiters have text against them, which is what
/// decides whether the run opens emphasis, closes it, or does neither.
struct Flanking {
  left: bool,
  right: bool,
  before_punctuation: bool,
  after_punctuation: bool,
}

impl Flanking {
  fn new(before: Option<char>, after: Option<char>) -> Flanking {
    let before_whitespace = before.is_none_or(char::is_whitespace);
    let after_whitespace = after.is_none_or(char::is_whitespace);
    let before_punctuation = before.is_some_and(|c| c.is_ascii_punctuation());
    let after_punctuation = after.is_some_and(|c| c.is_ascii_punctuation());
    Flanking {
      left: !after_whitespace && (!after_punctuation || before_whitespace || before_punctuation),
      right: !before_whitespace && (!before_punctuation || after_whitespace || after_punctuation),
      before_punctuation,
      after_punctuation,
    }
  }
}

/// The character the node is written out beginning with, as far as a delimiter
/// beside it could run into.
fn written_first_char(node: Option<&Node>, parent_content: Option<Span>, context: &Context) -> Option<char> {
  match node? {
    Node::Text(text) => text.text.chars().next(),
    Node::TextDecoration(decoration) => decoration_delimiter(decoration, parent_content, context).chars().next(),
    Node::Code(_) => Some('`'),
    Node::Html(html) => html.text.chars().next(),
    Node::InlineMath(_) | Node::DisplayMath(_) => Some('$'),
    Node::AutoLink(_) => Some('<'),
    Node::InlineLink(_) | Node::ReferenceLink(_) | Node::ShortcutLink(_) | Node::FootnoteReference(_) => Some('['),
    Node::InlineImage(_) | Node::ReferenceImage(_) | Node::ShortcutImage(_) => Some('!'),
    _ => None,
  }
}

/// The character the node is written out ending with.
fn written_last_char(node: Option<&Node>, parent_content: Option<Span>, context: &Context) -> Option<char> {
  match node? {
    Node::Text(text) => last_unescaped_char(text.text),
    Node::TextDecoration(decoration) => decoration_delimiter(decoration, parent_content, context).chars().last(),
    Node::Code(_) => Some('`'),
    Node::Html(html) => html.text.chars().last(),
    Node::InlineMath(_) | Node::DisplayMath(_) => Some('$'),
    Node::AutoLink(_) => Some('>'),
    Node::InlineLink(_) | Node::InlineImage(_) => Some(')'),
    Node::ReferenceLink(_)
    | Node::ShortcutLink(_)
    | Node::ReferenceImage(_)
    | Node::ShortcutImage(_)
    | Node::FootnoteReference(_) => Some(']'),
    _ => None,
  }
}

fn gen_html(node: &Html, ctx: &mut Context) -> PrintItems {
  gen_range(node.span, ctx)
}

fn gen_display_math(node: &DisplayMath, ctx: &mut Context) -> PrintItems {
  gen_range(node.span, ctx)
}

fn gen_inline_math(node: &InlineMath, ctx: &mut Context) -> PrintItems {
  gen_range(node.span, ctx)
}

/// Writes out the text of the span as it is in the file, apart from the block
/// quote markers that the printer writes out itself.
fn gen_range(span: Span, ctx: &mut Context) -> PrintItems {
  let text = span.text(ctx.file_text).trim_end_matches(WHITESPACE);
  if text.is_empty() {
    return PrintItems::new();
  }
  let mut items = PrintItems::new();
  items.push_sc(sc!("")); // force first line indentation
  items.extend(ir_helpers::gen_from_raw_string(&trim_line_ends(
    &strip_raw_block_quote_markers(text, ctx),
  )));
  items
}

/// Trims the whitespace written at the end of each line, which is only a space
/// or a tab -- a character that merely looks like one is text of the line.
fn trim_line_ends(text: &str) -> Cow<'_, str> {
  if !text.split('\n').any(|line| line.ends_with(SPACES)) {
    return Cow::Borrowed(text);
  }
  let mut result = String::with_capacity(text.len());
  for (index, line) in text.split('\n').enumerate() {
    if index > 0 {
      result.push('\n');
    }
    result.push_str(line.trim_end_matches(SPACES));
  }
  Cow::Owned(result)
}

/// Removes the block quote markers that the continued lines of raw text picked
/// up from the file, keeping the indentation that follows them.
///
/// Only the markers the surrounding block quotes contributed are removed,
/// since the printer writes back exactly that many. Any further ones are the
/// text's own.
fn strip_raw_block_quote_markers<'a>(text: &'a str, context: &Context) -> Cow<'a, str> {
  let depth = context.block_quote_depth();
  // the first line is written out where the printer has already put the
  // markers, so only what follows it can have picked any up
  let continued_lines = || text.split('\n').skip(1);
  if depth == 0 || !continued_lines().any(|line| strip_markers(line, depth).len() != line.len()) {
    return Cow::Borrowed(text);
  }

  let mut result = String::with_capacity(text.len());
  result.push_str(text.split('\n').next().unwrap_or(""));
  for line in continued_lines() {
    result.push('\n');
    result.push_str(strip_markers(line, depth));
  }
  return Cow::Owned(result);

  fn strip_markers(line: &str, depth: usize) -> &str {
    let mut line = line;
    for _ in 0..depth {
      let trimmed = line.trim_start_matches([' ', '\t']);
      let Some(rest) = trimmed.strip_prefix('>') else {
        return line;
      };
      // a single space after a marker is part of it
      line = rest.strip_prefix(' ').unwrap_or(rest);
    }
    line
  }
}

fn gen_footnote_reference(footnote_reference: &FootnoteReference, _: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_string(format!("[^{}]", footnote_reference.name.trim_matches(WHITESPACE)));
  ir_helpers::with_no_new_lines(items)
}

fn gen_footnote_definition(footnote_definition: &FootnoteDefinition, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_string(format!("[^{}]: ", footnote_definition.name.trim_matches(WHITESPACE)));
  items.extend(with_indent_times(gen_nodes(&footnote_definition.children, context), 4));
  items
}

fn gen_inline_link(link: &InlineLink, context: &mut Context) -> PrintItems {
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    items.push_sc(sc!("["));
    items.extend(gen_nodes(&link.children, context));
    items.push_sc(sc!("]"));
    items.push_sc(sc!("("));
    items.extend(gen_link_destination_text(format_raw_link_destination(
      link.url.trim_matches(WHITESPACE),
    )));
    if let Some(title) = &link.title {
      items.extend(gen_title(title, context));
    }
    items.push_sc(sc!(")"));

    ir_helpers::new_line_group(items)
  })
}

/// Writes out a rendered link destination, handling the characters the printer
/// can't be handed as part of a string.
fn gen_link_destination_text(text: Cow<'_, str>) -> PrintItems {
  // a destination can't contain a line ending in either of its forms, so keep
  // one as the character reference it could only have come from
  let text = if text.contains(['\n', '\r']) {
    Cow::Owned(text.replace('\r', "&#13;").replace('\n', "&#10;"))
  } else {
    text
  };
  gen_text_with_tabs(text.into_owned())
}

/// Generates an image's alt text, which is the raw text from the file.
fn gen_image_alt_text(text: &str, context: &Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_sc(sc!("!["));
  items.extend(gen_raw_text(text.trim_matches(WHITESPACE), context));
  items.push_sc(sc!("]"));
  items
}

/// Generates the label of a reference image or link.
fn gen_reference_label(reference: &str, context: &Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_sc(sc!("["));
  items.extend(gen_raw_text(reference.trim_matches(WHITESPACE), context));
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
  items.extend(gen_raw_text(&escape_title(title.trim_matches(WHITESPACE)), context));
  items.push_sc(sc!("\""));
  items
}

/// Escapes the double quotes of a title that aren't escaped already, since a
/// title is always written out within them.
fn escape_title(title: &str) -> Cow<'_, str> {
  if !title.contains('"') && !title.ends_with('\\') {
    return Cow::Borrowed(title);
  }
  let mut text = String::with_capacity(title.len() + 2);
  let mut chars = title.chars();
  while let Some(c) = chars.next() {
    match c {
      '\\' => {
        text.push('\\');
        // a trailing backslash would escape the closing quote, so it escapes
        // itself instead
        text.push(chars.next().unwrap_or('\\'));
      }
      '"' => text.push_str("\\\""),
      _ => text.push(c),
    }
  }
  Cow::Owned(text)
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
    items.extend(gen_text_with_tabs(line.trim_end_matches(SPACES).to_string()));
  }
  for line in lines {
    items.extend(get_newline_wrapping_based_on_config(context));
    // the printer provides the indentation and block quote markers of a
    // continued line, so drop the ones this picked up from the file
    let line = strip_block_quote_markers(line, context);
    items.extend(gen_text_with_tabs(line.trim_end_matches(SPACES).to_string()));
  }
  items
}

/// Strips the markers a line of raw text picked up by continuing within a
/// block quote.
fn strip_block_quote_markers<'a>(line: &'a str, context: &Context) -> &'a str {
  let mut line = line.trim_start_matches(SPACES);
  if context.is_in_block_quote() {
    while let Some(rest) = line.strip_prefix('>') {
      line = rest.trim_start_matches(SPACES);
    }
  }
  line
}

/// Writes the word, breaking it up where a line can be broken within a script
/// written without spaces between its words.
///
/// A run of such a script holds no spaces for the line to be broken at, so
/// without this it would be written on one line however long it ran. Only a
/// break with one of its characters on either side can be written: anywhere
/// else the break would be read back as a space that wasn't written.
fn gen_word_with_unspaced_script_breaks(word: String, context: &Context) -> PrintItems {
  if context.configuration.text_wrap != TextWrap::Always || context.is_text_wrap_disabled() {
    return gen_text_with_tabs(word);
  }

  let mut items = PrintItems::new();
  let mut segment_start = 0;
  let mut last_char: Option<char> = None;
  for (index, character) in word.char_indices() {
    if matches!(last_char, Some(last) if breaks_between(last, character)) {
      items.extend(gen_text_with_tabs(word[segment_start..index].to_string()));
      items.push_signal(Signal::PossibleNewLine);
      segment_start = index;
    }
    last_char = Some(character);
  }
  items.extend(gen_text_with_tabs(word[segment_start..].to_string()));

  return items;

  /// Whether a line can be broken between the two characters, which needs both
  /// to belong to a script written without spaces, and neither to be a mark
  /// that has to stay with the character beside it.
  fn breaks_between(last: char, next: char) -> bool {
    utils::is_unspaced_script(last)
      && utils::is_unspaced_script(next)
      && !utils::forbids_line_break_after(last)
      && !utils::forbids_line_break_before(next)
  }
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

/// Whether the destination has to be written within pointy brackets, which is
/// when it starts with `<`, holds a space or an ascii control character, or has
/// unbalanced parentheses.
fn link_destination_needs_pointy_brackets(destination: &str) -> bool {
  let mut chars = destination.chars().peekable();
  let mut parentheses_depth = 0;
  let mut is_first = true;
  while let Some(c) = chars.next() {
    // an escaped character stands for itself once the escape is resolved
    let c = if c == '\\' && chars.peek().is_some_and(|next| next.is_ascii_punctuation()) {
      chars.next().unwrap()
    } else {
      c
    };
    match c {
      '<' if is_first => return true,
      c if c == ' ' || c.is_ascii_control() => return true,
      '(' => parentheses_depth += 1,
      ')' => parentheses_depth -= 1,
      _ => (),
    }
    is_first = false;
    if parentheses_depth < 0 {
      return true;
    }
  }
  parentheses_depth != 0
}

fn gen_reference_link(link: &ReferenceLink, context: &mut Context) -> PrintItems {
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    items.push_sc(sc!("["));
    // a collapsed reference is named by its own text, so that text is kept as
    // it was written rather than being normalized out of matching
    items.extend(gen_link_text(&link.children, link.is_collapsed, context));
    items.push_sc(sc!("]"));
    items.extend(gen_reference_label(
      if link.is_collapsed { "" } else { &link.reference },
      context,
    ));
    ir_helpers::new_line_group(items)
  })
}

/// Writes out the text between a link's brackets, keeping it exactly as it was
/// written where that text is the name the link is matched by.
fn gen_link_text(children: &[Node], is_the_name: bool, context: &mut Context) -> PrintItems {
  if is_the_name {
    context.with_preserved_decorations(|context| gen_nodes(children, context))
  } else {
    gen_nodes(children, context)
  }
}

fn gen_shortcut_link(link: &ShortcutLink, context: &mut Context) -> PrintItems {
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    items.push_sc(sc!("["));
    items.extend(gen_link_text(&link.children, true, context));
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

  let url = format_raw_link_destination(link_ref.link.trim_matches(WHITESPACE));
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
fn format_raw_link_destination(destination: &str) -> Cow<'_, str> {
  let destination = destination
    .strip_prefix('<')
    .and_then(|destination| destination.strip_suffix('>'))
    .unwrap_or(destination);
  let needs_pointy_brackets = link_destination_needs_pointy_brackets(destination);
  // the overwhelmingly common destination is written back out as it is
  if !needs_pointy_brackets && !destination.contains('\\') {
    return Cow::Borrowed(destination);
  }

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
  Cow::Owned(text)
}

fn gen_inline_image(image: &InlineImage, context: &mut Context) -> PrintItems {
  context.with_no_text_wrap(|context| {
    let mut items = PrintItems::new();
    items.extend(gen_image_alt_text(&image.text, context));
    items.push_sc(sc!("("));
    // like a link reference definition, this is the raw text from the file
    items.extend(gen_link_destination_text(format_raw_link_destination(
      image.url.trim_matches(WHITESPACE),
    )));
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
        if context.has_leading_blankline(child.span().start) {
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
      let marker_width = prefix_text.chars().count() as u32 + 1;
      let indent_increment = match context.configuration.list_indent_kind {
        crate::configuration::ListIndentKind::CommonMark => marker_width,
        crate::configuration::ListIndentKind::PythonMarkdown => std::cmp::max(marker_width, 4),
      };
      // only a bullet marker shares its character with a thematic break
      let marker_char = list.start_index.is_none().then(|| prefix_text.chars().next()).flatten();
      let marker_lines_up = indent_increment == marker_width;
      context.indent_level += indent_increment;
      items.push_string(prefix_text);
      // the space is dropped where nothing follows it on the line, so an item
      // with nothing in it is written as the marker alone. Whether anything
      // does follow is the item's to say, not something to measure: content of
      // no width is content all the same
      items.push_signal(Signal::SpaceIfNotTrailing);
      let child_items = context.mark_in_list_item(marker_char, marker_lines_up, |context| generate(child, context));
      items.extend(with_indent_times(child_items, indent_increment));
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

  // what the item begins with is written beside its marker, unless a blank
  // line puts it on a line of its own
  if let Some(first) = item.children.first() {
    if !context.has_leading_blankline(first.span().start) {
      context.mark_marker_beside();
    }
  }
  items.extend(gen_task_list_marker_children(
    &item.children,
    item.marker.as_ref(),
    context,
  ));

  if !item.sub_lists.is_empty() {
    items.push_signal(Signal::NewLine);
    if context.has_leading_blankline(item.sub_lists.first().unwrap().span().start) {
      items.push_signal(Signal::NewLine);
    }
    items.extend(gen_nodes(&item.sub_lists, context));
  }

  items
}

fn gen_definition_list(definition_list: &DefinitionList, context: &mut Context) -> PrintItems {
  context.mark_in_list(|context| {
    let mut items = PrintItems::new();

    for (index, child) in definition_list.children.iter().enumerate() {
      if index > 0 {
        items.push_signal(Signal::NewLine);
        if context.has_leading_blankline(child.span().start) {
          items.push_signal(Signal::NewLine);
        }
      }

      items.extend(match child {
        Node::DefinitionListTitle(title) => gen_term(title, context),
        Node::DefinitionListDefinition(definition) => gen_definition(definition, context),
        _ => generate(child, context),
      });
    }

    items
  })
}

/// Every term of a definition list must be kept on its own line, otherwise it
/// stops being a definition list, so keep the line breaks found in the source
/// and never wrap the text.
fn gen_term(title: &DefinitionListTitle, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  let terms = title
    .children
    .split(|c| matches!(c, Node::SoftBreak(_) | Node::HardBreak(_)));

  for (index, term) in terms.enumerate() {
    if index > 0 {
      items.push_signal(Signal::NewLine);
    }
    items.extend(context.with_no_text_wrap(|context| ir_helpers::with_no_new_lines(gen_nodes(term, context))));
  }

  items
}

fn gen_definition(definition: &DefinitionListDefinition, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  let indent_increment = match context.configuration.list_indent_kind {
    crate::configuration::ListIndentKind::CommonMark => 2,
    crate::configuration::ListIndentKind::PythonMarkdown => 4,
  };

  context.indent_level += indent_increment;
  items.push_sc(sc!(":"));
  items.push_signal(Signal::SpaceIfNotTrailing);
  items.extend(with_indent_times(
    gen_nodes(&definition.children, context),
    indent_increment,
  ));
  context.indent_level -= indent_increment;

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
        Node::List(_)
          | Node::DefinitionList(_)
          | Node::CodeBlock(_)
          | Node::BlockQuote(_)
          | Node::Heading(_)
          | Node::Table(_)
      ) || context.has_leading_blankline(c.span().start)
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
    if context.has_leading_blankline(children[indent_child_index_end].span().start) {
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

fn gen_horizontal_rule(_: &HorizontalRule, position: NodePosition) -> PrintItems {
  // a break made of the same character as the marker beside it would read as
  // one long break instead (ex. `- ---`), and one made of dashes below a
  // paragraph would underline it into a heading
  let runs_together = position.beside_marker && position.marker_char == Some('-');
  match runs_together || position.after_paragraph {
    true => "***".into(),
    false => "---".into(),
  }
}

/// Generates a hard break, which has no representation on a single line, so it
/// becomes a space where newlines are being forced off (ex. within an ATX
/// heading). Otherwise the marker it leaves behind would either escape the
/// character that followed it or be collapsed as extra whitespace.
fn gen_hard_break(context: &mut Context) -> PrintItems {
  /// The two spaces a double space hard break leaves behind, measured as zero
  /// columns because trailing whitespace isn't visible.
  const DOUBLE_SPACE: StringContainer = StringContainer::proc_macro_new_with_char_count("  ", 0);
  /// Stands in for the text a hard break writes, which is otherwise hidden
  /// within a condition where nothing rewriting the line can see it (ex. a
  /// block quote, which writes its markers in front of a line's text).
  const WRITES_TEXT: StringContainer = StringContainer::proc_macro_new_with_char_count("", 0);

  let hard_break = {
    let mut items = PrintItems::new();
    match context.configuration.hard_break_kind {
      HardBreakKind::Backslash => items.push_sc(sc!("\\")),
      // the two spaces sit at the end of the line, where they take no visible
      // width, so they're written as zero columns to keep them out of the
      // decision of where the text before them wraps
      HardBreakKind::DoubleSpace => items.push_sc(&DOUBLE_SPACE),
    }
    items.push_signal(Signal::NewLine);
    items
  };
  let mut items = PrintItems::new();
  items.push_sc(&WRITES_TEXT);
  items.push_condition(if_true_or(
    "hardBreakOrSpaceIfNewlinesDisabled",
    condition_resolvers::is_forcing_no_newlines(),
    space(),
    hard_break,
  ));
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
      // a cell past the last column has no column to be sized or aligned to,
      // so it is written out as it is
      let difference = column_widths.get(i).map(|width| width - cell_width).unwrap_or(0);
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
    // the delimiter row is what says how many columns the table has, and a
    // cell written past the last of them is no part of any column
    let mut column_widths = Vec::with_capacity(column_alignments.len());
    for (i, column_alignment) in column_alignments.iter().enumerate() {
      // + 1 in order to have at least one dash
      let mut max_width = get_column_alignment_properties(*column_alignment).count() + 1;

      // get header width
      if let Some((_, width)) = header.get(i) {
        max_width = std::cmp::max(max_width, *width);
      }

      // check each row width
      for row in rows.iter() {
        if let Some((_, width)) = row.get(i) {
          max_width = std::cmp::max(max_width, *width);
        }
      }

      column_widths.push(max_width);
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
        .format_text("yaml", node.text)
        .ok()
        .flatten()
        .map(Cow::from)
        .unwrap_or_else(|| Cow::from(node.text));
      items.extend(ir_helpers::gen_from_string(&trim_line_ends(
        text.trim_end_matches(WHITESPACE),
      )));
    }
    MetadataBlockKind::PlusesStyle => {
      items.extend(ir_helpers::gen_from_raw_string(&trim_line_ends(
        node.text.trim_end_matches(WHITESPACE),
      )));
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
  // trailing whitespace isn't visible, so ignore it when measuring
  // (ex. the two spaces of a hard break)
  rendered
    .lines()
    .map(|line| UnicodeWidthStr::width(line.trim_end_matches(WHITESPACE)))
    .max()
    .unwrap_or(0)
}

/// Whether the text after a word can be wrapped onto the line below it, which
/// is what would leave the word by itself on a line of its own.
fn text_can_be_wrapped_away(context: &Context) -> bool {
  context.configuration.text_wrap == TextWrap::Always && !context.is_text_wrap_disabled()
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

/// What takes the place of a line break that falls between two characters of
/// a script written without spaces between its words, where the break isn't
/// rendered as a space and so is dropped rather than turned into one.
fn get_unspaced_script_newline_wrapping(context: &Context) -> PrintItems {
  match context.configuration.text_wrap {
    // the line may still be broken where it was, since the break isn't read
    // as anything either way
    TextWrap::Always if !context.is_text_wrap_disabled() => Signal::PossibleNewLine.into(),
    TextWrap::Always | TextWrap::Never => PrintItems::new(),
    // where newlines are being forced off (ex. within a heading) the printer
    // drops this one, which is what should take the place of a break that read
    // as nothing anyway
    TextWrap::Maintain => Signal::NewLine.into(),
  }
}

fn get_newline_wrapping_based_on_config(context: &Context) -> PrintItems {
  if context.is_text_wrap_disabled() {
    // ex. within a link, whose text is moved onto a single line when text is
    // being wrapped, but keeps the line breaks it has when text is maintained
    return match context.configuration.text_wrap {
      TextWrap::Always | TextWrap::Never => space(),
      TextWrap::Maintain => if_true_or(
        "newLineOrSpaceIfNewlinesDisabled",
        condition_resolvers::is_forcing_no_newlines(),
        space(),
        Signal::NewLine.into(),
      )
      .into(),
    };
  }
  match context.configuration.text_wrap {
    TextWrap::Always => Signal::SpaceOrNewLine.into(),
    TextWrap::Never => space(),
    // a line break can't be written where newlines are being forced off
    // (ex. within a heading), where it stands for the space between the words
    // it separated
    TextWrap::Maintain => if_true_or(
      "newLineOrSpaceIfNewlinesDisabled",
      condition_resolvers::is_forcing_no_newlines(),
      space(),
      Signal::NewLine.into(),
    )
    .into(),
  }
}

/// If the list's first items are both 1s
fn is_all_ones_list(list: &List, context: &Context) -> bool {
  list.start_index == Some(1)
    && matches!(
      list.children.get(1),
      Some(Node::Item(item)) if item.marker_span.text(context.file_text).starts_with('1')
    )
}

#[cfg(test)]
mod tests {
  use super::format_raw_link_destination;

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
