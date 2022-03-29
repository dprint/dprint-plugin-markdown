use dprint_core::plugins::FormatResult;
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
  is_in_list_count: u32,
  text_wrap_disabled_count: u32,
  pub format_code_block_text: Box<dyn for<'b> FnMut(&str, &'b str, u32) -> FormatResult + 'a>,
  pub ignore_regex: Regex,
  pub ignore_start_regex: Regex,
  pub ignore_end_regex: Regex,
}

impl<'a> Context<'a> {
  pub fn new(
    file_text: &'a str,
    configuration: &'a Configuration,
    format_code_block_text: impl for<'b> FnMut(&str, &'b str, u32) -> FormatResult + 'a,
  ) -> Context<'a> {
    Context {
      file_text,
      configuration,
      indent_level: 0,
      raw_indent_level: 0,
      is_in_list_count: 0,
      text_wrap_disabled_count: 0,
      format_code_block_text: Box::new(format_code_block_text),
      ignore_regex: get_ignore_comment_regex(&configuration.ignore_directive),
      ignore_start_regex: get_ignore_comment_regex(&configuration.ignore_start_directive),
      ignore_end_regex: get_ignore_comment_regex(&configuration.ignore_end_directive),
    }
  }

  pub fn mark_in_list<T>(&mut self, func: impl FnOnce(&mut Context) -> T) -> T {
    self.is_in_list_count += 1;
    let items = func(self);
    self.is_in_list_count -= 1;
    items
  }

  pub fn is_in_list(&self) -> bool {
    self.is_in_list_count > 0
  }

  pub fn with_no_text_wrap<T>(&mut self, func: impl FnOnce(&mut Context) -> T) -> T {
    self.text_wrap_disabled_count += 1;
    let items = func(self);
    self.text_wrap_disabled_count -= 1;
    items
  }

  pub fn is_text_wrap_disabled(&self) -> bool {
    self.text_wrap_disabled_count > 0
  }

  pub fn format_text<'b>(&mut self, tag: &str, text: &'b str) -> FormatResult {
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
      if byte == &b'\n' {
        count += 1;
      }
    }
    count
  }
}
