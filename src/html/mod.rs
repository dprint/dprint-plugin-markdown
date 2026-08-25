//! Formatting for the html written within a markdown file.
//!
//! Html in markdown is not a document -- it's whatever fragment of one fits
//! between the blank lines that close an html block -- so the parser refuses
//! anything it can't put back together and the printer leaves that text as it
//! was written. Nothing here ever adds or removes a tag.

mod ast;
mod parser;
mod printer;
mod tags;

#[cfg(test)]
mod debug_json;
#[cfg(test)]
pub mod spec_test;
#[cfg(test)]
mod text_fuzz;

pub use printer::format_html;
pub use printer::HtmlFormatOptions;
