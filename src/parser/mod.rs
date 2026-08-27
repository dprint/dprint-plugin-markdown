//! A markdown parser written for formatting rather than rendering.
//!
//! Unlike a parser that exists to produce html, this one accounts for every
//! byte of the source: nothing is normalized away, every node knows the exact
//! text it came from, and the constructs a renderer has no use for (ex. link
//! reference definitions) are nodes like any other. It borrows from the source
//! text wherever it can, only allocating for content that a container's line
//! prefixes split apart.

mod ast;
mod block;
mod inline;
mod links;
mod source;
mod unicode;

#[cfg(test)]
mod debug_json;
#[cfg(test)]
pub mod spec_test;
#[cfg(test)]
mod text_fuzz;

use std::collections::HashSet;

pub use ast::*;

pub use block::block_start_escape;
pub use block::is_whole_html_block;
pub use block::line_start_escape;
pub use block::starts_block_at_block_start;
pub use block::starts_block_in_paragraph;
pub use inline::is_markdown_punctuation;
pub use source::SPACES;
pub use source::WHITESPACE;

use block::BlockParser;
use inline::InlineContext;
use source::source_lines;
use source::ContentLine;

/// The document was written in a way the parser won't read.
#[derive(Debug)]
pub enum ParseError {
  /// Containers are written more deeply within one another than the parser
  /// will follow.
  TooDeep { limit: usize },
}

impl std::fmt::Display for ParseError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ParseError::TooDeep { limit } => {
        write!(f, "the document nests blocks more than {} deep", limit)
      }
    }
  }
}

/// Parses markdown text into a [`SourceFile`].
pub fn parse(source: &str) -> Result<SourceFile<'_>, ParseError> {
  let lines = source_lines(source);
  let (metadata, body_start) = parse_metadata_block(source, &lines);

  let (link_labels, footnote_labels) = collect_labels(source, &lines[body_start..]);
  let context = InlineContext::new(source, link_labels, footnote_labels, false);
  let parser = BlockParser {
    source,
    context: &context,
    depth: Default::default(),
    too_deep: Default::default(),
  };

  let mut children: Vec<Node<'_>> = Vec::new();
  if let Some(metadata) = metadata {
    children.push(metadata.into());
  }
  children.extend(parser.parse_blocks(&lines[body_start..]));
  if parser.too_deep.get() {
    return Err(ParseError::TooDeep {
      limit: block::MAX_NESTING,
    });
  }

  Ok(SourceFile {
    span: Span::new(0, source.len()),
    children,
  })
}

/// Parses the front matter at the top of the file, returning the line the rest
/// of the document starts at.
fn parse_metadata_block<'a>(source: &'a str, lines: &[ContentLine<'a>]) -> (Option<MetadataBlock<'a>>, usize) {
  let Some(first) = lines.first() else {
    return (None, 0);
  };
  let kind = match first.text.trim_end_matches(SPACES) {
    "---" => MetadataBlockKind::YamlStyle,
    "+++" => MetadataBlockKind::PlusesStyle,
    _ => return (None, 0),
  };
  let close = match kind {
    MetadataBlockKind::YamlStyle => ["---", "..."],
    MetadataBlockKind::PlusesStyle => ["+++", "+++"],
  };

  // the block's first line can be neither blank nor its closing delimiter
  match lines.get(1) {
    Some(line) if !line.is_blank() && !close.contains(&line.text.trim_end_matches(SPACES)) => {}
    _ => return (None, 0),
  }

  let Some(end) = lines[2..]
    .iter()
    .position(|line| close.contains(&line.text.trim_end_matches(SPACES)))
    .map(|index| index + 2)
  else {
    return (None, 0);
  };

  let text = match lines.get(1) {
    Some(start) if end > 1 => Span::new(start.start, lines[end - 1].end()).text(source),
    _ => "",
  };
  (
    Some(MetadataBlock {
      span: Span::new(first.start, lines[end].trim_end().end()),
      kind,
      text,
    }),
    end + 1,
  )
}

/// Finds the labels the document defines, which decide whether a `[label]` is
/// a link or a footnote reference rather than plain text.
fn collect_labels(source: &str, lines: &[ContentLine<'_>]) -> (HashSet<String>, HashSet<String>) {
  let mut link_labels = HashSet::new();
  let mut footnote_labels = HashSet::new();
  // definitions always contain a `]:`, so most documents can skip the pass
  if !source.contains("]:") {
    return (link_labels, footnote_labels);
  }

  let context = InlineContext::new(source, HashSet::new(), HashSet::new(), true);
  let parser = BlockParser {
    source,
    context: &context,
    depth: Default::default(),
    too_deep: Default::default(),
  };
  collect_node_labels(&parser.parse_blocks(lines), &mut link_labels, &mut footnote_labels);
  (link_labels, footnote_labels)
}

fn collect_node_labels(nodes: &[Node<'_>], link_labels: &mut HashSet<String>, footnote_labels: &mut HashSet<String>) {
  for node in nodes {
    match node {
      Node::LinkReference(reference) => {
        link_labels.insert(links::normalize_label(&reference.name).into_owned());
      }
      Node::FootnoteDefinition(definition) => {
        footnote_labels.insert(links::normalize_label(definition.name).into_owned());
      }
      _ => {}
    }
    collect_node_labels(node.children(), link_labels, footnote_labels);
    if let Node::Item(item) = node {
      collect_node_labels(&item.sub_lists, link_labels, footnote_labels);
    }
  }
}
