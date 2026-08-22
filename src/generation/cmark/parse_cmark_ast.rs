use super::parsing::parse_image as parse_image_from_text;
use super::parsing::parse_link_reference;
use super::parsing::parse_link_reference_definitions;
use crate::generation::common::*;
use crate::generation::trim_document_whitespace;
use crate::generation::trim_spaces_and_newlines;
use crate::generation::trim_start_spaces_and_newlines;
use pulldown_cmark::*;

struct EventIterator<'a> {
  iterator: OffsetIter<'a, DefaultBrokenLinkCallback>,
  file_text: &'a str,
  last_range: Range,
  next: Option<(Event<'a>, Range)>,
  allow_empty_text_events: bool,
  in_table_count: usize,
  in_block_quote_count: usize,
}

impl<'a> EventIterator<'a> {
  pub fn new(file_text: &'a str, iterator: OffsetIter<'a, DefaultBrokenLinkCallback>) -> EventIterator<'a> {
    let mut event_iterator = EventIterator {
      file_text,
      iterator,
      last_range: Range { start: 0, end: 0 },
      next: None,
      allow_empty_text_events: false,
      in_table_count: 0,
      in_block_quote_count: 0,
    };
    // get the first event through the same code path as the rest
    // so that the nesting counts are kept up to date
    event_iterator.next = event_iterator.move_iterator_next();
    event_iterator
  }

  pub fn next(&mut self) -> Option<Event<'a>> {
    if let Some((event, range)) = self.next.take() {
      // eprintln!("{:?} {:?}", range, event);
      self.last_range = range;
      self.next = self.move_iterator_next();

      if !self.allow_empty_text_events {
        // skip over any empty text or html events
        while let Some((Event::Text(_), range)) | Some((Event::Html(_), range)) = &self.next {
          if trim_document_whitespace(&self.file_text[range.start..range.end]).is_empty() {
            self.next = self.move_iterator_next();
          } else {
            break;
          }
        }
      }

      Some(event)
    } else {
      None
    }
  }

  fn move_iterator_next(&mut self) -> Option<(Event<'a>, Range)> {
    let next = self.iterator.next();
    // eprintln!("Raw event: {:?}", next);

    match next {
      Some((Event::Start(Tag::Table(_)), _)) => self.in_table_count += 1,
      Some((Event::End(TagEnd::Table), _)) => self.in_table_count = self.in_table_count.saturating_sub(1),
      Some((Event::Start(Tag::BlockQuote(_)), _)) => self.in_block_quote_count += 1,
      Some((Event::End(TagEnd::BlockQuote(_)), _)) => {
        self.in_block_quote_count = self.in_block_quote_count.saturating_sub(1)
      }
      _ => {}
    }

    next
  }

  pub fn in_table_count(&self) -> usize {
    self.in_table_count
  }

  pub fn in_block_quote_count(&self) -> usize {
    self.in_block_quote_count
  }

  pub fn start(&self) -> usize {
    self.last_range.start
  }

  pub fn get_range_for_start(&self, start: usize) -> Range {
    Range {
      start,
      end: self.last_range.end,
    }
  }

  pub fn get_last_range(&self) -> Range {
    self.last_range.clone()
  }

  #[allow(dead_code)]
  pub fn peek(&self) -> &Option<(Event<'_>, Range)> {
    &self.next
  }

  pub fn get_not_implemented(&self) -> Node {
    NotImplemented {
      range: self.last_range.clone(),
    }
    .into()
  }
}

pub fn parse_cmark_ast(markdown_text: &str) -> Result<SourceFile, ParseError> {
  let mut options = Options::empty();
  options.insert(Options::ENABLE_TABLES);
  options.insert(Options::ENABLE_FOOTNOTES);
  options.insert(Options::ENABLE_STRIKETHROUGH);
  options.insert(Options::ENABLE_TASKLISTS);
  options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
  options.insert(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS);
  options.insert(Options::ENABLE_MATH);
  if should_enable_definition_lists(markdown_text, options) {
    options.insert(Options::ENABLE_DEFINITION_LIST);
  }

  let mut children: Vec<Node> = Vec::new();
  let mut iterator = EventIterator::new(
    markdown_text,
    Parser::new_ext(markdown_text, options).into_offset_iter(),
  );
  let mut last_event_range: Option<Range> = None;

  while let Some(event) = iterator.next() {
    let current_range = iterator.get_last_range();

    // do not parse for link references while inside a table
    if iterator.in_table_count() <= 1 {
      // `or(Some(0))` so the text before the first event is looked at too
      if let Some(references) = parse_references(
        last_event_range.as_ref().map(|r| r.end).or(Some(0)),
        current_range.start,
        &mut iterator,
      )? {
        children.push(references);
      }
    }

    let node = parse_event(event, &mut iterator)?;
    // cmark says a definition list ends at the end of the block that follows
    // it, so use the corrected range of the node in that case
    last_event_range = Some(match &node {
      Node::DefinitionList(list) => list.range.clone(),
      _ => current_range,
    });
    children.push(node);
  }

  if let Some(references) = parse_references(
    last_event_range.as_ref().map(|r| r.end).or(Some(0)),
    markdown_text.len(),
    &mut iterator,
  )? {
    children.push(references);
  }

  Ok(SourceFile {
    children,
    range: iterator.get_range_for_start(0),
  })
}

/// Whether it's safe to have cmark parse definition lists for this file.
///
/// cmark treats a `:` at the start of a line as a definition marker even when
/// no whitespace follows it, while other markdown parsers require it (ex. a
/// `:::note` admonition or a `:smile:` emoji shortcode is a regular paragraph
/// to them). Formatting those as a definition list would change what the text
/// means, so give up on definition lists for the entire file when one shows up.
fn should_enable_definition_lists(markdown_text: &str, options: Options) -> bool {
  // quick check in order to not parse the text an additional time for most files
  if !markdown_text.lines().any(|line| line.trim_start().starts_with(':')) {
    return false;
  }

  Parser::new_ext(markdown_text, options | Options::ENABLE_DEFINITION_LIST)
    .into_offset_iter()
    .all(|(event, range)| match event {
      Event::Start(Tag::DefinitionListDefinition) => is_definition_marker(&markdown_text[range]),
      _ => true,
    })
}

/// Whether the text starts with a `:` marker that's followed by whitespace or
/// the end of the line (an empty definition). Note that the marker may be
/// indented, so it isn't necessarily the first character.
fn is_definition_marker(text: &str) -> bool {
  let mut chars = text.chars().skip_while(|c| *c == ' ' || *c == '\t');
  matches!(
    (chars.next(), chars.next()),
    (Some(':'), Some(' ') | Some('\t') | Some('\n') | Some('\r') | None)
  )
}

fn parse_references(
  last_event_end: Option<usize>,
  end: usize,
  iterator: &mut EventIterator,
) -> Result<Option<Node>, ParseError> {
  if let Some(last_event_end) = last_event_end {
    if last_event_end < end {
      let references = parse_link_reference_definitions(last_event_end, &iterator.file_text[last_event_end..end])?;
      if !references.is_empty() {
        return Ok(Some(
          Paragraph {
            range: Range {
              start: references.first().unwrap().range.start,
              end: references.last().unwrap().range.end,
            },
            children: references.into_iter().map(|x| x.into()).collect(),
            marker: None,
          }
          .into(),
        ));
      }
    }
  }
  Ok(None)
}

fn parse_event(event: Event, iterator: &mut EventIterator) -> Result<Node, ParseError> {
  match event {
    Event::Start(tag) => parse_start(tag, iterator),
    Event::End(_) => Ok(iterator.get_not_implemented()), // do nothing
    Event::Code(_) => parse_code(iterator).map(|x| x.into()),
    Event::Text(_) => parse_text(iterator).map(|x| x.into()),
    Event::Html(html) => parse_html(html, iterator).map(|x| x.into()),
    Event::InlineHtml(html) => parse_html(html, iterator).map(Into::into),
    Event::FootnoteReference(reference) => parse_footnote_reference(reference, iterator).map(|x| x.into()),
    Event::SoftBreak => Ok(
      SoftBreak {
        range: iterator.get_last_range(),
      }
      .into(),
    ),
    Event::HardBreak => Ok(
      HardBreak {
        range: iterator.get_last_range(),
      }
      .into(),
    ),
    Event::Rule => Ok(
      HorizontalRule {
        range: iterator.get_last_range(),
      }
      .into(),
    ),
    Event::TaskListMarker(is_checked) => Ok(
      TaskListMarker {
        range: iterator.get_last_range(),
        is_checked,
      }
      .into(),
    ),
    Event::InlineMath(text) => parse_inline_math(text, iterator).map(|n| n.into()),
    Event::DisplayMath(text) => parse_display_math(text, iterator).map(|n| n.into()),
  }
}

fn parse_start(start_tag: Tag, iterator: &mut EventIterator) -> Result<Node, ParseError> {
  match start_tag {
    Tag::Heading { level, .. } => parse_heading(level, iterator).map(|x| x.into()),
    Tag::Paragraph => parse_paragraph(iterator).map(|x| x.into()),
    Tag::BlockQuote(_) => parse_block_quote(iterator).map(|x| x.into()),
    Tag::CodeBlock(kind) => parse_code_block(kind, iterator).map(|x| x.into()),
    Tag::FootnoteDefinition(label) => parse_footnote_definition(label, iterator).map(|x| x.into()),
    Tag::Table(column_alignment) => parse_table(column_alignment, iterator).map(|x| x.into()),
    Tag::TableHead => parse_table_head(iterator).map(|x| x.into()),
    Tag::TableRow => parse_table_row(iterator).map(|x| x.into()),
    Tag::TableCell => parse_table_cell(iterator).map(|x| x.into()),
    Tag::Emphasis => parse_text_decoration(TextDecorationKind::Emphasis, iterator).map(|x| x.into()),
    Tag::Strong => parse_text_decoration(TextDecorationKind::Strong, iterator).map(|x| x.into()),
    Tag::Strikethrough => parse_text_decoration(TextDecorationKind::Strikethrough, iterator).map(|x| x.into()),
    Tag::Link {
      link_type,
      dest_url: destination_url,
      title: link_title,
      ..
    } => parse_link(link_type, &destination_url, &link_title, iterator),
    Tag::Image { link_type, .. } => parse_image(link_type, iterator),
    Tag::List(first_item_number) => parse_list(first_item_number, iterator).map(|x| x.into()),
    Tag::Item => parse_item(iterator).map(|x| x.into()),
    Tag::HtmlBlock => parse_html_block(iterator).map(|x| x.into()),
    Tag::MetadataBlock(metadata_block_kind) => parse_metadata(metadata_block_kind, iterator).map(|x| x.into()),
    Tag::DefinitionList => parse_definition_list(iterator).map(|x| x.into()),
    Tag::DefinitionListTitle => parse_definition_list_title(iterator).map(|x| x.into()),
    Tag::DefinitionListDefinition => parse_definition_list_definition(iterator).map(|x| x.into()),
    // these tags are only emitted when their corresponding options are enabled, which they aren't
    Tag::Superscript | Tag::Subscript => Err(ParseError::new(
      iterator.get_last_range(),
      format!("Tag not implemented {:?}", start_tag),
    )),
  }
}

fn parse_heading(level: HeadingLevel, iterator: &mut EventIterator) -> Result<Heading, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::Heading(end_level)) => {
        if end_level == level {
          break;
        }
        return Err(ParseError::new(
          iterator.get_last_range(),
          format!("Found end tag with level {}, but expected {}", end_level, level),
        ));
      }
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  Ok(Heading {
    range: iterator.get_range_for_start(start),
    level: level as u32,
    children,
  })
}

fn parse_paragraph(iterator: &mut EventIterator) -> Result<Paragraph, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();
  let mut marker: Option<TaskListMarker> = None;

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::Paragraph) => break,
      Event::TaskListMarker(is_checked) if marker.is_none() => {
        marker = Some(TaskListMarker {
          range: iterator.get_last_range(),
          is_checked,
        });
      }
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  Ok(Paragraph {
    range: iterator.get_range_for_start(start),
    children,
    marker,
  })
}

fn parse_block_quote(iterator: &mut EventIterator) -> Result<BlockQuote, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();
  let mut last_event_end: Option<usize> = None;

  while let Some(event) = iterator.next() {
    if matches!(event, Event::End(TagEnd::BlockQuote(_))) {
      break;
    }

    // cmark doesn't raise events for link reference definitions, so look for
    // them in the text leading up to this event
    let current_range = iterator.get_last_range();
    if let Some(references) = parse_references(last_event_end.or(Some(start)), current_range.start, iterator)? {
      children.push(references);
    }

    children.push(parse_event(event, iterator)?);
    last_event_end = Some(current_range.end);
  }

  let range = iterator.get_range_for_start(start);
  // a block quote may consist of only link reference definitions, in which
  // case it has no children to search after
  if let Some(references) = parse_references(last_event_end.or(Some(start)), range.end, iterator)? {
    children.push(references);
  }

  Ok(BlockQuote { range, children })
}

fn parse_code_block(code_block_kind: CodeBlockKind, iterator: &mut EventIterator) -> Result<CodeBlock, ParseError> {
  let start = iterator.start();
  let mut code = String::new();

  let original_allow_empty_text_events = iterator.allow_empty_text_events;
  iterator.allow_empty_text_events = true;

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::CodeBlock) => break,
      Event::Text(_) => {
        let last_range = iterator.get_last_range();
        let raw_text = &iterator.file_text[last_range];
        code.push_str(raw_text)
      }
      _ => {
        return Err(ParseError::new(
          iterator.get_last_range(),
          "Unexpected event found when parsing code block.",
        ))
      }
    }
  }

  iterator.allow_empty_text_events = original_allow_empty_text_events;

  let is_fenced = matches!(code_block_kind, CodeBlockKind::Fenced(_));
  let tag = match code_block_kind {
    CodeBlockKind::Indented => None,
    CodeBlockKind::Fenced(tag) => {
      let tag = String::from(tag.as_ref().trim());
      if tag.is_empty() {
        None
      } else {
        Some(tag)
      }
    }
  };

  Ok(CodeBlock {
    range: iterator.get_range_for_start(start),
    tag,
    is_fenced,
    code,
  })
}

fn parse_code(iterator: &mut EventIterator) -> Result<Code, ParseError> {
  let mut raw_text = &iterator.file_text[iterator.get_last_range()];
  while raw_text.starts_with('`') && raw_text.ends_with('`') {
    raw_text = &raw_text[1..raw_text.len() - 1];
  }
  Ok(Code {
    range: iterator.get_last_range(),
    code: strip_block_quote_markers(&raw_text.replace("\r\n", "\n"), iterator.in_block_quote_count()),
  })
}

/// A code span may span multiple lines, in which case the raw text of the
/// continuation lines still contains the block quote markers they're within.
fn strip_block_quote_markers(text: &str, block_quote_count: usize) -> String {
  if block_quote_count == 0 || !text.contains('\n') {
    return text.to_string();
  }

  let mut result = String::with_capacity(text.len());
  for (i, line) in text.split('\n').enumerate() {
    if i > 0 {
      result.push('\n');
      result.push_str(strip_line_block_quote_markers(line, block_quote_count));
    } else {
      result.push_str(line);
    }
  }
  return result;

  fn strip_line_block_quote_markers(line: &str, block_quote_count: usize) -> &str {
    // a marker may be indented up to three spaces. Any further and it's part
    // of the code span's text rather than a marker
    const MAX_MARKER_INDENT: usize = 3;

    let mut line = line;
    for _ in 0..block_quote_count {
      let indent = line.len() - line.trim_start_matches(' ').len();
      if indent > MAX_MARKER_INDENT {
        break;
      }
      match line[indent..].strip_prefix('>') {
        // a single space following the marker is part of the marker
        Some(text) => line = text.strip_prefix(' ').unwrap_or(text),
        None => break,
      }
    }
    line
  }
}

fn parse_text(iterator: &mut EventIterator) -> Result<Text, ParseError> {
  // Pulldown cmark breaks up text items when they have escape chars
  // in them, so just combine the results.
  let raw_start = iterator.get_last_range().start;
  // Pulldown cmark has the start range in the wrong place when there's a leading backslash
  let is_start_backslash = raw_start > 0 && iterator.file_text.as_bytes()[raw_start - 1] == b'\\';
  let raw_start = if is_start_backslash { raw_start - 1 } else { raw_start };
  while let Some((Event::Text(_), _)) = iterator.peek() {
    iterator.next();
  }
  let raw_end = iterator.get_last_range().end;

  let raw_text = &iterator.file_text[raw_start..raw_end];
  let trimmed_text = trim_spaces_and_newlines(raw_text);
  let start = raw_start + (raw_text.len() - trim_start_spaces_and_newlines(raw_text).len());

  Ok(Text {
    range: Range {
      start,
      end: start + trimmed_text.len(),
    },
    text: trimmed_text.replace("\r\n", "\n"),
  })
}

fn parse_text_decoration(kind: TextDecorationKind, iterator: &mut EventIterator) -> Result<TextDecoration, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::Emphasis) => break,
      Event::End(TagEnd::Strikethrough) => break,
      Event::End(TagEnd::Strong) => break,
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  Ok(TextDecoration {
    range: iterator.get_range_for_start(start),
    kind,
    children,
  })
}

fn parse_html(text: CowStr, iterator: &mut EventIterator) -> Result<Html, ParseError> {
  let text = text.as_ref().trim_end();
  let start = iterator.get_last_range().start;
  Ok(Html {
    range: Range {
      start,
      end: start + text.len(),
    },
  })
}

fn parse_display_math(_text: CowStr, iterator: &mut EventIterator) -> Result<DisplayMath, ParseError> {
  Ok(DisplayMath {
    range: iterator.get_last_range(),
  })
}

fn parse_inline_math(_text: CowStr, iterator: &mut EventIterator) -> Result<InlineMath, ParseError> {
  Ok(InlineMath {
    range: iterator.get_last_range(),
  })
}

fn parse_html_block(iterator: &mut EventIterator) -> Result<Html, ParseError> {
  let start = iterator.start();
  let original_allow_empty_text_events = iterator.allow_empty_text_events;
  iterator.allow_empty_text_events = true;

  while let Some(event) = iterator.next() {
    if let Event::End(TagEnd::HtmlBlock) = event {
      break;
    }
  }

  iterator.allow_empty_text_events = original_allow_empty_text_events;

  let range = iterator.get_range_for_start(start);
  Ok(Html {
    range: Range {
      start,
      end: start + iterator.file_text[range].trim_end().len(),
    },
  })
}

fn parse_footnote_reference(name: CowStr, iterator: &mut EventIterator) -> Result<FootnoteReference, ParseError> {
  Ok(FootnoteReference {
    range: iterator.get_last_range(),
    name: String::from(name.as_ref()),
  })
}

fn parse_footnote_definition(name: CowStr, iterator: &mut EventIterator) -> Result<FootnoteDefinition, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::FootnoteDefinition) => break,
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  Ok(FootnoteDefinition {
    range: iterator.get_range_for_start(start),
    name: String::from(name.as_ref()),
    children,
  })
}

fn parse_link(
  link_type: LinkType,
  destination_url: &str,
  link_title: &str,
  iterator: &mut EventIterator,
) -> Result<Node, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::Link) => break,
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  let end = iterator.get_last_range().end;
  let range = Range { start, end };
  match link_type {
    LinkType::Inline => Ok(
      InlineLink {
        range,
        children,
        url: destination_url.to_string(),
        title: if link_title.trim().is_empty() {
          None
        } else {
          Some(link_title.trim().to_string())
        },
      }
      .into(),
    ),
    LinkType::Reference | LinkType::ReferenceUnknown | LinkType::Collapsed | LinkType::CollapsedUnknown => {
      let reference = parse_link_reference(&iterator.file_text[start..end]);
      Ok(
        ReferenceLink {
          range,
          children,
          reference,
        }
        .into(),
      )
    }
    LinkType::Shortcut | LinkType::ShortcutUnknown => Ok(ShortcutLink { range, children }.into()),
    LinkType::Email | LinkType::Autolink => Ok(AutoLink { range, children }.into()),
    // only emitted when Options::ENABLE_WIKILINKS is enabled, which it isn't
    LinkType::WikiLink { .. } => Err(ParseError::new(
      range,
      format!("Link type not implemented {:?}", link_type),
    )),
  }
}

fn parse_image(link_type: LinkType, iterator: &mut EventIterator) -> Result<Node, ParseError> {
  let start = iterator.start();

  while let Some(event) = iterator.next() {
    // ignore link children
    if let Event::End(TagEnd::Image) = event {
      break;
    }
  }

  parse_image_from_text(start, &iterator.file_text[start..], link_type)
}

fn parse_list(start_index: Option<u64>, iterator: &mut EventIterator) -> Result<List, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::List(_)) => break,
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  Ok(List {
    range: iterator.get_range_for_start(start),
    start_index,
    children,
  })
}

fn parse_table(column_alignment: Vec<Alignment>, iterator: &mut EventIterator) -> Result<Table, ParseError> {
  let start = iterator.start();
  let head_event = iterator.next();

  let header = if let Some(Event::Start(Tag::TableHead)) = head_event {
    parse_table_head(iterator)?
  } else {
    return Err(ParseError::new(
      iterator.get_last_range(),
      format!("Expected a table head event, but found: {:?}", head_event),
    ));
  };

  let mut rows = Vec::new();
  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::Table) => break,
      Event::Start(Tag::TableRow) => rows.push(parse_table_row(iterator)?),
      _ => {
        return Err(ParseError::new(
          iterator.get_last_range(),
          format!("Unexpected event kind in table: {:?}", event),
        ))
      }
    }
  }

  Ok(Table {
    range: iterator.get_range_for_start(start),
    header,
    column_alignment: column_alignment
      .into_iter()
      .map(|alignment| match alignment {
        Alignment::Left => ColumnAlignment::Left,
        Alignment::Center => ColumnAlignment::Center,
        Alignment::Right => ColumnAlignment::Right,
        Alignment::None => ColumnAlignment::None,
      })
      .collect(),
    rows,
  })
}

// todo: lots of duplicate code here... something should be done

fn parse_table_head(iterator: &mut EventIterator) -> Result<TableHead, ParseError> {
  let start = iterator.start();
  let mut cells = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::TableHead) => break,
      Event::Start(Tag::TableCell) => cells.push(parse_table_cell(iterator)?),
      _ => {
        return Err(ParseError::new(
          iterator.get_last_range(),
          format!("Unexpected event kind in table head: {:?}", event),
        ))
      }
    }
  }

  Ok(TableHead {
    range: iterator.get_range_for_start(start),
    cells,
  })
}

fn parse_table_row(iterator: &mut EventIterator) -> Result<TableRow, ParseError> {
  let start = iterator.start();
  let mut cells = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::TableRow) => break,
      Event::Start(Tag::TableCell) => cells.push(parse_table_cell(iterator)?),
      _ => {
        return Err(ParseError::new(
          iterator.get_last_range(),
          format!("Unexpected event kind in table row: {:?}", event),
        ))
      }
    }
  }

  Ok(TableRow {
    range: iterator.get_range_for_start(start),
    cells,
  })
}

fn parse_table_cell(iterator: &mut EventIterator) -> Result<TableCell, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::TableCell) => break,
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  Ok(TableCell {
    range: iterator.get_range_for_start(start),
    children,
  })
}

fn parse_item(iterator: &mut EventIterator) -> Result<Item, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();
  let mut sub_lists = Vec::new();

  let mut last_event_end: Option<usize> = None;
  let marker = if let Some((Event::TaskListMarker(is_checked), _)) = iterator.peek() {
    let marker = TaskListMarker {
      range: iterator.get_last_range(),
      is_checked: *is_checked,
    };
    iterator.next();
    last_event_end = Some(iterator.get_last_range().end);
    Some(marker)
  } else {
    None
  };

  while let Some(event) = iterator.next() {
    if matches!(event, Event::End(TagEnd::Item)) {
      break;
    }

    // cmark doesn't raise events for link reference definitions, so look for
    // them in the text leading up to this event
    let current_range = iterator.get_last_range();
    let references = match last_event_end {
      Some(last_event_end) => parse_references(Some(last_event_end), current_range.start, iterator)?,
      // the text before the first event contains the list item's marker and
      // possibly a task list marker, so ignore it when it doesn't parse
      None => parse_item_start_references(start, current_range.start, iterator),
    };
    if let Some(references) = references {
      children.append(&mut sub_lists); // the sub lists are no longer last
      children.push(references);
    }

    match event {
      Event::Start(Tag::List(_)) => sub_lists.push(parse_event(event, iterator)?),
      _ => {
        children.append(&mut sub_lists); // only add to the sub_lists if it's the last children
        children.push(parse_event(event, iterator)?)
      }
    }
    // a node may consume multiple events (ex. adjacent text events), so take
    // whatever the iterator ended up on
    last_event_end = Some(std::cmp::max(current_range.end, iterator.get_last_range().end));
  }

  let range = iterator.get_range_for_start(start);

  let references_start = last_event_end
    // an item may consist of only link reference definitions, in which case
    // it has no children, so start searching after the list item marker
    .or_else(|| get_item_marker_end(iterator.file_text, range.start));
  if let Some(references) = parse_references(references_start, range.end, iterator)? {
    children.append(&mut sub_lists); // the sub lists are no longer last
    children.push(references);
  }

  Ok(Item {
    range,
    marker,
    children,
    sub_lists,
  })
}

/// Parses any link reference definitions that appear before a list item's first
/// event, ignoring the text when it's not link reference definitions (ex. it
/// could be a task list marker, which cmark raises an event for later on).
fn parse_item_start_references(item_start: usize, end: usize, iterator: &mut EventIterator) -> Option<Node> {
  let start = get_item_marker_end(iterator.file_text, item_start)?;
  parse_references(Some(start), end, iterator).ok().flatten()
}

/// Gets the byte position directly after a list item's marker
/// (ex. after the `-` in `- test` or after the `1.` in `1. test`).
fn get_item_marker_end(file_text: &str, item_start: usize) -> Option<usize> {
  let mut chars = file_text[item_start..].char_indices();
  let (_, first_char) = chars.next()?;

  if matches!(first_char, '-' | '*' | '+') {
    return Some(item_start + first_char.len_utf8());
  }
  if !first_char.is_ascii_digit() {
    return None;
  }

  // ordered list marker (ex. `1.` or `1)`)
  for (index, c) in chars {
    if c.is_ascii_digit() {
      continue;
    }
    return if matches!(c, '.' | ')') {
      Some(item_start + index + c.len_utf8())
    } else {
      None
    };
  }

  None
}

fn parse_metadata(kind: MetadataBlockKind, iterator: &mut EventIterator) -> Result<MetadataBlock, ParseError> {
  let start = iterator.get_last_range().start;
  let mut text = String::new();
  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::MetadataBlock(end_kind)) if kind == end_kind => break,
      Event::End(TagEnd::MetadataBlock(end_kind)) => {
        return Err(ParseError::new(
          iterator.get_last_range(),
          format!("Expected metadata block to end with {kind:?}, found {end_kind:?}."),
        ))
      }
      Event::Text(t) => text.push_str(&t),
      Event::SoftBreak | Event::HardBreak => continue,
      // assuming here that pulldown-cmark doesn't tokenize anything beyond text and line breaks
      // between metadata block markers then everything else should be a parse error here
      _ => {
        return Err(ParseError::new(
          iterator.get_last_range(),
          "Unexpected event found when parsing code block.",
        ))
      }
    }
  }

  Ok(MetadataBlock {
    range: Range {
      start,
      end: start + text.len(),
    },
    kind,
    text,
  })
}

fn parse_definition_list(iterator: &mut EventIterator) -> Result<DefinitionList, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::DefinitionList) => break,
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  // cmark ends a definition list at the end of the block that follows it
  // rather than at the end of the list, so use the end of the last definition
  let end = match children.last() {
    Some(last) => last.range().end,
    None => iterator.get_last_range().end,
  };

  Ok(DefinitionList {
    range: Range { start, end },
    children,
  })
}

fn parse_definition_list_title(iterator: &mut EventIterator) -> Result<DefinitionListTitle, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(TagEnd::DefinitionListTitle) => break,
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  Ok(DefinitionListTitle {
    range: iterator.get_range_for_start(start),
    children,
  })
}

fn parse_definition_list_definition(iterator: &mut EventIterator) -> Result<DefinitionListDefinition, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();
  let mut last_event_end: Option<usize> = None;

  while let Some(event) = iterator.next() {
    if matches!(event, Event::End(TagEnd::DefinitionListDefinition)) {
      break;
    }

    // cmark doesn't raise events for link reference definitions, so look for
    // them in the text leading up to this event
    let current_range = iterator.get_last_range();
    let references_start = last_event_end.or_else(|| get_definition_marker_end(iterator.file_text, start));
    if let Some(references) = parse_references(references_start, current_range.start, iterator)? {
      children.push(references);
    }

    children.push(parse_event(event, iterator)?);
    // a node may consume multiple events (ex. adjacent text events), so take
    // whatever the iterator ended up on
    last_event_end = Some(std::cmp::max(current_range.end, iterator.get_last_range().end));
  }

  let range = iterator.get_range_for_start(start);
  // a definition may consist of only link reference definitions, in which case
  // it has no children to search after
  let references_start = last_event_end.or_else(|| get_definition_marker_end(iterator.file_text, start));
  if let Some(references) = parse_references(references_start, range.end, iterator)? {
    children.push(references);
  }

  Ok(DefinitionListDefinition { range, children })
}

/// Gets the byte position directly after a definition's `:` marker, which may
/// be indented (ex. after the `:` in `: test` or ` : test`).
fn get_definition_marker_end(file_text: &str, definition_start: usize) -> Option<usize> {
  let text = &file_text[definition_start..];
  let marker_pos = text.find(|c| c != ' ' && c != '\t')?;
  if text[marker_pos..].starts_with(':') {
    Some(definition_start + marker_pos + 1)
  } else {
    None
  }
}
