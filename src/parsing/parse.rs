use super::common::*;
use super::parser_types::*;
use super::utils;
use crate::configuration::*;
use dprint_core::formatting::*;
use dprint_core::formatting::{condition_resolvers, conditions::*, parser_helpers::*};
use std::borrow::Cow;

pub fn parse_node(node: &Node, context: &mut Context) -> PrintItems {
  // println!("Kind: {:?}", node.kind());
  // println!("Text: {:?}", node.text(context));
  match node {
    Node::SourceFile(node) => parse_source_file(node, context),
    Node::Heading(node) => parse_heading(node, context),
    Node::Paragraph(node) => parse_paragraph(node, context),
    Node::BlockQuote(node) => parse_block_quote(node, context),
    Node::CodeBlock(node) => parse_code_block(node, context),
    Node::Code(node) => parse_code(node, context),
    Node::Text(node) => parse_text(node, context),
    Node::TextDecoration(node) => parse_text_decoration(node, context),
    Node::Html(node) => parse_html(node, context),
    Node::FootnoteReference(node) => parse_footnote_reference(node, context),
    Node::FootnoteDefinition(node) => parse_footnote_definition(node, context),
    Node::InlineLink(node) => parse_inline_link(node, context),
    Node::ReferenceLink(node) => parse_reference_link(node, context),
    Node::ShortcutLink(node) => parse_shortcut_link(node, context),
    Node::AutoLink(node) => parse_auto_link(node, context),
    Node::LinkReference(node) => parse_link_reference(node, context),
    Node::InlineImage(node) => parse_inline_image(node, context),
    Node::ReferenceImage(node) => parse_reference_image(node, context),
    Node::List(node) => parse_list(node, false, context),
    Node::Item(node) => parse_item(node, context),
    Node::TaskListMarker(node) => parse_task_list_marker(node, context),
    Node::HorizontalRule(node) => parse_horizontal_rule(node, context),
    Node::SoftBreak(_) => PrintItems::new(),
    Node::HardBreak(_) => parse_hard_break(context),
    Node::Table(node) => parse_table(node, context),
    Node::TableHead(_) => unreachable!(),
    Node::TableRow(_) => unreachable!(),
    Node::TableCell(node) => parse_table_cell(node, context),
    Node::NotImplemented(_) => parse_raw_string(node.text(context)),
  }
}

fn parse_source_file(source_file: &SourceFile, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();

  if let Some(yaml_header) = &source_file.yaml_header {
    items.extend(parser_helpers::parse_raw_string(&yaml_header.text));

    if source_file.children.len() > 0 {
      items.push_signal(Signal::NewLine);
      items.push_signal(Signal::NewLine);
    }
  }

  items.extend(parse_nodes(&source_file.children, context));

  items.push_condition(if_true(
    "endOfFileNewLine",
    |context| Some(context.writer_info.column_number > 0 || context.writer_info.line_number > 0),
    Signal::NewLine.into(),
  ));

  items
}

fn parse_nodes(nodes: &Vec<Node>, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  let mut last_node: Option<&Node> = None;
  let mut node_iterator = nodes.iter().filter(|n| !matches!(n, Node::SoftBreak(_)));

  while let Some(node) = node_iterator.next() {
    let mut node = node;

    // handle alternate lists
    if let Some(Node::List(last_list)) = &last_node {
      if let Node::List(list) = &node {
        if last_list.start_index.is_some() == list.start_index.is_some() {
          items.extend(get_conditional_blank_line(node.range(), context));
          items.extend(parse_list(list, true, context));
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
          | Node::BlockQuote(_) => {
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
          | Node::ReferenceImage(_) => {
            let between_range = (last_node.range().end, node.range().start);
            let new_line_count = context.get_new_lines_in_range(between_range.0, between_range.1);

            if new_line_count == 1 {
              if matches!(node, Node::Html(_)) {
                items.push_signal(Signal::NewLine);
              } else {
                items.extend(get_newline_wrapping_based_on_config(context));
              }
            } else if new_line_count > 1 {
              items.push_signal(Signal::NewLine);
              items.push_signal(Signal::NewLine);
            } else {
              let needs_space = if matches!(last_node, Node::Text(_)) || matches!(node, Node::Text(_)) {
                if let Node::Html(_) = last_node {
                  node.has_preceeding_space(&context.file_text)
                } else {
                  node.has_preceeding_space(&context.file_text)
                    || !last_node.ends_with_punctuation(&context.file_text) && !node.starts_with_punctuation(&context.file_text)
                }
              } else if let Node::FootnoteReference(_) = node {
                false
              } else if let Node::Html(_) = node {
                node.has_preceeding_space(&context.file_text)
              } else {
                true
              };

              if needs_space {
                if node.starts_with_list_word() {
                  items.push_str(" ");
                } else {
                  items.extend(get_space_or_newline_based_on_config(context));
                }
              }
            }
          }
          Node::LinkReference(_) => {
            let needs_newline = if let Node::LinkReference(_) = node { true } else { false };
            if needs_newline {
              items.push_signal(Signal::NewLine);
            }
          }
          _ => {}
        }
      }
    }

    items.extend(parse_node(node, context));
    last_node = Some(node);

    // check for ignore comment
    if let Node::Html(html) = node {
      if context.ignore_regex.is_match(&html.text) {
        items.push_signal(Signal::NewLine);
        if let Some(node) = node_iterator.next() {
          if utils::has_leading_blankline(node.range().start, context.file_text) {
            items.push_signal(Signal::NewLine);
          }
          items.extend(parser_helpers::parse_raw_string(node.text(context).trim_end()));
          last_node = Some(node);
        }
      } else if context.ignore_start_regex.is_match(&html.text) {
        let mut range: Option<Range> = None;
        let mut end_comment = None;
        while let Some(node) = node_iterator.next() {
          last_node = Some(node);

          if let Node::Html(html) = node {
            if context.ignore_end_regex.is_match(&html.text) {
              end_comment = Some(html);
              break;
            }
          }

          if let Some(taken_range) = range {
            range = Some(Range {
              start: taken_range.start,
              end: node.range().end,
            });
          } else {
            range = Some(node.range().to_owned());
          }
        }

        if let Some(range) = range {
          items.extend(get_conditional_blank_line(&range, context));
          items.extend(parser_helpers::parse_raw_string(&context.file_text[range.start..range.end].trim()));
          if let Some(end_comment) = end_comment {
            items.extend(get_conditional_blank_line(end_comment.range(), context));
            items.extend(parse_html(end_comment, context));
          }
        }
      }
    }
  }

  return items;

  fn get_conditional_blank_line(range: &Range, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    if !context.is_in_list() || utils::has_leading_blankline(range.start, context.file_text) {
      items.push_signal(Signal::NewLine);
    }
    items.push_signal(Signal::NewLine);
    items
  }
}

fn parse_heading(heading: &Heading, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();

  items.push_str(&format!("{} ", "#".repeat(heading.level as usize)));
  items.extend(with_no_new_lines(parse_nodes(&heading.children, context)));

  items
}

fn parse_paragraph(paragraph: &Paragraph, context: &mut Context) -> PrintItems {
  parse_nodes(&paragraph.children, context)
}

fn parse_block_quote(block_quote: &BlockQuote, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();

  // add a > for any string that is on the start of a line
  for print_item in parse_nodes(&block_quote.children, context).iter() {
    match print_item {
      PrintItem::String(text) => {
        items.push_condition(if_true(
          "angleBracketIfStartOfLine",
          |context| Some(context.writer_info.is_start_of_line()),
          "> ".into(),
        ));
        items.push_item(PrintItem::String(text));
      }
      PrintItem::Signal(Signal::NewLine) => {
        items.push_condition(if_true(
          "angleBracketIfStartOfLine",
          |context| Some(context.writer_info.is_start_of_line()),
          ">".into(),
        ));
        items.push_signal(Signal::NewLine);
      }
      _ => items.push_item(print_item),
    }
  }

  items
}

fn parse_code_block(code_block: &CodeBlock, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  let measured_raw_indent_level = context.get_indent_level_at_pos(code_block.range.start);
  let relative_raw_indent_level = utils::safe_subtract_to_zero(measured_raw_indent_level, context.raw_indent_level);
  let relative_indent_level = ((relative_raw_indent_level as f64 / 4_f64).round() * 4_f64) as u32;
  let code_text = get_code_text(code_block, context);
  let backtick_text = "`".repeat(get_backtick_count(&code_text));

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
    items.extend(parser_helpers::parse_string(&code_text));
  }

  // footer
  if code_block.is_fenced {
    if !code_text.is_empty() {
      items.push_signal(Signal::NewLine);
    }
    items.push_str(&backtick_text);
  }

  return with_indent_times(items, relative_indent_level);

  fn get_code_text<'a>(code_block: &'a CodeBlock, context: &mut Context) -> Cow<'a, str> {
    let code = code_block.code.trim();
    if !code.is_empty() {
      if let Some(tag) = &code_block.tag {
        if let Ok(text) = context.format_text(tag, code) {
          return Cow::Owned(text.trim().to_string());
        }
      }
    }
    Cow::Borrowed(code)
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

fn parse_code(code: &Code, _: &mut Context) -> PrintItems {
  let text = code.code.trim();
  let mut backtick_text = "`";
  let mut separator = "";
  if text.contains("`") {
    backtick_text = "``";
    if text.starts_with("`") || text.ends_with("`") {
      separator = " ";
    }
  }

  format!("{}{}{}{}{}", backtick_text, separator, text, separator, backtick_text).into()
}

fn parse_text(text: &Text, context: &mut Context) -> PrintItems {
  let mut text_builder = TextBuilder::new(context);

  for c in text.text.chars() {
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
      if character.is_whitespace() {
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
          if utils::is_list_word(&current_word) {
            self.items.push_str(" ");
          } else if self.was_last_newline {
            self.items.push_signal(Signal::NewLine)
          } else {
            self.items.extend(get_space_or_newline_based_on_config(self.context));
          }
        }

        self.items.push_str(&current_word);
        self.was_last_newline = false;
      }
    }
  }
}

fn parse_text_decoration(text: &TextDecoration, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  let decoration_text = match &text.kind {
    TextDecorationKind::Emphasis => match context.configuration.emphasis_kind {
      EmphasisKind::Asterisks => "*",
      EmphasisKind::Underscores => "_",
    },
    TextDecorationKind::Strong => match context.configuration.strong_kind {
      StrongKind::Asterisks => "**",
      StrongKind::Underscores => "__",
    },
    TextDecorationKind::Strikethrough => "~~",
  };

  items.push_str(&decoration_text);
  items.extend(parse_nodes(&text.children, context));
  items.push_str(&decoration_text);

  items
}

fn parse_html(html: &Html, _: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_str(html.text.trim_end());
  items
}

fn parse_footnote_reference(footnote_reference: &FootnoteReference, _: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_str(&format!("[^{}]", footnote_reference.name.trim()));
  parser_helpers::with_no_new_lines(items)
}

fn parse_footnote_definition(footnote_definition: &FootnoteDefinition, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_str(&format!("[^{}]: ", footnote_definition.name.trim()));
  items.extend(parse_nodes(&footnote_definition.children, context));
  items
}

fn parse_inline_link(link: &InlineLink, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  let parsed_children = parse_nodes(&link.children, context);
  items.push_str("[");

  // force the text to be on a single line in some scenarios
  let (parsed_children, parsed_children_clone) = clone_items(parsed_children);
  let single_line_text = get_items_text(parser_helpers::with_no_new_lines(parsed_children_clone));
  if single_line_text.len() < (context.configuration.line_width / 2) as usize {
    items.push_str(&single_line_text);
  } else {
    items.extend(parsed_children);
  }

  items.push_str("]");
  items.push_str("(");
  items.push_str(&link.url.trim());
  if let Some(title) = &link.title {
    items.push_str(&format!(" \"{}\"", title.trim()));
  }
  items.push_str(")");

  parser_helpers::new_line_group(items)
}

fn parse_reference_link(link: &ReferenceLink, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_str("[");
  items.extend(parse_nodes(&link.children, context));
  items.push_str("]");
  items.push_str(&format!("[{}]", link.reference.trim()));
  parser_helpers::new_line_group(items)
}

fn parse_shortcut_link(link: &ShortcutLink, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_str("[");
  items.extend(parse_nodes(&link.children, context));
  items.push_str("]");
  parser_helpers::new_line_group(items)
}

fn parse_auto_link(link: &AutoLink, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_str("<");
  items.extend(parse_nodes(&link.children, context));
  items.push_str(">");
  parser_helpers::new_line_group(items)
}

fn parse_link_reference(link_ref: &LinkReference, _: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_str(&format!("[{}]: ", link_ref.name.trim()));
  items.push_str(&link_ref.link.trim());
  if let Some(title) = &link_ref.title {
    items.push_str(&format!(" \"{}\"", title.trim()));
  }
  parser_helpers::new_line_group(items)
}

fn parse_inline_image(image: &InlineImage, _: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_str(&format!("![{}]", image.text.trim()));
  items.push_str("(");
  items.push_str(&image.url.trim());
  if let Some(title) = &image.title {
    items.push_str(&format!(" \"{}\"", title.trim()));
  }
  items.push_str(")");
  parser_helpers::new_line_group(items)
}

fn parse_reference_image(image: &ReferenceImage, _: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_str(&format!("![{}]", image.text.trim()));
  items.push_str(&format!("[{}]", image.reference.trim()));
  parser_helpers::new_line_group(items)
}

fn parse_list(list: &List, is_alternate: bool, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  context.is_in_list_count += 1;
  let measured_raw_indent_level = context.get_indent_level_at_pos(list.range.start);
  let raw_list_start_indent_increment = utils::safe_subtract_to_zero(measured_raw_indent_level, context.raw_indent_level);
  context.raw_indent_level += raw_list_start_indent_increment;

  // parse items
  for (index, child) in list.children.iter().enumerate() {
    if index > 0 {
      items.push_signal(Signal::NewLine);
      if utils::has_leading_blankline(child.range().start, context.file_text) {
        items.push_signal(Signal::NewLine);
      }
    }
    let prefix_text = if let Some(start_index) = list.start_index {
      let end_char = if is_alternate { ")" } else { "." };
      let display_index = if is_all_ones_list(list, context) { 1 } else { start_index + index as u64 };
      format!("{}{}", display_index, end_char)
    } else {
      String::from(if is_alternate { "*" } else { "-" })
    };
    let indent_increment = (prefix_text.chars().count() + 1) as u32;
    context.indent_level += indent_increment;
    context.raw_indent_level += indent_increment;
    items.push_str(&prefix_text);
    let after_child = Info::new("afterChild");
    items.push_condition(if_true(
      "spaceIfHasChild",
      move |context| Some(!condition_resolvers::is_at_same_position(context, &after_child)?),
      Signal::SpaceIfNotTrailing.into(),
    ));
    items.extend(with_indent_times(parse_node(child, context), indent_increment));
    items.push_info(after_child);
    context.indent_level -= indent_increment;
    context.raw_indent_level -= indent_increment;
  }

  context.raw_indent_level -= raw_list_start_indent_increment;
  context.is_in_list_count -= 1;
  items
}

fn parse_item(item: &Item, context: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();

  if let Some(marker) = &item.marker {
    items.extend(parse_task_list_marker(marker, context));
    if !item.children.is_empty() {
      items.push_str(" ");
    }
  }

  items.extend(parse_nodes(&item.children, context));

  if !item.sub_lists.is_empty() {
    items.push_signal(Signal::NewLine);
    if utils::has_leading_blankline(item.sub_lists.first().unwrap().range().start, context.file_text) {
      items.push_signal(Signal::NewLine);
    }
    items.extend(parse_nodes(&item.sub_lists, context));
  }

  items
}

fn parse_task_list_marker(marker: &TaskListMarker, _: &mut Context) -> PrintItems {
  if marker.is_checked {
    "[x]".into()
  } else {
    "[ ]".into()
  }
}

fn parse_horizontal_rule(_: &HorizontalRule, _: &mut Context) -> PrintItems {
  "---".into()
}

fn parse_hard_break(_: &mut Context) -> PrintItems {
  let mut items = PrintItems::new();
  items.push_str("\\");
  items.push_signal(Signal::NewLine);
  items
}

fn parse_table(table: &Table, context: &mut Context) -> PrintItems {
  let header = table
    .header
    .cells
    .iter()
    .map(|cell| get_cell_items_and_width(cell, context))
    .collect::<Vec<_>>();
  let rows = table
    .rows
    .iter()
    .map(|row| row.cells.iter().map(|cell| get_cell_items_and_width(cell, context)).collect::<Vec<_>>())
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

  fn get_divider_row(column_widths: &Vec<usize>, column_alignments: &Vec<ColumnAlignment>) -> PrintItems {
    let mut items = PrintItems::new();
    for (i, column_width) in column_widths.iter().enumerate() {
      let column_alignment = column_alignments.get(i).map(|x| *x).unwrap_or(ColumnAlignment::None);
      if i == 0 {
        items.push_str("| ");
      } else {
        items.push_str(" ");
      }

      let column_alignment_props = get_column_alignment_properties(column_alignment);
      let dashes_count = column_width - column_alignment_props.count();

      if column_alignment_props.has_left_colon {
        items.push_str(":");
      }
      items.push_str(&"-".repeat(dashes_count));
      if column_alignment_props.has_right_colon {
        items.push_str(":");
      }

      items.push_str(" |");
    }

    parser_helpers::with_no_new_lines(items)
  }

  fn get_row_items(row_cells: Vec<(PrintItems, usize)>, column_widths: &Vec<usize>, column_alignments: &Vec<ColumnAlignment>) -> PrintItems {
    let mut items = PrintItems::new();
    for (i, (cell_items, cell_width)) in row_cells.into_iter().enumerate() {
      let column_alignment = column_alignments.get(i).map(|x| *x).unwrap_or(ColumnAlignment::None);
      let column_max_width = *column_widths.get(i).unwrap();
      let difference = column_max_width - cell_width;
      if i == 0 {
        items.push_str("| ")
      } else {
        items.push_str(" ");
      }

      if difference > 0 {
        match column_alignment {
          ColumnAlignment::None | ColumnAlignment::Left => {}
          ColumnAlignment::Center => {
            if difference > 1 {
              items.push_str(&" ".repeat((difference as f32 / 2_f32).floor() as usize))
            }
          }
          ColumnAlignment::Right => {
            items.push_str(&" ".repeat(difference));
          }
        }
      }

      items.extend(cell_items);

      if difference > 0 {
        match column_alignment {
          ColumnAlignment::None | ColumnAlignment::Left => {
            items.push_str(&" ".repeat(difference));
          }
          ColumnAlignment::Center => items.push_str(&" ".repeat((difference as f32 / 2_f32).ceil() as usize)),
          ColumnAlignment::Right => {}
        }
      }

      items.push_str(" |");
    }

    parser_helpers::with_no_new_lines(items)
  }

  fn get_column_widths(header: &Vec<(PrintItems, usize)>, rows: &Vec<Vec<(PrintItems, usize)>>, column_alignments: &Vec<ColumnAlignment>) -> Vec<usize> {
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
        max_width = std::cmp::max(max_width, get_column_alignment_properties(*column_alignment).count() + 1);
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
    let items = parse_table_cell(cell, context);
    get_items_single_line_width(items)
  }
}

fn parse_table_cell(table_cell: &TableCell, context: &mut Context) -> PrintItems {
  parse_nodes(&table_cell.children, context)
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
  items1.push_optional_path(rc_path.clone());
  items2.push_optional_path(rc_path);
  (items1, items2)
}

fn measure_single_line_width(items: PrintItems) -> usize {
  // this doesn't seem ideal, but good enough for now
  get_items_text(items).chars().count()
}

fn get_items_text(items: PrintItems) -> String {
  print(
    parser_helpers::with_no_new_lines(items),
    PrintOptions {
      indent_width: 0,
      max_width: std::u32::MAX,
      use_tabs: false,
      new_line_text: "",
    },
  )
}

fn get_space_or_newline_based_on_config(context: &Context) -> PrintItems {
  match context.configuration.text_wrap {
    TextWrap::Always => Signal::SpaceOrNewLine.into(),
    TextWrap::Never | TextWrap::Maintain => " ".into(),
  }
}

fn get_newline_wrapping_based_on_config(context: &Context) -> PrintItems {
  match context.configuration.text_wrap {
    TextWrap::Always => Signal::SpaceOrNewLine.into(),
    TextWrap::Never => " ".into(),
    TextWrap::Maintain => Signal::NewLine.into(),
  }
}

/// If the list's first items are both 1s
fn is_all_ones_list(list: &List, context: &Context) -> bool {
  list.children.len() > 1 && list.start_index.unwrap_or(0) == 1 && {
    let text = list.children.get(1).unwrap().text(context).trim();
    text.starts_with("1.") || text.starts_with("1)")
  }
}
