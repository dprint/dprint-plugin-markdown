#[test]
fn d() {
  let config = dprint_plugin_markdown::configuration::ConfigurationBuilder::new().build();
  for input in [
    "a_*a*\n",
    "*a*_a\n",
    "**_*\n",
    "**_***a*\n",
    "__x_*_\n",
    "_*a**_\n",
    "***_***\n",
    "*_*_\n",
    "*_foo_*\n",
    "foo *_*\n",
    "foo __*__\n",
    "foo***bar***baz\n",
    "a *b* c\n",
    "*foo __bar *baz bim__ bam*\n",
  ] {
    let once = dprint_plugin_markdown::format_text(input, &config, |_, _, _| Ok(None)).unwrap().unwrap_or_else(|| input.to_string());
    let twice = dprint_plugin_markdown::format_text(&once, &config, |_, _, _| Ok(None)).unwrap();
    let mark = if twice.is_none() { "" } else { " [NOT IDEMPOTENT]" };
    eprintln!("D {:?} -> {:?}{}", input, once, mark);
  }
}
