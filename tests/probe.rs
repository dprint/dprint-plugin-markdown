#[test]
fn probe() {
  let config = dprint_plugin_markdown::configuration::ConfigurationBuilder::new().build();
  let cases = [
    ("issue 37", "| name                                 |                   description                              |\n| ------------------------ | ------------------------------ |\n| `get(key: name): string | null` | Get the value of the string.             |\n"),
    ("excess cells", "| a | b |\n| - | - |\n| 1 | 2 | 3 |\n"),
    ("fewer cells", "| a | b |\n| - | - |\n| 1 |\n"),
    ("excess in header", "| a | b | c |\n| - | - |\n| 1 | 2 |\n"),
  ];
  for (name, input) in cases {
    let once = dprint_plugin_markdown::format_text(input, &config, |_, _, _| Ok(None)).unwrap().unwrap_or_else(|| input.to_string());
    let twice = dprint_plugin_markdown::format_text(&once, &config, |_, _, _| Ok(None)).unwrap();
    let mark = if twice.is_none() { "" } else { "  [NOT IDEMPOTENT]" };
    eprintln!("== {name}{mark}
--- in
{}--- once
{}--- twice
{:?}", input, once, twice);
  }
}
