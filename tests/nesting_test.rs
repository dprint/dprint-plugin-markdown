//! Checks what the formatter does with a document that nests blocks far more
//! deeply than anything written by hand, which used to take the stack out from
//! under the process.

use dprint_plugin_markdown::configuration::*;
use dprint_plugin_markdown::*;

/// One less than the limit the parser holds, which has to keep working.
const DEEP: usize = 63;

#[test]
fn formats_a_document_nested_as_deeply_as_the_parser_allows() {
  let config = ConfigurationBuilder::new().build();
  for input in [
    ">".repeat(DEEP) + " a\n",
    "> ".repeat(DEEP) + "a\n",
    "- ".repeat(DEEP) + "a\n",
    "> - ".repeat(DEEP / 2) + "a\n",
  ] {
    let result = format_text(&input, &config, |_, _, _| Ok(None));
    assert!(result.is_ok(), "for {:?}", &input[..input.len().min(40)]);
  }
}

#[test]
fn refuses_a_document_nested_deeper_than_the_parser_follows() {
  let config = ConfigurationBuilder::new().build();
  for input in [
    ">".repeat(5000) + " a\n",
    "> ".repeat(5000) + "a\n",
    "- ".repeat(5000) + "a\n",
    "> - ".repeat(5000) + "a\n",
    ">".repeat(50000) + " a\n",
  ] {
    let err = format_text(&input, &config, |_, _, _| Ok(None))
      .expect_err(&format!("expected an error for {:?}", &input[..input.len().min(40)]));
    assert!(
      err.to_string().contains("nests blocks more than"),
      "unexpected error: {}",
      err
    );
  }
}
