//! Serializes the html AST to json so that the parser's spec tests can assert
//! on it.
//!
//! Every node reports the span it was parsed from, which is recovered from the
//! text it borrows by measuring that slice against the start of the source.
//! Nothing here is part of what the crate hands out -- it exists so that a
//! spec file can show what the parser read.

use super::ast::*;

pub fn to_json(document: &Document<'_>, source: &str) -> String {
  let mut writer = JsonWriter {
    source,
    text: String::new(),
    indent: 0,
  };
  writer.text.push_str("{\n");
  writer.indent += 1;
  writer.property("kind", "Document");
  writer.write_children(&document.children);
  writer.close_node();
  writer.text.push('\n');
  writer.text
}

/// The byte the slice starts at within the source it was borrowed from.
pub fn offset_in(slice: &str, source: &str) -> usize {
  slice.as_ptr() as usize - source.as_ptr() as usize
}

struct JsonWriter<'a> {
  source: &'a str,
  text: String,
  indent: usize,
}

impl JsonWriter<'_> {
  fn write_node(&mut self, node: &Node<'_>) {
    self.open_node(node.kind(), node.source());
    if let Node::Element(element) = node {
      self.property("name", element.name);
      self.property("elementKind", &format!("{:?}", element.kind));
      if element.self_closing_syntax {
        self.raw_property("selfClosingSyntax", "true");
      }
      self.write_attributes(&element.attributes);
      if !element.content.is_empty() {
        self.property("content", element.content);
      }
      self.write_children(&element.children);
    }
    self.close_node();
  }

  fn write_attributes(&mut self, attributes: &[Attribute<'_>]) {
    if attributes.is_empty() {
      return;
    }
    self.begin_property("attributes");
    self.begin_array();
    for attribute in attributes {
      self.write_indent();
      self.text.push_str("{\n");
      self.indent += 1;
      self.property("name", attribute.name);
      if let Some(value) = &attribute.value {
        self.property("value", value.text);
        match value.quote {
          Some(quote) => self.property("quote", &quote.to_string()),
          None => self.raw_property("quoted", "false"),
        }
      }
      self.close_node();
      self.text.push_str(",\n");
    }
    self.end_array();
  }

  fn write_children(&mut self, children: &[Node<'_>]) {
    if children.is_empty() {
      return;
    }
    self.begin_property("children");
    self.begin_array();
    for child in children {
      self.write_indent();
      self.write_node(child);
      self.text.push_str(",\n");
    }
    self.end_array();
  }

  // ---- primitives ----

  fn open_node(&mut self, kind: &str, source: &str) {
    self.text.push_str("{\n");
    self.indent += 1;
    self.property("kind", kind);
    let start = offset_in(source, self.source);
    self.raw_property("span", &format!("[{}, {}]", start, start + source.len()));
    self.property("text", source);
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
    write_json_string(&mut self.text, name);
    self.text.push_str(": ");
  }

  fn property(&mut self, name: &str, value: &str) {
    self.begin_property(name);
    write_json_string(&mut self.text, value);
    self.text.push_str(",\n");
  }

  fn raw_property(&mut self, name: &str, value: &str) {
    self.begin_property(name);
    self.text.push_str(value);
    self.text.push_str(",\n");
  }

  fn write_indent(&mut self) {
    for _ in 0..self.indent {
      self.text.push_str("  ");
    }
  }

  fn trim_trailing_comma(&mut self) {
    if self.text.ends_with(",\n") {
      self.text.truncate(self.text.len() - 2);
      self.text.push('\n');
    }
  }
}

fn write_json_string(text: &mut String, value: &str) {
  text.push('"');
  for ch in value.chars() {
    match ch {
      '"' => text.push_str("\\\""),
      '\\' => text.push_str("\\\\"),
      '\n' => text.push_str("\\n"),
      '\r' => text.push_str("\\r"),
      '\t' => text.push_str("\\t"),
      ch if (ch as u32) < 0x20 => text.push_str(&format!("\\u{:04x}", ch as u32)),
      ch => text.push(ch),
    }
  }
  text.push('"');
}
