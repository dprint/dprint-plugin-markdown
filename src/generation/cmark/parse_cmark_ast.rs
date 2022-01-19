use super::parsing::parse_image as parse_image_from_text;
use super::parsing::parse_link_reference;
use super::parsing::parse_link_reference_definitions;
use crate::generation::common::*;
use crate::generation::trim_spaces_and_newlines;
use crate::generation::trim_start_spaces_and_newlines;
use pulldown_cmark::*;

struct EventIterator<'a> {
  iterator: OffsetIter<'a, 'a>,
  file_text: &'a str,
  last_range: Range,
  next: Option<(Event<'a>, Range)>,
  allow_empty_text_events: bool,
  in_table_count: i8,
}

impl<'a> EventIterator<'a> {
  pub fn new(file_text: &'a str, iterator: OffsetIter<'a, 'a>) -> EventIterator<'a> {
    let mut iterator = iterator;
    let next = iterator.next();
    // println!("Raw event: {:?}", next);
    EventIterator {
      file_text,
      iterator,
      last_range: Range { start: 0, end: 0 },
      next,
      allow_empty_text_events: false,
      in_table_count: 0,
    }
  }

  pub fn next(&mut self) -> Option<Event<'a>> {
    if let Some((event, range)) = self.next.take() {
      // println!("{:?} {:?}", range, event);
      self.last_range = range;
      self.next = self.move_iterator_next();

      if !self.allow_empty_text_events {
        // skip over any empty text or html events
        while let Some((Event::Text(_), range)) | Some((Event::Html(_), range)) = &self.next {
          if self.file_text[range.start..range.end].trim().is_empty() {
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
    // println!("Raw event: {:?}", next);

    match next {
      Some((Event::Start(Tag::Table(_)), _)) => self.in_table_count += 1,
      Some((Event::End(Tag::Table(_)), _)) => self.in_table_count -= 1,
      _ => {}
    }

    next
  }

  pub fn is_in_table(&self) -> bool {
    self.in_table_count > 0
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
  pub fn peek(&self) -> &Option<(Event, Range)> {
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

  let mut children: Vec<Node> = Vec::new();
  let mut iterator = EventIterator::new(markdown_text, Parser::new_ext(markdown_text, options).into_offset_iter());
  let mut last_event_range: Option<Range> = None;

  while let Some(event) = iterator.next() {
    let current_range = iterator.get_last_range();

    // do not parse for link references while inside a table
    if !iterator.is_in_table() {
      if let Some(references) = parse_references(&last_event_range, current_range.start, &mut iterator)? {
        children.push(references);
      }
    }

    children.push(parse_event(event, &mut iterator)?);
    last_event_range = Some(current_range);
  }

  if let Some(references) = parse_references(&last_event_range, markdown_text.len(), &mut iterator)? {
    children.push(references);
  }

  Ok(SourceFile {
    children,
    range: iterator.get_range_for_start(0),
    yaml_header: None,
  })
}

fn parse_references(last_event_range: &Option<Range>, end: usize, iterator: &mut EventIterator) -> Result<Option<Node>, ParseError> {
  if let Some(last_event_range) = last_event_range {
    if last_event_range.end < end {
      let references = parse_link_reference_definitions(last_event_range.end, &iterator.file_text[last_event_range.end..end])?;
      if !references.is_empty() {
        return Ok(Some(
          Paragraph {
            range: Range {
              start: references.first().unwrap().range.start,
              end: references.last().unwrap().range.end,
            },
            children: references.into_iter().map(|x| x.into()).collect(),
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
    Event::Code(code) => parse_code(code, iterator).map(|x| x.into()),
    Event::Text(_) => parse_text(iterator).map(|x| x.into()),
    Event::Html(html) => parse_html(html, iterator).map(|x| x.into()),
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
  }
}

fn parse_start(start_tag: Tag, iterator: &mut EventIterator) -> Result<Node, ParseError> {
  match start_tag {
    Tag::Heading(level, _, _) => parse_heading(level, iterator).map(|x| x.into()),
    Tag::Paragraph => parse_paragraph(iterator).map(|x| x.into()),
    Tag::BlockQuote => parse_block_quote(iterator).map(|x| x.into()),
    Tag::CodeBlock(kind) => parse_code_block(kind, iterator).map(|x| x.into()),
    Tag::FootnoteDefinition(label) => parse_footnote_definition(label, iterator).map(|x| x.into()),
    Tag::Table(column_alignment) => parse_table(column_alignment, iterator).map(|x| x.into()),
    Tag::TableHead => parse_table_head(iterator).map(|x| x.into()),
    Tag::TableRow => parse_table_row(iterator).map(|x| x.into()),
    Tag::TableCell => parse_table_cell(iterator).map(|x| x.into()),
    Tag::Emphasis => parse_text_decoration(TextDecorationKind::Emphasis, iterator).map(|x| x.into()),
    Tag::Strong => parse_text_decoration(TextDecorationKind::Strong, iterator).map(|x| x.into()),
    Tag::Strikethrough => parse_text_decoration(TextDecorationKind::Strikethrough, iterator).map(|x| x.into()),
    Tag::Link(link_type, destination_url, link_title) => parse_link(link_type, &destination_url, &link_title, iterator),
    Tag::Image(link_type, _, _) => parse_image(link_type, iterator),
    Tag::List(first_item_number) => parse_list(first_item_number, iterator).map(|x| x.into()),
    Tag::Item => parse_item(iterator).map(|x| x.into()),
  }
}

fn parse_heading(level: HeadingLevel, iterator: &mut EventIterator) -> Result<Heading, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(Tag::Heading(end_level, _, _)) => {
        if end_level == level {
          break;
        }
        return Err(ParseError::new(
          iterator.get_last_range(),
          &format!("Found end tag with level {}, but expected {}", end_level, level),
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

  while let Some(event) = iterator.next() {
    match event {
      Event::End(Tag::Paragraph) => break,
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  Ok(Paragraph {
    range: iterator.get_range_for_start(start),
    children,
  })
}

fn parse_block_quote(iterator: &mut EventIterator) -> Result<BlockQuote, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(Tag::BlockQuote) => break,
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  Ok(BlockQuote {
    range: iterator.get_range_for_start(start),
    children,
  })
}

fn parse_code_block(code_block_kind: CodeBlockKind, iterator: &mut EventIterator) -> Result<CodeBlock, ParseError> {
  let start = iterator.start();
  let mut code = String::new();

  iterator.allow_empty_text_events = true;

  while let Some(event) = iterator.next() {
    match event {
      Event::End(Tag::CodeBlock(_)) => break,
      Event::Text(event_text) => code.push_str(event_text.as_ref()),
      _ => return Err(ParseError::new(iterator.get_last_range(), "Unexpected event found when parsing code block.")),
    }
  }

  iterator.allow_empty_text_events = false;

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

fn parse_code(code: CowStr, iterator: &mut EventIterator) -> Result<Code, ParseError> {
  Ok(Code {
    range: iterator.get_last_range(),
    code: String::from(code.as_ref()),
  })
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
      Event::End(Tag::Emphasis) => break,
      Event::End(Tag::Strikethrough) => break,
      Event::End(Tag::Strong) => break,
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
  let text = String::from(text.as_ref().trim_end());
  let start = iterator.get_last_range().start;
  Ok(Html {
    range: Range {
      start,
      end: start + text.len(),
    },
    text,
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
      Event::End(Tag::FootnoteDefinition(_)) => break,
      _ => children.push(parse_event(event, iterator)?),
    }
  }

  Ok(FootnoteDefinition {
    range: iterator.get_range_for_start(start),
    name: String::from(name.as_ref()),
    children,
  })
}

fn parse_link(link_type: LinkType, destination_url: &str, link_title: &str, iterator: &mut EventIterator) -> Result<Node, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(Tag::Link(_, _, _)) => break,
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
      Ok(ReferenceLink { range, children, reference }.into())
    }
    LinkType::Shortcut | LinkType::ShortcutUnknown => Ok(ShortcutLink { range, children }.into()),
    LinkType::Email | LinkType::Autolink => Ok(AutoLink { range, children }.into()),
  }
}

fn parse_image(link_type: LinkType, iterator: &mut EventIterator) -> Result<Node, ParseError> {
  let start = iterator.start();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(Tag::Image(_, _, _)) => break,
      _ => {} // ignore link children
    }
  }

  parse_image_from_text(start, &iterator.file_text[start..], link_type)
}

fn parse_list(start_index: Option<u64>, iterator: &mut EventIterator) -> Result<List, ParseError> {
  let start = iterator.start();
  let mut children = Vec::new();

  while let Some(event) = iterator.next() {
    match event {
      Event::End(Tag::List(_)) => break,
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
      &format!("Expected a table head event, but found: {:?}", head_event),
    ));
  };

  let mut rows = Vec::new();
  while let Some(event) = iterator.next() {
    match event {
      Event::End(Tag::Table(_)) => break,
      Event::Start(Tag::TableRow) => rows.push(parse_table_row(iterator)?),
      _ => {
        return Err(ParseError::new(
          iterator.get_last_range(),
          &format!("Unexpected event kind in table: {:?}", event),
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
      Event::End(Tag::TableHead) => break,
      Event::Start(Tag::TableCell) => cells.push(parse_table_cell(iterator)?),
      _ => {
        return Err(ParseError::new(
          iterator.get_last_range(),
          &format!("Unexpected event kind in table head: {:?}", event),
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
      Event::End(Tag::TableRow) => break,
      Event::Start(Tag::TableCell) => cells.push(parse_table_cell(iterator)?),
      _ => {
        return Err(ParseError::new(
          iterator.get_last_range(),
          &format!("Unexpected event kind in table row: {:?}", event),
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
      Event::End(Tag::TableCell) => break,
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

  let marker = if let Some((Event::TaskListMarker(is_checked), _)) = iterator.peek() {
    let marker = TaskListMarker {
      range: iterator.get_last_range(),
      is_checked: *is_checked,
    };
    iterator.next();
    Some(marker)
  } else {
    None
  };

  while let Some(event) = iterator.next() {
    match event {
      Event::End(Tag::Item) => break,
      Event::Start(Tag::List(_)) => sub_lists.push(parse_event(event, iterator)?),
      _ => {
        children.append(&mut sub_lists); // only add to the sub_lists if it's the last children
        children.push(parse_event(event, iterator)?)
      }
    }
  }

  let range = iterator.get_range_for_start(start);

  let last_range = sub_lists
    .last()
    .map(|c| c.range())
    .or_else(|| children.last().map(|c| c.range()))
    .map(|r| r.to_owned());
  if let Some(references) = parse_references(&last_range, range.end, iterator)? {
    children.push(references);
  }

  Ok(Item {
    range,
    marker,
    children,
    sub_lists,
  })
}
