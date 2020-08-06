use dprint_plugin_markdown::configuration::*;
use dprint_plugin_markdown::*;

#[test]
fn test_issue22_with_carriage_return_line_feeds() {
    let config = ConfigurationBuilder::new().build();
    let result = format_text(
        &"```\r\ntest\r\n\r\ntest\r\n```\r\n",
        &config,
        Box::new(|_, file_text, _| Ok(file_text.to_string())),
    ).unwrap();
    assert_eq!(result, "```\ntest\n\ntest\n```\n");
}

#[test]
fn test_issue26_with_carriage_return_line_feeds() {
    let config = ConfigurationBuilder::new().build();
    let result = format_text(
        &"Testing:\r\n<!-- dprint-ignore -->\r\n```json\r\ntesting\r\n```\r\n",
        &config,
        Box::new(|_, file_text, _| Ok(file_text.to_string())),
    ).unwrap();
    assert_eq!(result, "Testing:\n\n<!-- dprint-ignore -->\n```json\ntesting\n```\n");
}
