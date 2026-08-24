use dprint_plugin_markdown::configuration::*;
use dprint_plugin_markdown::*;

#[test]
fn test_issue22_with_carriage_return_line_feeds() {
  let config = ConfigurationBuilder::new().build();
  let result = format_text("```\r\ntest\r\n\r\ntest\r\n```\r\n", &config, |_, _, _| Ok(None)).unwrap();
  assert_eq!(result.unwrap(), "```\ntest\n\ntest\n```\n");
}

#[test]
fn test_issue26_with_carriage_return_line_feeds() {
  let config = ConfigurationBuilder::new().build();
  let result = format_text(
    "Testing:\r\n<!-- dprint-ignore -->\r\n```json\r\ntesting\r\n```\r\n",
    &config,
    |_, _, _| Ok(None),
  )
  .unwrap();
  assert_eq!(
    result.unwrap(),
    "Testing:\n\n<!-- dprint-ignore -->\n```json\ntesting\n```\n"
  );
}

#[test]
fn test_issue35_convert_two_spaces_end_of_line_to_hard_break() {
  // In markdown, two spaces at the end of the line is a hard break.
  // To make this easier to see, we convert it to a hard break.
  let config = ConfigurationBuilder::new().build();
  let result = format_text("testing  \nasdf", &config, |_, _, _| Ok(None)).unwrap();
  assert_eq!(result.unwrap(), "testing\\\nasdf\n");
}

#[test]
fn test_issue35_ignore_two_spaces_before_hard_break() {
  let config = ConfigurationBuilder::new().build();
  let result = format_text("testing  \\\nasdf", &config, |_, _, _| Ok(None)).unwrap();
  assert_eq!(result.unwrap(), "testing\\\nasdf\n");
}

#[test]
fn test_non_breaking_spaces_with_whitespace() {
  let config = ConfigurationBuilder::new().build();
  let result = format_text("<foo>\u{a0} <foo>", &config, |_, _, _| Ok(None)).unwrap();
  assert_eq!(result.unwrap(), "<foo>\u{a0} <foo>\n");
}

#[test]
fn test_carriage_return_written_on_its_own_ends_a_line() {
  // markdown ends a line at a carriage return whether a newline follows it or
  // not, so one on its own is a line ending rather than text
  let config = ConfigurationBuilder::new().build();
  for (input, expected) in [
    ("a\r", "a\n"),
    ("a\rb", "a\nb\n"),
    ("a\r\r\n", "a\n"),
    ("# a\rb\r", "# a\n\nb\n"),
    ("- a\r- b\r", "- a\n- b\n"),
    ("a  \r\r\nb", "a\n\nb\n"),
  ] {
    let result = format_text(input, &config, |_, _, _| Ok(None)).unwrap();
    assert_eq!(result.unwrap(), expected, "for {:?}", input);
  }
}

#[test]
fn test_carriage_returns_settle_in_one_pass() {
  let config = ConfigurationBuilder::new().build();
  for input in [
    "\r",
    "a\r",
    "- Foo\r\r\n---\r\r\n",
    "  - foo\r\r\n\r\r\n    bar\r\n",
    "``\r\r\nfoo\r\r\nbar  ``\r\r\n",
  ] {
    let once = format_text(input, &config, |_, _, _| Ok(None))
      .unwrap()
      .unwrap_or_else(|| input.to_string());
    let twice = format_text(&once, &config, |_, _, _| Ok(None)).unwrap();
    assert_eq!(twice, None, "for {:?}, which became {:?}", input, once);
  }
}

#[test]
fn test_keeps_the_whitespace_that_is_text_of_the_line() {
  // markdown counts only a space or a tab as the whitespace written around its
  // markup, so everything else is text of the document
  let config = ConfigurationBuilder::new().build();
  for input in [
    "a\u{b}\n",
    "a\n\u{b}\nb\n",
    "\t\u{b}\t\n",
    "<div>\n*foo*\n\u{b}\n*bar*\n",
    "a\u{c}\n",
    "a\u{a0}\n",
    "a\u{2003}\n",
    "a\u{3000}\n",
  ] {
    let once = format_text(input, &config, |_, _, _| Ok(None))
      .unwrap()
      .unwrap_or_else(|| input.to_string());
    let kept = |text: &str| {
      text
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\r'))
        .collect::<String>()
    };
    assert_eq!(kept(&once), kept(input), "for {:?}, which became {:?}", input, once);
  }
}

#[test]
fn test_carriage_return_ends_a_row_of_a_table_left_as_written() {
  let config = ConfigurationBuilder::new().table_skip_format(true).build();
  for (input, expected) in [
    ("|a|b|\r|-|-|\r|c|d|\r", "|a|b|\n|-|-|\n|c|d|\n"),
    ("> |a|b|\r> |-|-|\r> |c|d|\r", "> |a|b|\n> |-|-|\n> |c|d|\n"),
  ] {
    let result = format_text(input, &config, |_, _, _| Ok(None)).unwrap();
    assert_eq!(result.unwrap(), expected, "for {:?}", input);
  }
}
