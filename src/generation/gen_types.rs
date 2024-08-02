use std::collections::HashMap;

use dprint_core::formatting::PrintItemPath;
use dprint_core::formatting::PrintItems;
use dprint_core::formatting::Signal;
use regex::Regex;

use super::utils::*;
use crate::configuration::Configuration;
use crate::format_text;
use anyhow::Result;

type FormatResult = Result<Option<String>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoizedRcPathKind {
  StartIndent(u32),
  StartWithSingleIndent(u32),
  FinishIndent(u32),
}

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
  memoized_rc_paths: HashMap<MemoizedRcPathKind, Option<PrintItemPath>>,
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
      memoized_rc_paths: HashMap::new(),
    }
  }

  pub fn get_memoized_rc_path(&mut self, kind: MemoizedRcPathKind) -> Option<PrintItemPath> {
    if let Some(path) = self.memoized_rc_paths.get(&kind) {
      *path
    } else {
      let mut items = PrintItems::new();
      match kind {
        MemoizedRcPathKind::StartIndent(times) => {
          for _ in 0..times {
            items.push_signal(Signal::StartIndent);
          }
        }
        MemoizedRcPathKind::StartWithSingleIndent(times) => {
          items.push_optional_path(self.get_memoized_rc_path(MemoizedRcPathKind::StartIndent(times)));
          for _ in 0..times {
            items.push_signal(Signal::SingleIndent);
          }
        }
        MemoizedRcPathKind::FinishIndent(times) => {
          for _ in 0..times {
            items.push_signal(Signal::FinishIndent);
          }
        }
      }
      let path = items.into_rc_path();
      self.memoized_rc_paths.insert(kind, path);
      path
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

    match tag {
      "markdown" | "md" => format_text(text, self.configuration, |tag, file_text, line_width| {
        (self.format_code_block_text)(tag, file_text, line_width)
      }),
      _ => (self.format_code_block_text)(tag, text, line_width),
    }
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
