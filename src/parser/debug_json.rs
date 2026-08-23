//! Serializes the AST to json so that the parser's spec tests can assert on it.

use super::ast::*;

pub fn to_json(file: &SourceFile<'_>, source: &str) -> String {
  let mut writer = JsonWriter {
    source,
    text: String::new(),
    indent: 0,
  };
  writer.open_node("SourceFile", file.span);
  writer.write_children(&file.children);
  writer.close_node();
  writer.text.push('\n');
  writer.text
}

struct JsonWriter<'a> {
  source: &'a str,
  text: String,
  indent: usize,
}

impl<'a> JsonWriter<'a> {
  fn write_node(&mut self, node: &Node<'_>) {
    self.open_node(node.kind().as_str(), node.span());
    self.write_fields(node);
    self.write_child_nodes(node);
    self.close_node();
  }

  fn write_fields(&mut self, node: &Node<'_>) {
    match node {
      Node::MetadataBlock(node) => {
        self.property("metadataKind", &format!("{:?}", node.kind));
        self.property("content", node.text);
      }
      Node::Heading(node) => {
        self.raw_property("level", &node.level.to_string());
        self.property("headingStyle", &format!("{:?}", node.style));
      }
      Node::TextDecoration(node) => self.property("decoration", &format!("{:?}", node.kind)),
      Node::Html(node) => {
        self.raw_property("isBlock", &node.is_block.to_string());
        self.content_property(node.span, &node.text);
      }
      Node::DisplayMath(node) => self.property("content", &node.text),
      Node::InlineMath(node) => self.property("content", &node.text),
      Node::Code(node) => self.property("content", &node.code),
      Node::CodeBlock(node) => {
        self.raw_property("isFenced", &node.is_fenced().to_string());
        if let Some(fence) = &node.fence {
          self.property("fence", &fence.fence_char.to_string().repeat(fence.fence_len));
        }
        if let Some(tag) = node.tag() {
          self.property("tag", tag);
        }
        self.property("content", &node.code);
      }
      Node::FootnoteReference(node) => self.property("name", node.name),
      Node::FootnoteDefinition(node) => self.property("name", node.name),
      Node::InlineLink(node) => {
        self.property("url", &node.url);
        self.optional_property("title", node.title.as_deref());
      }
      Node::ReferenceLink(node) => {
        self.property("reference", &node.reference);
        self.raw_property("isCollapsed", &node.is_collapsed.to_string());
      }
      Node::LinkReference(node) => {
        self.property("name", &node.name);
        self.property("link", &node.link);
        self.optional_property("title", node.title.as_deref());
      }
      Node::InlineImage(node) => {
        self.property("alt", &node.text);
        self.property("url", &node.url);
        self.optional_property("title", node.title.as_deref());
      }
      Node::ReferenceImage(node) => {
        self.property("alt", &node.text);
        self.property("reference", &node.reference);
      }
      Node::ShortcutImage(node) => self.property("alt", &node.text),
      Node::List(node) => {
        self.property("markerChar", &node.marker_char.to_string());
        match node.start_index {
          Some(start_index) => self.raw_property("startIndex", &start_index.to_string()),
          None => self.property("listKind", "Bullet"),
        }
      }
      Node::Item(node) => {
        self.span_property("markerSpan", node.marker_span);
        self.write_task_list_marker(node.marker.as_ref());
      }
      Node::Paragraph(node) => self.write_task_list_marker(node.marker.as_ref()),
      Node::TaskListMarker(node) => self.raw_property("isChecked", &node.is_checked.to_string()),
      Node::Table(node) => {
        let alignment: Vec<String> = node
          .column_alignment
          .iter()
          .map(|alignment| format!("{:?}", alignment))
          .collect();
        self.string_array_property("columnAlignment", &alignment);
      }
      _ => {}
    }
  }

  fn write_child_nodes(&mut self, node: &Node<'_>) {
    match node {
      Node::Table(node) => {
        self.begin_property("header");
        self.open_node("TableHead", node.header.span);
        self.write_cells(&node.header.cells);
        self.close_node();
        self.text.push_str(",\n");

        if !node.rows.is_empty() {
          self.begin_property("rows");
          self.begin_array();
          for row in &node.rows {
            self.write_indent();
            self.open_node("TableRow", row.span);
            self.write_cells(&row.cells);
            self.close_node();
            self.text.push_str(",\n");
          }
          self.end_array();
        }
      }
      Node::Item(item) => {
        self.write_children(&item.children);
        if !item.sub_lists.is_empty() {
          self.begin_property("subLists");
          self.write_node_array(&item.sub_lists);
        }
      }
      _ => self.write_children(node.children()),
    }
  }

  fn write_cells(&mut self, cells: &[TableCell<'_>]) {
    if cells.is_empty() {
      return;
    }
    self.begin_property("cells");
    self.begin_array();
    for cell in cells {
      self.write_indent();
      self.open_node("TableCell", cell.span);
      self.write_children(&cell.children);
      self.close_node();
      self.text.push_str(",\n");
    }
    self.end_array();
  }

  fn write_task_list_marker(&mut self, marker: Option<&TaskListMarker>) {
    let Some(marker) = marker else {
      return;
    };
    self.begin_property("marker");
    self.open_node("TaskListMarker", marker.span);
    self.raw_property("isChecked", &marker.is_checked.to_string());
    self.close_node();
    self.text.push_str(",\n");
  }

  fn write_children(&mut self, children: &[Node<'_>]) {
    if children.is_empty() {
      return;
    }
    self.begin_property("children");
    self.write_node_array(children);
  }

  fn write_node_array(&mut self, nodes: &[Node<'_>]) {
    self.begin_array();
    for node in nodes {
      self.write_indent();
      self.write_node(node);
      self.text.push_str(",\n");
    }
    self.end_array();
  }

  // ---- primitives ----

  fn open_node(&mut self, kind: &str, span: Span) {
    self.text.push_str("{\n");
    self.indent += 1;
    self.property("kind", kind);
    self.span_property("span", span);
    self.property("text", span.text(self.source));
  }

  fn close_node(&mut self) {
    self.trim_trailing_comma();
    self.indent -= 1;
    self.write_indent();
    self.text.push('}');
  }

  fn begin_array(&mut self) {
    self.text.push_str("[\n");
    self.indent += 1;
  }

  fn end_array(&mut self) {
    self.trim_trailing_comma();
    self.indent -= 1;
    self.write_indent();
    self.text.push_str("],\n");
  }

  fn begin_property(&mut self, name: &str) {
    self.write_indent();
    self.text.push('"');
    self.text.push_str(name);
    self.text.push_str("\": ");
  }

  fn property(&mut self, name: &str, value: &str) {
    self.begin_property(name);
    write_json_string(&mut self.text, value);
    self.text.push_str(",\n");
  }

  fn optional_property(&mut self, name: &str, value: Option<&str>) {
    if let Some(value) = value {
      self.property(name, value);
    }
  }

  /// Writes the node's content when it isn't the same as its source text,
  /// which happens when a container's prefixes were stripped out of it.
  fn content_property(&mut self, span: Span, value: &str) {
    if value != span.text(self.source) {
      self.property("content", value);
    }
  }

  fn raw_property(&mut self, name: &str, value: &str) {
    self.begin_property(name);
    self.text.push_str(value);
    self.text.push_str(",\n");
  }

  fn span_property(&mut self, name: &str, span: Span) {
    self.raw_property(name, &format!("[{}, {}]", span.start, span.end));
  }

  fn string_array_property(&mut self, name: &str, values: &[String]) {
    let values: Vec<String> = values
      .iter()
      .map(|value| {
        let mut text = String::new();
        write_json_string(&mut text, value);
        text
      })
      .collect();
    self.raw_property(name, &format!("[{}]", values.join(", ")));
  }

  fn trim_trailing_comma(&mut self) {
    if self.text.ends_with(",\n") {
      self.text.truncate(self.text.len() - 2);
      self.text.push('\n');
    }
  }

  fn write_indent(&mut self) {
    for _ in 0..self.indent {
      self.text.push_str("  ");
    }
  }
}

fn write_json_string(text: &mut String, value: &str) {
  text.push('"');
  for c in value.chars() {
    match c {
      '"' => text.push_str("\\\""),
      '\\' => text.push_str("\\\\"),
      '\n' => text.push_str("\\n"),
      '\r' => text.push_str("\\r"),
      '\t' => text.push_str("\\t"),
      c if (c as u32) < 0x20 => text.push_str(&format!("\\u{:04x}", c as u32)),
      c => text.push(c),
    }
  }
  text.push('"');
}
