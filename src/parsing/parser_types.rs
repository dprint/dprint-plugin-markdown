use dprint_core::types::ErrBox;
use regex::Regex;

use super::utils::*;
use crate::configuration::Configuration;

pub struct Context<'a> {
  pub file_text: &'a str,
  pub configuration: &'a Configuration,
  /** The current indentation level of what's being printed out. */
  pub indent_level: u32,
  /** The current indentation level within the file being formatted. */
  pub raw_indent_level: u32,
  pub is_in_list_count: u32,
  pub format_code_block_text: Box<dyn FnMut(&str, &str, u32) -> Result<String, ErrBox> + 'a>,
  pub ignore_regex: Regex,
  pub ignore_start_regex: Regex,
  pub ignore_end_regex: Regex,
}

impl<'a> Context<'a> {
  pub fn new(
    file_text: &'a str,
    configuration: &'a Configuration,
    format_code_block_text: impl FnMut(&str, &str, u32) -> Result<String, ErrBox> + 'a,
  ) -> Context<'a> {
    Context {
      file_text,
      configuration,
      indent_level: 0,
      raw_indent_level: 0,
      is_in_list_count: 0,
      format_code_block_text: Box::new(format_code_block_text),
      ignore_regex: get_ignore_comment_regex(&configuration.ignore_directive),
      ignore_start_regex: get_ignore_comment_regex(&configuration.ignore_start_directive),
      ignore_end_regex: get_ignore_comment_regex(&configuration.ignore_end_directive),
    }
  }

  pub fn is_in_list(&self) -> bool {
    self.is_in_list_count > 0
  }

  pub fn format_text(&mut self, tag: &str, text: &str) -> Result<String, ErrBox> {
    let line_width = std::cmp::max(10, self.configuration.line_width as i32 - self.indent_level as i32) as u32;
    (self.format_code_block_text)(tag, text, line_width)
  }

  pub fn get_new_lines_in_range(&self, start: usize, end: usize) -> u32 {
    if end < start {
      return 0;
    } // ignore

    let file_bytes = self.file_text.as_bytes();
    let mut count = 0;
    for byte in &file_bytes[start..end] {
      if byte == &('\n' as u8) {
        count += 1;
      }
    }
    count
  }

  pub fn get_indent_level_at_pos(&self, pos: usize) -> u32 {
    let file_bytes = self.file_text.as_bytes();
    let mut count = 0;

    // get the whitespace at and after the pos
    for byte in file_bytes[pos..].iter() {
      let c = *byte as char;
      if c != '\r' && c != '\n' && c != '\t' && c.is_whitespace() {
        count += 1;
      } else {
        break;
      }
    }

    // get the whitespace before the pos
    for byte in file_bytes[0..pos].iter().rev() {
      // This is ok because we are just investigating whitespace chars
      // which I believe are only 1 byte.
      let character = *byte as char;

      if character == '\n' {
        break;
      }

      if character == '\t' {
        count += 4;
      } else if character.is_whitespace() {
        count += 1;
      } else {
        // todo: unexpected... I guess break?
        break;
      }
    }

    const INDENT_WIDTH: usize = 1;
    (count as f64 / INDENT_WIDTH as f64).round() as u32
  }
}
