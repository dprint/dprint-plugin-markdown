//! The markdown AST produced by [`crate::parser::parse`].
//!
//! Every node borrows from the source text and carries the exact byte [`Span`]
//! it was parsed from, so the formatter can always fall back to the original
//! text. The nodes are allocated in an arena, along with the text of the ones
//! whose content isn't a contiguous slice of the source, which happens when a
//! container's line prefixes (ex. the `> ` of a block quote) had to be stripped
//! out of the middle of it. The arena never runs destructors, so nothing in a
//! node may own memory of its own.

use dprint_core::formatting::bumpalo::collections::Vec as BumpVec;

/// A byte range into the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
  pub start: usize,
  pub end: usize,
}

impl Span {
  pub fn new(start: usize, end: usize) -> Span {
    debug_assert!(start <= end, "span start {start} is past its end {end}");
    Span { start, end }
  }

  pub fn text<'a>(&self, source: &'a str) -> &'a str {
    &source[self.start..self.end]
  }

  pub fn len(&self) -> usize {
    self.end - self.start
  }
}

pub trait Ranged {
  fn span(&self) -> Span;
}

pub struct SourceFile<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
  /// The normalized labels of the link reference definitions the file holds,
  /// which decide whether a `[label]` within it is a link.
  pub link_labels: Labels<'a>,
  /// The labels of the footnote definitions the file holds.
  pub footnote_labels: Labels<'a>,
}

/// A set of normalized labels, held sorted in the arena so that it can be
/// searched without owning memory of its own.
#[derive(Debug, Clone, Copy, Default)]
pub struct Labels<'a>(&'a [&'a str]);

impl<'a> Labels<'a> {
  pub fn new(mut labels: BumpVec<'a, &'a str>) -> Labels<'a> {
    labels.sort_unstable();
    labels.dedup();
    Labels(labels.into_bump_slice())
  }

  pub fn contains(&self, label: &str) -> bool {
    self.0.binary_search(&label).is_ok()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

/// A `---` or `+++` delimited block of front matter at the top of a file.
pub struct MetadataBlock<'a> {
  pub span: Span,
  pub kind: MetadataBlockKind,
  /// The text between the opening and closing markers.
  pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataBlockKind {
  /// Delimited by `---`.
  YamlStyle,
  /// Delimited by `+++`.
  PlusesStyle,
}

pub struct Heading<'a> {
  pub span: Span,
  /// 1 through 6.
  pub level: u8,
  /// How the heading was written, which the formatter overrides with what its
  /// configuration asks for.
  #[allow(dead_code)]
  pub style: HeadingStyle,
  pub children: &'a [Node<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingStyle {
  /// A `#` prefixed heading.
  Atx,
  /// A heading underlined with `=` or `-`.
  Setext,
}

pub struct Paragraph<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
  /// The task list marker of the list item this paragraph starts, if any.
  pub marker: Option<TaskListMarker>,
}

pub struct BlockQuote<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
}

pub struct Text<'a> {
  pub span: Span,
  pub text: &'a str,
}

pub struct TextDecoration<'a> {
  pub span: Span,
  pub kind: TextDecorationKind,
  pub children: &'a [Node<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDecorationKind {
  Emphasis,
  Strong,
  Strikethrough,
}

/// A raw html block or inline html tag.
pub struct Html<'a> {
  pub span: Span,
  pub text: &'a str,
  /// Whether this is a block of its own rather than a tag within some text.
  pub is_block: bool,
}

/// A `$$` delimited math block.
pub struct DisplayMath<'a> {
  pub span: Span,
  /// The math between the delimiters.
  #[allow(dead_code)]
  pub text: &'a str,
}

/// A `$` delimited math span.
pub struct InlineMath<'a> {
  pub span: Span,
  /// The math between the delimiters.
  #[allow(dead_code)]
  pub text: &'a str,
}

pub struct FootnoteReference<'a> {
  pub span: Span,
  pub name: &'a str,
}

pub struct FootnoteDefinition<'a> {
  pub span: Span,
  pub name: &'a str,
  pub children: &'a [Node<'a>],
}

pub struct InlineLink<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
  pub url: &'a str,
  pub title: Option<&'a str>,
}

/// A link that refers to a [`LinkReference`] by name (ex. `[text][name]` or,
/// where the text is the name, `[text][]`).
pub struct ReferenceLink<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
  pub reference: &'a str,
  /// Whether the name was left out because the link's text is it
  /// (ex. `[name][]`).
  pub is_collapsed: bool,
}

/// A link whose text is also its reference name (ex. `[name]`).
pub struct ShortcutLink<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
}

/// A `<https://example.com>` style link, or a bare email address in brackets.
pub struct AutoLink<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
}

/// A link reference definition (ex. `[name]: https://example.com "title"`).
pub struct LinkReference<'a> {
  pub span: Span,
  pub name: &'a str,
  pub link: &'a str,
  pub title: Option<&'a str>,
}

pub struct InlineImage<'a> {
  pub span: Span,
  pub text: &'a str,
  pub url: &'a str,
  pub title: Option<&'a str>,
}

pub struct ReferenceImage<'a> {
  pub span: Span,
  pub text: &'a str,
  pub reference: &'a str,
}

pub struct ShortcutImage<'a> {
  pub span: Span,
  pub text: &'a str,
}

pub struct SoftBreak {
  pub span: Span,
}

pub struct HardBreak {
  pub span: Span,
}

pub struct List<'a> {
  pub span: Span,
  /// The number the first item of an ordered list starts at, or `None` when
  /// the list is unordered.
  pub start_index: Option<u64>,
  /// The character the items are marked with (ex. `-`, `*`, `+`, `.` or `)`),
  /// which the formatter writes from its configuration rather than the file.
  #[allow(dead_code)]
  pub marker_char: char,
  pub children: &'a [Node<'a>],
}

pub struct Item<'a> {
  pub span: Span,
  /// The `-` or `1.` that starts the item, not including the whitespace
  /// that follows it.
  pub marker_span: Span,
  pub marker: Option<TaskListMarker>,
  pub children: &'a [Node<'a>],
  /// The lists at the end of the item, which the formatter keeps beside the
  /// item's other children rather than indenting them further.
  pub sub_lists: &'a [Node<'a>],
}

pub struct TaskListMarker {
  pub span: Span,
  pub is_checked: bool,
}

/// A definition list. Contains alternating `DefinitionListTitle` and
/// `DefinitionListDefinition` children.
pub struct DefinitionList<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
}

/// The term of a definition list.
pub struct DefinitionListTitle<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
}

/// A `:` prefixed definition of the preceding definition list term.
pub struct DefinitionListDefinition<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
}

/// Inline code.
pub struct Code<'a> {
  pub span: Span,
  pub code: &'a str,
}

pub struct CodeBlock<'a> {
  pub span: Span,
  /// The fence the block is delimited by, or `None` when it's an indented
  /// code block.
  pub fence: Option<CodeFence<'a>>,
  pub code: &'a str,
}

impl<'a> CodeBlock<'a> {
  pub fn is_fenced(&self) -> bool {
    self.fence.is_some()
  }

  /// The language tag of a fenced code block (ex. the `ts` of ```` ```ts ````).
  pub fn tag(&self) -> Option<&'a str> {
    self.fence.as_ref().and_then(|f| f.info)
  }
}

pub struct CodeFence<'a> {
  /// Either a backtick or a `~`. The formatter picks the character that suits
  /// the code rather than keeping this one.
  #[allow(dead_code)]
  pub fence_char: char,
  /// How many fence characters the opening fence has, which the formatter
  /// likewise decides for itself.
  #[allow(dead_code)]
  pub fence_len: usize,
  /// The trimmed info string, or `None` when there isn't one.
  pub info: Option<&'a str>,
}

pub struct HorizontalRule {
  pub span: Span,
}

pub struct Table<'a> {
  pub span: Span,
  pub header: TableHead<'a>,
  pub column_alignment: &'a [ColumnAlignment],
  pub rows: &'a [TableRow<'a>],
}

pub struct TableHead<'a> {
  pub span: Span,
  pub cells: &'a [TableCell<'a>],
}

pub struct TableRow<'a> {
  pub span: Span,
  pub cells: &'a [TableCell<'a>],
}

pub struct TableCell<'a> {
  pub span: Span,
  pub children: &'a [Node<'a>],
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum ColumnAlignment {
  None,
  Left,
  Center,
  Right,
}

macro_rules! generate_node {
  ($($node_name:ident$(<$lifetime:lifetime>)?),* $(,)?) => {
    pub enum Node<'a> {
      $($node_name($node_name$(<$lifetime>)?)),*,
    }

    /// The name of a node's kind, which the parser's spec tests write out.
    #[cfg(test)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NodeKind {
      $($node_name),*,
    }

    #[cfg(test)]
    impl NodeKind {
      pub fn as_str(&self) -> &'static str {
        match self {
          $(NodeKind::$node_name => stringify!($node_name)),*
        }
      }
    }

    #[cfg(test)]
    impl<'a> Node<'a> {
      pub fn kind(&self) -> NodeKind {
        match self {
          $(Node::$node_name(_) => NodeKind::$node_name),*
        }
      }
    }

    impl<'a> Ranged for Node<'a> {
      fn span(&self) -> Span {
        match self {
          $(Node::$node_name(node) => node.span),*
        }
      }
    }

    $(
      impl$(<$lifetime>)? Ranged for $node_name$(<$lifetime>)? {
        fn span(&self) -> Span {
          self.span
        }
      }

      impl<'a> From<$node_name$(<$lifetime>)?> for Node<'a> {
        fn from(node: $node_name$(<$lifetime>)?) -> Node<'a> {
          Node::$node_name(node)
        }
      }
    )*
  };
}

generate_node![
  SourceFile<'a>,
  MetadataBlock<'a>,
  Heading<'a>,
  Paragraph<'a>,
  BlockQuote<'a>,
  Text<'a>,
  TextDecoration<'a>,
  Html<'a>,
  DisplayMath<'a>,
  InlineMath<'a>,
  FootnoteReference<'a>,
  FootnoteDefinition<'a>,
  InlineLink<'a>,
  ReferenceLink<'a>,
  ShortcutLink<'a>,
  AutoLink<'a>,
  LinkReference<'a>,
  InlineImage<'a>,
  ReferenceImage<'a>,
  ShortcutImage<'a>,
  SoftBreak,
  HardBreak,
  List<'a>,
  Item<'a>,
  TaskListMarker,
  DefinitionList<'a>,
  DefinitionListTitle<'a>,
  DefinitionListDefinition<'a>,
  Code<'a>,
  CodeBlock<'a>,
  HorizontalRule,
  Table<'a>,
  TableHead<'a>,
  TableRow<'a>,
  TableCell<'a>,
];

impl<'a> Node<'a> {
  pub fn children(&self) -> &'a [Node<'a>] {
    match self {
      Node::SourceFile(node) => node.children,
      Node::Heading(node) => node.children,
      Node::Paragraph(node) => node.children,
      Node::BlockQuote(node) => node.children,
      Node::TextDecoration(node) => node.children,
      Node::FootnoteDefinition(node) => node.children,
      Node::InlineLink(node) => node.children,
      Node::ReferenceLink(node) => node.children,
      Node::ShortcutLink(node) => node.children,
      Node::AutoLink(node) => node.children,
      Node::List(node) => node.children,
      Node::Item(node) => node.children,
      Node::DefinitionList(node) => node.children,
      Node::DefinitionListTitle(node) => node.children,
      Node::DefinitionListDefinition(node) => node.children,
      Node::TableCell(node) => node.children,
      _ => &[],
    }
  }
}

// the arena never runs destructors, so a node that owned memory would leak it
const _: () = assert!(!std::mem::needs_drop::<Node<'static>>());
