//! Block parsing, the first of the parser's two phases.
//!
//! A markdown document is a tree of blocks whose structure is decided entirely
//! by what prefixes each line, so this works line by line: it recognizes which
//! block a line starts, gathers up the lines that belong to it, strips off the
//! prefix the block contributes and recurses on what's left. Leaf blocks hand
//! the lines they gathered to [`super::inline`] to be turned into inline nodes.

use std::borrow::Cow;

use super::ast::*;
use super::inline::parse_inlines;
use super::inline::InlineContext;
use super::inline::InlineText;
use super::links;
use super::source::join_lines;
use super::source::ContentLine;
use super::source::SPACES;

/// The indentation at which a line becomes an indented code block.
const CODE_INDENT: usize = 4;

pub struct BlockParser<'a, 'c> {
  pub source: &'a str,
  pub context: &'c InlineContext<'a>,
}

impl<'a, 'c> BlockParser<'a, 'c> {
  pub fn parse_blocks(&self, lines: &[ContentLine<'a>]) -> Vec<Node<'a>> {
    let mut nodes = Vec::new();
    let mut index = 0;
    while index < lines.len() {
      if lines[index].is_blank() {
        index += 1;
        continue;
      }
      let end = self.parse_block(lines, index, &mut nodes);
      // every block takes at least the line it starts on, and a block that took
      // none would leave this reading the same line forever
      debug_assert!(end > index, "no block was read at line {}", index);
      index = end.max(index + 1);
    }
    nodes
  }

  /// Parses the block starting at `index`, returning the line to continue at.
  fn parse_block(&self, lines: &[ContentLine<'a>], index: usize, nodes: &mut Vec<Node<'a>>) -> usize {
    let line = lines[index];

    // a line the container above took as the lazy continuation of a paragraph
    // can only ever be more of that paragraph
    if line.is_lazy {
      return self.parse_paragraph(lines, index, nodes);
    }
    if line.indent_columns() >= CODE_INDENT {
      return self.parse_indented_code(lines, index, nodes);
    }

    let rest = line.rest();
    match rest.as_bytes().first() {
      Some(b'>') => self.parse_block_quote(lines, index, nodes),
      Some(b'#') if atx_heading_level(rest).is_some() => self.parse_atx_heading(lines, index, nodes),
      Some(b'`') | Some(b'~') if code_fence(rest).is_some() => self.parse_fenced_code(lines, index, nodes),
      Some(b'<') if html_block_kind(rest, false).is_some() => self.parse_html_block(lines, index, nodes),
      Some(b'[') if footnote_definition_name(rest).is_some() => self.parse_footnote_definition(lines, index, nodes),
      Some(_) if is_thematic_break(rest) => {
        nodes.push(
          HorizontalRule {
            span: line.trim_end().span(),
          }
          .into(),
        );
        index + 1
      }
      Some(_) if list_marker(rest).is_some() => self.parse_list(lines, index, nodes),
      _ => self.parse_paragraph(lines, index, nodes),
    }
  }

  // ==== leaf blocks ====

  fn parse_atx_heading(&self, lines: &[ContentLine<'a>], index: usize, nodes: &mut Vec<Node<'a>>) -> usize {
    let line = lines[index].trimmed();
    let level = atx_heading_level(line.text).unwrap();
    let content = line.strip_bytes(level as usize).strip_columns(usize::MAX).trim_end();
    // trailing hashes close the heading when whitespace separates them from
    // its text
    let closing = content.text.trim_end_matches('#');
    let content = if closing.len() != content.text.len() && (closing.is_empty() || closing.ends_with([' ', '\t'])) {
      let mut content = content;
      content.text = closing;
      content.trim_end()
    } else {
      content
    };

    nodes.push(
      Heading {
        span: Span::new(line.start, lines[index].trim_end().end()),
        level,
        style: HeadingStyle::Atx,
        children: self.parse_inlines(&[content]),
      }
      .into(),
    );
    index + 1
  }

  fn parse_fenced_code(&self, lines: &[ContentLine<'a>], index: usize, nodes: &mut Vec<Node<'a>>) -> usize {
    let open_line = lines[index];
    let indent = open_line.indent_columns();
    let fence = code_fence(open_line.rest()).unwrap();
    let info = open_line.rest()[fence.len..].trim_matches(SPACES);

    let mut end = index + 1;
    let mut content_end = index + 1;
    while end < lines.len() {
      if is_closing_fence(lines[end], &fence) {
        content_end = end;
        end += 1;
        break;
      }
      end += 1;
      content_end = end;
    }

    let content: Vec<ContentLine<'a>> = lines[index + 1..content_end]
      .iter()
      .map(|line| line.strip_columns(indent))
      .collect();
    let code = join_lines(&content, self.source, !content.is_empty());

    nodes.push(
      CodeBlock {
        span: Span::new(open_line.rest_start(), lines[end - 1].trim_end().end()),
        fence: Some(CodeFence {
          fence_char: fence.char,
          fence_len: fence.len,
          info: (!info.is_empty()).then_some(info),
        }),
        code,
      }
      .into(),
    );
    end
  }

  fn parse_indented_code(&self, lines: &[ContentLine<'a>], index: usize, nodes: &mut Vec<Node<'a>>) -> usize {
    let mut end = index;
    let mut content_end = index;
    while end < lines.len() {
      let line = lines[end];
      if line.is_blank() {
        end += 1;
        continue;
      }
      if line.indent_columns() < CODE_INDENT {
        break;
      }
      end += 1;
      content_end = end;
    }

    let content: Vec<ContentLine<'a>> = lines[index..content_end]
      .iter()
      .map(|line| line.strip_columns(CODE_INDENT))
      .collect();
    nodes.push(
      CodeBlock {
        span: Span::new(lines[index].rest_start(), lines[content_end - 1].trim_end().end()),
        fence: None,
        code: join_lines(&content, self.source, true),
      }
      .into(),
    );
    content_end
  }

  fn parse_html_block(&self, lines: &[ContentLine<'a>], index: usize, nodes: &mut Vec<Node<'a>>) -> usize {
    let kind = html_block_kind(lines[index].rest(), false).unwrap();
    let mut end = index;
    while end < lines.len() {
      let line = lines[end];
      match &kind {
        HtmlBlockKind::Closes(close) => {
          let contains_close = line.text.contains(close.as_ref());
          end += 1;
          if contains_close {
            break;
          }
        }
        HtmlBlockKind::BlankLine => {
          if line.is_blank() {
            break;
          }
          end += 1;
        }
      }
    }

    let content: Vec<ContentLine<'a>> = lines[index..end].iter().map(|line| line.trim_end()).collect();
    nodes.push(
      Html {
        span: Span::new(lines[index].start, content.last().unwrap().end()),
        text: join_lines(&content, self.source, false),
        is_block: true,
      }
      .into(),
    );
    end
  }

  // ==== container blocks ====

  fn parse_block_quote(&self, lines: &[ContentLine<'a>], index: usize, nodes: &mut Vec<Node<'a>>) -> usize {
    let mut content = Vec::new();
    let mut end = index;
    let mut paragraph = OpenParagraph::default();

    while end < lines.len() {
      let line = lines[end];
      if line.indent_columns() < CODE_INDENT && line.starts_with(">") {
        // a single space after the marker is part of it
        let line = line.trimmed().strip_bytes(1).strip_columns(1);
        paragraph.push(line);
        content.push(line);
        end += 1;
        continue;
      }
      // a paragraph within the quote can continue on a line without a marker
      if self.continues_paragraph(&paragraph, lines, end) {
        let line = line.as_lazy();
        paragraph.push(line);
        content.push(line);
        end += 1;
        continue;
      }
      break;
    }

    let span = Span::new(lines[index].rest_start(), lines[end - 1].trim_end().end());
    nodes.push(
      BlockQuote {
        span,
        children: self.parse_blocks(&content),
      }
      .into(),
    );
    end
  }

  fn parse_list(&self, lines: &[ContentLine<'a>], index: usize, nodes: &mut Vec<Node<'a>>) -> usize {
    let first = list_marker(lines[index].rest()).unwrap();
    let mut items = Vec::new();
    let mut end = index;

    while end < lines.len() {
      let line = lines[end];
      if line.is_blank() {
        // whether a blank line ends the list is decided by the item that
        // follows it, since one may sit between two items of a loose list
        end += 1;
        continue;
      }
      if line.indent_columns() >= CODE_INDENT {
        break;
      }
      let rest = line.rest();
      if is_thematic_break(rest) {
        break;
      }
      let Some(marker) = list_marker(rest) else { break };
      if marker.char != first.char || marker.is_ordered != first.is_ordered {
        break;
      }
      let (item, next) = self.parse_item(lines, end, &marker);
      items.push(item);
      end = next;
    }

    // any blank lines that followed the last item aren't part of the list
    let last_line = items.last().map(|item| item.span.end).unwrap_or(lines[index].end());
    nodes.push(
      List {
        span: Span::new(lines[index].rest_start(), last_line),
        start_index: first.is_ordered.then_some(first.start_index),
        marker_char: first.char,
        children: items.into_iter().map(Into::into).collect(),
      }
      .into(),
    );
    end
  }

  fn parse_item(&self, lines: &[ContentLine<'a>], index: usize, marker: &ListMarker) -> (Item<'a>, usize) {
    let start_line = lines[index].trimmed();
    let marker_span = Span::new(start_line.start, start_line.start + marker.len);
    let after_marker = start_line.strip_bytes(marker.len);

    // the item's content starts after the whitespace that follows its marker,
    // unless there's a lot of it or none of it
    let spaces = after_marker.indent_columns();
    let spaces = if after_marker.is_blank() || spaces > CODE_INDENT {
      1
    } else {
      spaces
    };
    // measured from where the container the item is in starts, which is what
    // its lines have already had stripped off them
    let content_indent = lines[index].indent_columns() + marker.len + spaces;

    let mut content = vec![after_marker.strip_columns(spaces)];
    // an item may begin with at most one blank line, so a second one before
    // any content ends it
    let begins_blank = content[0].is_blank();
    let mut paragraph = OpenParagraph::default();
    paragraph.push(content[0]);
    let mut end = index + 1;
    let mut last_content = end;
    while end < lines.len() {
      let line = lines[end];
      if line.is_blank() {
        if begins_blank && last_content == index + 1 {
          break;
        }
        // a blank line carries the item's indentation as much as any other,
        // and what's left of it is content of whatever block it falls within
        let line = line.strip_columns(content_indent);
        paragraph.push(line);
        content.push(line);
        end += 1;
        continue;
      }
      if line.indent_columns() >= content_indent {
        let line = line.strip_columns(content_indent);
        paragraph.push(line);
        content.push(line);
        end += 1;
        last_content = end;
        continue;
      }
      // another item of the list ends this one, no matter what number it
      // starts at, because the paragraph within it isn't what's being
      // continued at this point. An indented marker is code rather than an
      // item, and a lazy line never carried a marker at all, so neither of
      // those ends anything.
      if !line.is_lazy && line.indent_columns() < CODE_INDENT && list_marker(line.rest()).is_some() {
        break;
      }
      if self.continues_paragraph(&paragraph, lines, end) {
        let line = line.as_lazy();
        paragraph.push(line);
        content.push(line);
        end += 1;
        last_content = end;
        continue;
      }
      break;
    }
    content.truncate(content.len() - (end - last_content));
    let end = last_content;

    let (task_marker, content) = split_task_list_marker(content);
    let mut children = self.parse_blocks(&content);
    remove_indent_beside_marker(&mut children, self.source);
    let sub_lists = split_trailing_lists(&mut children);

    let item = Item {
      span: Span::new(start_line.start, lines[end - 1].trim_end().end()),
      marker_span,
      marker: task_marker,
      children,
      sub_lists,
    };
    (item, end)
  }

  fn parse_footnote_definition(&self, lines: &[ContentLine<'a>], index: usize, nodes: &mut Vec<Node<'a>>) -> usize {
    let start_line = lines[index].trimmed();
    let name = footnote_definition_name(start_line.text).unwrap();
    let marker_len = name.len() + 4; // `[^` + name + `]:`

    let mut content = vec![start_line.strip_bytes(marker_len).strip_columns(usize::MAX)];
    let mut paragraph = OpenParagraph::default();
    paragraph.push(content[0]);
    let mut end = index + 1;
    let mut last_content = end;
    while end < lines.len() {
      let line = lines[end];
      if line.is_blank() {
        let line = line.strip_columns(CODE_INDENT);
        paragraph.push(line);
        content.push(line);
        end += 1;
        continue;
      }
      if line.indent_columns() >= CODE_INDENT {
        let line = line.strip_columns(CODE_INDENT);
        paragraph.push(line);
        content.push(line);
        end += 1;
        last_content = end;
        continue;
      }
      if self.continues_paragraph(&paragraph, lines, end) {
        let line = line.as_lazy();
        paragraph.push(line);
        content.push(line);
        end += 1;
        last_content = end;
        continue;
      }
      break;
    }
    content.truncate(content.len() - (end - last_content));
    let end = last_content;

    let mut children = self.parse_blocks(&content);
    remove_indent_beside_marker(&mut children, self.source);

    nodes.push(
      FootnoteDefinition {
        span: Span::new(start_line.start, lines[end - 1].trim_end().end()),
        name,
        children,
      }
      .into(),
    );
    end
  }

  // ==== paragraphs and what they can turn into ====

  /// Gathers the lines of a paragraph, which may end up being a setext
  /// heading, a table or a definition list instead.
  fn parse_paragraph(&self, lines: &[ContentLine<'a>], index: usize, nodes: &mut Vec<Node<'a>>) -> usize {
    // the definitions a paragraph begins with are blocks of their own, and the
    // rest of it carries on as the paragraph they were taken from -- so what
    // follows them only starts a block of its own if it could have interrupted
    // that paragraph. Nothing is looked at this way until a definition has been
    // taken, since the line this was called with has to be read as a paragraph
    // whatever it looks like -- leaving it for another block to read would
    // leave it for this to be called with again
    let after_definitions = self.push_link_reference_definitions(lines, index, nodes);
    let index = if after_definitions == index {
      index
    } else {
      if after_definitions >= lines.len() || lines[after_definitions].is_blank() {
        return after_definitions;
      }
      if lines[after_definitions].indent_columns() < CODE_INDENT && self.interrupts_paragraph(lines, after_definitions)
      {
        return after_definitions;
      }
      after_definitions
    };

    let mut content = vec![lines[index]];
    let mut end = index + 1;

    while end < lines.len() {
      let line = lines[end];
      if line.is_blank() {
        break;
      }
      let rest = line.rest();

      // a line without the prefix of the container it's in can only be more of
      // the paragraph, never the start of one of these
      if line.indent_columns() < CODE_INDENT && !line.is_lazy {
        if let Some(level) = setext_heading_level(rest) {
          nodes.push(
            Heading {
              span: Span::new(content[0].rest_start(), line.trim_end().end()),
              level,
              style: HeadingStyle::Setext,
              children: self.parse_inlines(&content),
            }
            .into(),
          );
          return end + 1;
        }
        // a delimiter row turns the line above it into a table's header,
        // leaving the rest of the paragraph above the table
        if let Some(alignment) = table_delimiter_row(rest, content.last().unwrap().rest()) {
          self.push_paragraph(&content[..content.len() - 1], nodes);
          return self.parse_table(lines, end - 1, end, alignment, nodes);
        }
        if is_definition_marker(rest) {
          return self.parse_definition_list(lines, index, end, content, nodes);
        }
      }
      if !line.is_lazy && line.indent_columns() < CODE_INDENT && self.interrupts_paragraph(lines, end) {
        break;
      }

      content.push(line);
      end += 1;
    }

    // a blank line may separate the terms of a definition list from their
    // definitions
    let after_blanks = skip_blank_lines(lines, end);
    if after_blanks < lines.len()
      && lines[after_blanks].indent_columns() < CODE_INDENT
      && is_definition_marker(lines[after_blanks].rest())
    {
      return self.parse_definition_list(lines, index, after_blanks, content, nodes);
    }

    self.push_paragraph(&content, nodes);
    end
  }

  fn push_paragraph(&self, content: &[ContentLine<'a>], nodes: &mut Vec<Node<'a>>) {
    if content.is_empty() {
      return;
    }
    nodes.push(
      Paragraph {
        span: Span::new(content[0].rest_start(), content.last().unwrap().trim_end().end()),
        children: self.parse_inlines(content),
        marker: None,
      }
      .into(),
    );
  }

  /// Pushes the link reference definitions that begin at `index`, returning
  /// the line that follows them.
  ///
  /// They can only appear where a paragraph would begin, so this looks no
  /// further than the blank line that ends one.
  fn push_link_reference_definitions(
    &self,
    lines: &[ContentLine<'a>],
    index: usize,
    nodes: &mut Vec<Node<'a>>,
  ) -> usize {
    let mut block_end = index + 1;
    while block_end < lines.len()
      && !lines[block_end].is_blank()
      && !(!lines[block_end].is_lazy
        && lines[block_end].indent_columns() < CODE_INDENT
        && self.interrupts_paragraph(lines, block_end))
    {
      block_end += 1;
    }
    let content = &lines[index..block_end];
    if !content.iter().any(|line| line.text.contains("]:")) {
      return index;
    }

    let text = InlineText::new(content, self.source);
    let mut pos = 0;
    let mut found = 0;
    while let Some((end, definition)) = links::match_link_reference_definition(&text, pos) {
      nodes.push(definition.into());
      found += text.str_between(pos, end).matches('\n').count() + 1;
      pos = end + 1; // move past the line's newline
      if pos >= text.len() {
        break;
      }
    }
    index + found.min(content.len())
  }

  fn parse_table(
    &self,
    lines: &[ContentLine<'a>],
    index: usize,
    delimiter_index: usize,
    column_alignment: Vec<ColumnAlignment>,
    nodes: &mut Vec<Node<'a>>,
  ) -> usize {
    let header = TableHead {
      span: lines[index].trim_end().span(),
      cells: self.parse_table_cells(lines[index]),
    };

    let mut rows = Vec::new();
    let mut end = delimiter_index + 1;
    while end < lines.len() {
      let line = lines[end];
      if line.is_blank() || line.is_lazy || line.indent_columns() >= CODE_INDENT {
        break;
      }
      if self.interrupts_paragraph(lines, end) {
        break;
      }
      // a row holds every cell it was written with, even the ones past the
      // last column -- those aren't rendered, but they are still text of the
      // file, and a row short of cells is filled out to the last column
      let mut cells = self.parse_table_cells(line);
      let row_end = line.trim_end().end();
      while cells.len() < column_alignment.len() {
        cells.push(TableCell {
          span: Span::new(row_end, row_end),
          children: Vec::new(),
        });
      }
      rows.push(TableRow {
        span: line.trim_end().span(),
        cells,
      });
      end += 1;
    }

    nodes.push(
      Table {
        span: Span::new(lines[index].rest_start(), lines[end - 1].trim_end().end()),
        header,
        column_alignment,
        rows,
      }
      .into(),
    );
    end
  }

  fn parse_table_cells(&self, line: ContentLine<'a>) -> Vec<TableCell<'a>> {
    let mut cells = Vec::new();
    for (start, end) in table_cell_ranges(line.rest()) {
      let mut cell = line.trimmed();
      cell = cell.strip_bytes(start);
      cell.text = &cell.text[..end - start];
      let cell = cell.strip_columns(usize::MAX).trim_end();
      cells.push(TableCell {
        span: cell.span(),
        children: self.parse_inlines(&[cell]),
      });
    }
    cells
  }

  /// Parses the definition list whose terms are the lines gathered so far and
  /// whose first definition marker is at `marker_index`.
  fn parse_definition_list(
    &self,
    lines: &[ContentLine<'a>],
    index: usize,
    marker_index: usize,
    titles: Vec<ContentLine<'a>>,
    nodes: &mut Vec<Node<'a>>,
  ) -> usize {
    let mut children: Vec<Node<'a>> = Vec::new();
    let mut end = marker_index;
    let mut titles = titles;

    loop {
      for title in &titles {
        let title = title.trimmed().trim_end();
        children.push(
          DefinitionListTitle {
            span: title.span(),
            children: self.parse_inlines(&[title]),
          }
          .into(),
        );
      }

      // the definitions that follow the terms
      while end < lines.len() {
        let next = skip_blank_lines(lines, end);
        if next >= lines.len() || !is_definition_marker(lines[next].rest()) {
          break;
        }
        end = self.parse_definition(lines, next, &mut children);
      }

      // another group of terms continues the same list
      let next = skip_blank_lines(lines, end);
      let Some(group_end) = self.definition_group_end(lines, next) else {
        break;
      };
      titles = lines[next..group_end].to_vec();
      end = group_end;
    }

    nodes.push(
      DefinitionList {
        span: Span::new(lines[index].rest_start(), children.last().unwrap().span().end),
        children,
      }
      .into(),
    );
    end
  }

  fn parse_definition(&self, lines: &[ContentLine<'a>], index: usize, children: &mut Vec<Node<'a>>) -> usize {
    let start_line = lines[index].trimmed();
    let after_marker = start_line.strip_bytes(1);
    let spaces = after_marker.indent_columns();
    let spaces = if after_marker.is_blank() || spaces > CODE_INDENT {
      1
    } else {
      spaces
    };
    let content_indent = lines[index].indent_columns() + 1 + spaces;

    let mut content = vec![after_marker.strip_columns(spaces)];
    let mut paragraph = OpenParagraph::default();
    paragraph.push(content[0]);
    let mut end = index + 1;
    let mut last_content = end;
    while end < lines.len() {
      let line = lines[end];
      if line.is_blank() {
        let line = line.strip_columns(content_indent);
        paragraph.push(line);
        content.push(line);
        end += 1;
        continue;
      }
      if !line.is_lazy && is_definition_marker(line.rest()) {
        break;
      }
      if line.indent_columns() >= content_indent {
        let line = line.strip_columns(content_indent);
        paragraph.push(line);
        content.push(line);
        end += 1;
        last_content = end;
        continue;
      }
      if self.continues_paragraph(&paragraph, lines, end) {
        let line = line.as_lazy();
        paragraph.push(line);
        content.push(line);
        end += 1;
        last_content = end;
        continue;
      }
      break;
    }
    content.truncate(content.len() - (end - last_content));
    let end = last_content;

    let mut definition_children = self.parse_blocks(&content);
    remove_indent_beside_marker(&mut definition_children, self.source);
    children.push(
      DefinitionListDefinition {
        span: Span::new(start_line.start, lines[end - 1].trim_end().end()),
        children: definition_children,
      }
      .into(),
    );
    end
  }

  /// The line the group of terms starting at `index` ends at, when the lines
  /// there are terms of a definition list.
  fn definition_group_end(&self, lines: &[ContentLine<'a>], index: usize) -> Option<usize> {
    let mut end = index;
    while end < lines.len() {
      let line = lines[end];
      if line.is_blank() || line.indent_columns() >= CODE_INDENT {
        return None;
      }
      if is_definition_marker(line.rest()) {
        return (end > index).then_some(end);
      }
      if !line.is_lazy && self.interrupts_paragraph(lines, end) {
        return None;
      }
      end += 1;
    }
    None
  }

  // ==== helpers ====

  fn parse_inlines(&self, content: &[ContentLine<'a>]) -> Vec<Node<'a>> {
    parse_inlines(content, self.context)
  }

  /// Whether the line starts a block that can appear in the middle of a
  /// paragraph, ending it.
  fn interrupts_paragraph(&self, lines: &[ContentLine<'a>], index: usize) -> bool {
    let line = lines[index];
    if line.indent_columns() >= CODE_INDENT {
      return false; // indented code can't interrupt a paragraph
    }
    let rest = line.rest();
    match rest.as_bytes().first() {
      None => true,
      Some(b'>') => true,
      Some(b'#') => atx_heading_level(rest).is_some(),
      Some(b'`') | Some(b'~') => code_fence(rest).is_some(),
      Some(b'<') => html_block_kind(rest, true).is_some(),
      Some(b'[') => footnote_definition_name(rest).is_some(),
      Some(_) if is_thematic_break(rest) => true,
      // a list item only interrupts a paragraph when it isn't empty, and an
      // ordered one has to start at 1
      Some(_) => match list_marker(rest) {
        Some(marker) => {
          !rest[marker.len..].trim_matches(SPACES).is_empty() && (!marker.is_ordered || marker.start_index == 1)
        }
        None => false,
      },
    }
  }

  /// Whether the line continues the paragraph the container's content ends
  /// with, which is the only thing a line missing the container's prefix can
  /// do.
  fn continues_paragraph(&self, paragraph: &OpenParagraph<'a>, lines: &[ContentLine<'a>], index: usize) -> bool {
    paragraph.is_open() && !lines[index].is_blank() && !self.starts_block_beside_container(lines, index)
  }

  /// Whether the line starts a block where the prefix of the container above it
  /// is missing.
  ///
  /// The block starts beside that container rather than within its paragraph,
  /// so the rules that only hold for interrupting a paragraph (an empty list
  /// item, an ordered list that doesn't begin at 1) don't apply here.
  fn starts_block_beside_container(&self, lines: &[ContentLine<'a>], index: usize) -> bool {
    let line = lines[index];
    if line.indent_columns() >= CODE_INDENT {
      return false; // indented code continues a paragraph rather than starting
    }
    let rest = line.rest();
    match rest.as_bytes().first() {
      Some(_) if list_marker(rest).is_some() => true,
      _ => self.interrupts_paragraph(lines, index),
    }
  }
}

/// Whether the text would begin a block if it were at the start of a line
/// within a paragraph, which is what stops the formatter from moving it there.
pub fn starts_block_in_paragraph(text: &str) -> bool {
  let text = text.trim_start_matches([' ', '\t']);
  match text.as_bytes().first() {
    None => false,
    Some(b'>') => true,
    Some(b'#') => atx_heading_level(text).is_some(),
    Some(b'`') | Some(b'~') => code_fence(text).is_some(),
    Some(b'<') => html_block_kind(text, true).is_some(),
    Some(b'[') => footnote_definition_name(text).is_some(),
    // an underline turns the line above it into a heading rather than
    // interrupting it, but either way the text can't be moved to a line start
    Some(b'=') | Some(b'-') if setext_heading_level(text).is_some() => true,
    Some(_) if is_thematic_break(text) => true,
    // a row of dashes and pipes turns the line above it into a table's header
    Some(_) if is_table_delimiter_shape(text) => true,
    // a definition marker turns the line above it into a term
    Some(b':') if is_definition_marker(text) => true,
    Some(_) => match list_marker(text) {
      Some(marker) => {
        !text[marker.len..].trim_matches(SPACES).is_empty() && (!marker.is_ordered || marker.start_index == 1)
      }
      None => false,
    },
  }
}

/// Whether the text is made of the characters a table's delimiter row is, which
/// is what it takes for the line above it to become a header.
fn is_table_delimiter_shape(text: &str) -> bool {
  text.contains('-') && has_unescaped_pipe(text) && text.bytes().all(|b| matches!(b, b'-' | b':' | b'|' | b' ' | b'\t'))
}

// ==== block recognition ====

fn atx_heading_level(text: &str) -> Option<u8> {
  let level = text.bytes().take_while(|b| *b == b'#').count();
  let is_heading = (1..=6).contains(&level) && matches!(text.as_bytes().get(level), None | Some(b' ') | Some(b'\t'));
  is_heading.then_some(level as u8)
}

fn setext_heading_level(text: &str) -> Option<u8> {
  let text = text.trim_end_matches(SPACES);
  let first = text.as_bytes().first()?;
  if !matches!(first, b'=' | b'-') || !text.bytes().all(|b| b == *first) {
    return None;
  }
  Some(if *first == b'=' { 1 } else { 2 })
}

fn is_thematic_break(text: &str) -> bool {
  let Some(first) = text.as_bytes().first() else {
    return false;
  };
  if !matches!(first, b'-' | b'_' | b'*') {
    return false;
  }
  let mut count = 0;
  for byte in text.bytes() {
    match byte {
      b' ' | b'\t' => continue,
      byte if byte == *first => count += 1,
      _ => return false,
    }
  }
  count >= 3
}

/// Tracks whether the content gathered for a container ends in the middle of a
/// paragraph, which is the only thing a line that's missing the container's
/// prefix can continue.
#[derive(Default)]
struct OpenParagraph<'a> {
  fence: Option<CodeFenceInfo>,
  is_open: bool,
  /// The single line the open paragraph holds so far, which a delimiter row
  /// would turn into a table's header.
  header: Option<ContentLine<'a>>,
  /// The container the content ends within, whose own paragraph is the one a
  /// lazy line would reach.
  inner: Option<(Container, Box<OpenParagraph<'a>>)>,
}

impl<'a> OpenParagraph<'a> {
  fn push(&mut self, line: ContentLine<'a>) {
    // a code block runs until its fence closes, whatever is in between
    if let Some(fence) = &self.fence {
      if is_closing_fence(line, fence) {
        self.fence = None;
      }
      return;
    }
    if line.is_blank() {
      self.is_open = false;
      self.inner = None;
      return;
    }
    // a line taken as a lazy continuation is more of the paragraph that was
    // already open, whatever else it may look like
    if line.is_lazy {
      if let Some((_, inner)) = &mut self.inner {
        inner.push(line);
      }
      return;
    }
    // a line that keeps the prefix of the container the content is within
    // belongs to that container, so it decides what's open
    if let Some((container, inner)) = &mut self.inner {
      if let Some(content) = container.strip(line) {
        inner.push(content);
        self.is_open = inner.is_open();
        return;
      }
    }

    if self.is_open {
      // a paragraph that turns out to be a table's header stops being one
      if let Some(header) = self.header.take() {
        if line.indent_columns() < CODE_INDENT && table_delimiter_row(line.rest(), header.rest()).is_some() {
          self.is_open = false;
          return;
        }
      }
      // an indented line, or one that starts no block of its own, is more of
      // the paragraph
      if line.indent_columns() >= CODE_INDENT || !starts_block_in_paragraph(line.rest()) {
        return;
      }
      self.is_open = false;
      // an underline turns the paragraph above it into a heading, which holds
      // no paragraph for anything to continue
      if setext_heading_level(line.rest()).is_some() {
        self.inner = None;
        return;
      }
    }

    self.inner = None;
    let rest = line.rest();
    if line.indent_columns() >= CODE_INDENT {
      return; // indented code
    }
    if let Some(fence) = code_fence(rest) {
      self.fence = Some(fence);
      return;
    }
    // the blocks that hold no paragraph of their own end on their own line
    if atx_heading_level(rest).is_some() || is_thematic_break(rest) || html_block_kind(rest, false).is_some() {
      return;
    }
    // what's open within a container is what a lazy line would continue,
    // because laziness reaches through however many containers are in between
    if let Some((container, content)) = Container::open(line) {
      let mut inner = OpenParagraph::default();
      inner.push(content);
      self.is_open = inner.is_open();
      self.inner = Some((container, Box::new(inner)));
      return;
    }
    self.is_open = true;
    self.header = Some(line);
  }

  fn is_open(&self) -> bool {
    self.is_open
  }
}

/// A container a tracked line opened, which says how to reach the content of
/// the lines that follow it.
enum Container {
  BlockQuote,
  /// A list item, whose content is indented this far.
  Item(usize),
}

impl Container {
  /// The container a line opens, along with the content it holds on that line.
  fn open<'a>(line: ContentLine<'a>) -> Option<(Container, ContentLine<'a>)> {
    let rest = line.rest();
    if rest.starts_with('>') {
      // a single space after the marker is part of it
      return Some((Container::BlockQuote, line.trimmed().strip_bytes(1).strip_columns(1)));
    }
    let marker = list_marker(rest)?;
    let after_marker = line.trimmed().strip_bytes(marker.len);
    let spaces = after_marker.indent_columns();
    let spaces = if after_marker.is_blank() || spaces > CODE_INDENT {
      1
    } else {
      spaces
    };
    let indent = line.indent_columns() + marker.len + spaces;
    Some((Container::Item(indent), after_marker.strip_columns(spaces)))
  }

  /// The content of a line that carries this container's prefix.
  fn strip<'a>(&self, line: ContentLine<'a>) -> Option<ContentLine<'a>> {
    match self {
      Container::BlockQuote => (line.indent_columns() < CODE_INDENT && line.starts_with(">"))
        .then(|| line.trimmed().strip_bytes(1).strip_columns(1)),
      Container::Item(indent) => (line.indent_columns() >= *indent).then(|| line.strip_columns(*indent)),
    }
  }
}

struct CodeFenceInfo {
  char: char,
  len: usize,
}

fn code_fence(text: &str) -> Option<CodeFenceInfo> {
  let first = *text.as_bytes().first()?;
  if !matches!(first, b'`' | b'~') {
    return None;
  }
  let len = text.bytes().take_while(|b| *b == first).count();
  if len < 3 {
    return None;
  }
  // an info string can't contain a backtick when the fence is backticks
  if first == b'`' && text[len..].contains('`') {
    return None;
  }
  Some(CodeFenceInfo {
    char: first as char,
    len,
  })
}

fn is_closing_fence(line: ContentLine<'_>, fence: &CodeFenceInfo) -> bool {
  if line.indent_columns() >= CODE_INDENT {
    return false;
  }
  let rest = line.rest();
  let len = rest.bytes().take_while(|b| *b as char == fence.char).count();
  len >= fence.len && rest[len..].trim_matches(SPACES).is_empty()
}

struct ListMarker {
  /// The marker character (ex. `-` for a bullet or `.` for `1.`).
  char: char,
  /// How many bytes the marker takes up.
  len: usize,
  is_ordered: bool,
  start_index: u64,
}

fn list_marker(text: &str) -> Option<ListMarker> {
  let bytes = text.as_bytes();
  let first = *bytes.first()?;

  if matches!(first, b'-' | b'+' | b'*') {
    return followed_by_space(bytes, 1).then_some(ListMarker {
      char: first as char,
      len: 1,
      is_ordered: false,
      start_index: 0,
    });
  }

  if !first.is_ascii_digit() {
    return None;
  }
  let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
  if digits > 9 {
    return None;
  }
  let delimiter = *bytes.get(digits)?;
  if !matches!(delimiter, b'.' | b')') || !followed_by_space(bytes, digits + 1) {
    return None;
  }
  Some(ListMarker {
    char: delimiter as char,
    len: digits + 1,
    is_ordered: true,
    start_index: text[..digits].parse().ok()?,
  })
}

fn followed_by_space(bytes: &[u8], index: usize) -> bool {
  matches!(bytes.get(index), None | Some(b' ') | Some(b'\t'))
}

/// Whether the text is a `:` marker followed by whitespace or nothing, which
/// starts the definition of the term above it.
fn is_definition_marker(text: &str) -> bool {
  text.starts_with(':') && matches!(text.as_bytes().get(1), None | Some(b' ') | Some(b'\t'))
}

fn footnote_definition_name(text: &str) -> Option<&str> {
  let rest = text.strip_prefix("[^")?;
  let end = rest.find(']')?;
  let name = &rest[..end];
  if name.is_empty() || !rest[end + 1..].starts_with(':') {
    return None;
  }
  Some(name)
}

/// What ends an html block.
enum HtmlBlockKind {
  /// The line containing the given text.
  Closes(Cow<'static, str>),
  /// The next blank line.
  BlankLine,
}

/// The html block the text starts, if any. Blocks that consist of a single
/// complete tag can't interrupt a paragraph.
fn html_block_kind(text: &str, interrupting: bool) -> Option<HtmlBlockKind> {
  let rest = text.strip_prefix('<')?;

  for (name, close) in [
    ("script", "</script>"),
    ("pre", "</pre>"),
    ("style", "</style>"),
    ("textarea", "</textarea>"),
  ] {
    if starts_with_tag_name(rest, name) {
      return Some(HtmlBlockKind::Closes(Cow::Borrowed(close)));
    }
  }
  if rest.starts_with("!--") {
    return Some(HtmlBlockKind::Closes(Cow::Borrowed("-->")));
  }
  if rest.starts_with('?') {
    return Some(HtmlBlockKind::Closes(Cow::Borrowed("?>")));
  }
  if rest.starts_with("![CDATA[") {
    return Some(HtmlBlockKind::Closes(Cow::Borrowed("]]>")));
  }
  // a declaration is `<!` followed by an ascii letter
  if rest.starts_with('!') && rest[1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
    return Some(HtmlBlockKind::Closes(Cow::Borrowed(">")));
  }

  let name_start = rest.strip_prefix('/').unwrap_or(rest);
  let name_len = name_start.bytes().take_while(u8::is_ascii_alphanumeric).count();
  if HTML_BLOCK_TAGS
    .binary_search_by(|tag| compare_tag_name(tag, &name_start[..name_len]))
    .is_ok()
    && followed_by_tag_end(&name_start[name_len..])
  {
    return Some(HtmlBlockKind::BlankLine);
  }

  // a line that is nothing but one complete tag
  if interrupting {
    return None;
  }
  let line = InlineText::new(
    &[ContentLine {
      start: 0,
      text,
      virtual_spaces: 0,
      column: 0,
      is_lazy: false,
    }],
    text,
  );
  let end = links::match_html_tag(&line, 0)?;
  line
    .str_between(end, line.len())
    .trim_matches(SPACES)
    .is_empty()
    .then_some(HtmlBlockKind::BlankLine)
}

/// Whether the text starts with the tag name, which is matched without regard
/// to case and has to end where a tag's name can.
fn starts_with_tag_name(text: &str, name: &str) -> bool {
  // compared as bytes because the name's length may land within a character
  let bytes = text.as_bytes();
  bytes.len() >= name.len()
    && bytes[..name.len()].eq_ignore_ascii_case(name.as_bytes())
    && followed_by_tag_end(&text[name.len()..])
}

fn compare_tag_name(tag: &str, name: &str) -> std::cmp::Ordering {
  tag
    .bytes()
    .map(|b| b.to_ascii_lowercase())
    .cmp(name.bytes().map(|b| b.to_ascii_lowercase()))
}

fn followed_by_tag_end(text: &str) -> bool {
  matches!(text.as_bytes().first(), None | Some(b' ') | Some(b'\t') | Some(b'>')) || text.starts_with("/>")
}

/// The tag names that start an html block, in the order `binary_search` needs.
const HTML_BLOCK_TAGS: [&str; 62] = [
  "address",
  "article",
  "aside",
  "base",
  "basefont",
  "blockquote",
  "body",
  "caption",
  "center",
  "col",
  "colgroup",
  "dd",
  "details",
  "dialog",
  "dir",
  "div",
  "dl",
  "dt",
  "fieldset",
  "figcaption",
  "figure",
  "footer",
  "form",
  "frame",
  "frameset",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "head",
  "header",
  "hr",
  "html",
  "iframe",
  "legend",
  "li",
  "link",
  "main",
  "menu",
  "menuitem",
  "nav",
  "noframes",
  "ol",
  "optgroup",
  "option",
  "p",
  "param",
  "search",
  "section",
  "summary",
  "table",
  "tbody",
  "td",
  "tfoot",
  "th",
  "thead",
  "title",
  "tr",
  "track",
  "ul",
];

// ==== tables ====

/// The ranges of the cells of a table row, not including its `|` separators.
///
/// Only a backslash keeps a `|` from separating cells, so one within a code
/// span separates them just the same.
fn table_cell_ranges(text: &str) -> Vec<(usize, usize)> {
  let bytes = text.as_bytes();
  let mut ranges = Vec::new();
  let mut start = if bytes.first() == Some(&b'|') { 1 } else { 0 };
  let mut index = start;

  while index < bytes.len() {
    match bytes[index] {
      b'\\' => index += 2,
      b'|' => {
        ranges.push((start, index));
        start = index + 1;
        index = start;
      }
      _ => index += 1,
    }
  }

  let trailing = text[start.min(text.len())..].trim_matches(SPACES);
  if !trailing.is_empty() {
    ranges.push((start, text.len()));
  }
  ranges
}

/// Whether the text holds a `|` that separates cells rather than one written
/// as part of a cell's content.
fn has_unescaped_pipe(text: &str) -> bool {
  let mut bytes = text.bytes();
  while let Some(byte) = bytes.next() {
    match byte {
      b'\\' => {
        bytes.next();
      }
      b'|' => return true,
      _ => {}
    }
  }
  false
}

/// The column alignments of a table's delimiter row, when the text is one and
/// it has as many columns as the header line above it.
fn table_delimiter_row(text: &str, header: &str) -> Option<Vec<ColumnAlignment>> {
  // the cheap checks come first because this runs on the second line of every
  // paragraph, while a table's delimiter row is rare. A row with no `|` at all
  // would make a table out of any two lines of prose.
  if !text.contains('-') || !has_unescaped_pipe(text) || !has_unescaped_pipe(header) {
    return None;
  }
  let cells = table_cell_ranges(text);
  if cells.is_empty() || cells.len() != table_cell_ranges(header).len() {
    return None;
  }

  let mut alignments = Vec::with_capacity(cells.len());
  for (start, end) in cells {
    let cell = text[start..end].trim_matches(SPACES);
    let left = cell.starts_with(':');
    let right = cell.ends_with(':');
    let dashes = cell.trim_matches(':');
    if dashes.is_empty() || !dashes.bytes().all(|b| b == b'-') {
      return None;
    }
    alignments.push(match (left, right) {
      (true, true) => ColumnAlignment::Center,
      (true, false) => ColumnAlignment::Left,
      (false, true) => ColumnAlignment::Right,
      (false, false) => ColumnAlignment::None,
    });
  }
  Some(alignments)
}

// ==== small helpers ====

fn skip_blank_lines(lines: &[ContentLine<'_>], index: usize) -> usize {
  let mut index = index;
  while index < lines.len() && lines[index].is_blank() {
    index += 1;
  }
  index
}

/// Splits a `[ ]` or `[x]` task list marker off the start of an item's content.
fn split_task_list_marker<'a>(content: Vec<ContentLine<'a>>) -> (Option<TaskListMarker>, Vec<ContentLine<'a>>) {
  let Some(first) = content.first() else {
    return (None, content);
  };
  if first.indent_columns() >= CODE_INDENT {
    return (None, content); // the line is code, whatever it holds
  }
  let trimmed = first.trimmed();
  let is_checked = match trimmed.text.as_bytes() {
    [b'[', b' ', b']', rest @ ..] if rest.first().is_none_or(|b| matches!(b, b' ' | b'\t')) => false,
    [b'[', b'x' | b'X', b']', rest @ ..] if rest.first().is_none_or(|b| matches!(b, b' ' | b'\t')) => true,
    _ => return (None, content),
  };

  let marker = TaskListMarker {
    span: Span::new(trimmed.start, trimmed.start + 3),
    is_checked,
  };
  let mut content = content;
  content[0] = trimmed.strip_bytes(3).strip_columns(1);
  (Some(marker), content)
}

/// A container's first child is written out beside its marker (ex. `- ` or
/// `: `), where the indentation an html block keeps for its first line stops
/// being indentation, so discard it in that case.
fn remove_indent_beside_marker(children: &mut [Node<'_>], source: &str) {
  let Some(Node::Html(html)) = children.first_mut() else {
    return;
  };
  let indent = {
    let text = html.span.text(source);
    text.len() - text.trim_start_matches(' ').len()
  };
  if indent == 0 {
    return;
  }
  html.span.start += indent;
  html.text = match std::mem::take(&mut html.text) {
    Cow::Borrowed(text) => Cow::Borrowed(text.trim_start_matches(' ')),
    Cow::Owned(text) => Cow::Owned(text.trim_start_matches(' ').to_string()),
  };
}

/// Takes the lists at the end of a list item's children, which the formatter
/// keeps beside them rather than indenting further.
fn split_trailing_lists<'a>(children: &mut Vec<Node<'a>>) -> Vec<Node<'a>> {
  let first_trailing = children
    .iter()
    .rposition(|node| !matches!(node, Node::List(_)))
    .map(|index| index + 1)
    .unwrap_or(0);
  children.split_off(first_trailing)
}
