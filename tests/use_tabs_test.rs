//! Checks that writing indentation with a tab says the same thing as writing
//! it with spaces.
//!
//! Every spec's text is run through the formatter with `useTabs` added to
//! whatever configuration it already sets, and what comes out is written back
//! with the option off. A tab that didn't reach the column the content has to begin at
//! would be read back as something other than the block it was written for,
//! which shows up here as text that no longer matches what the formatter
//! writes with spaces alone.

use std::path::Path;
use std::path::PathBuf;

use dprint_core::configuration::ConfigKeyMap;
use dprint_development::parse_specs;
use dprint_development::ParseSpecOptions;
use dprint_development::SpecConfigMap;
use dprint_plugin_markdown::configuration::*;
use dprint_plugin_markdown::*;

#[test]
fn writes_the_specs_with_tabs_without_changing_what_they_say() {
  for path in spec_files(Path::new("./tests/specs")) {
    let file_text = std::fs::read_to_string(&path).unwrap();
    let specs = parse_specs(
      file_text,
      &ParseSpecOptions {
        default_file_name: "file.md",
      },
    );
    for spec in specs.iter().filter(|spec| !spec.skip) {
      let tabs = resolve(&spec.config, true);
      let spaces = resolve(&spec.config, false);
      check(
        &spec.file_text,
        &tabs,
        &spaces,
        &path,
        &spec.message,
        !spec.skip_format_twice,
      );
    }
  }
}

fn check(text: &str, tabs: &Configuration, spaces: &Configuration, path: &Path, message: &str, check_twice: bool) {
  let at = format!("{} ({})", message, path.display());
  let with_spaces = format(text, spaces, &at);
  // a document the formatter doesn't settle on with spaces says nothing about
  // what it does with tabs
  if format(&with_spaces, spaces, &at) != with_spaces {
    return;
  }

  let with_tabs = format(text, tabs, &at);
  if check_twice {
    assert_eq!(
      format(&with_tabs, tabs, &at),
      with_tabs,
      "not idempotent with tabs in {}",
      at
    );
  }
  assert_eq!(
    format(&with_tabs, spaces, &at),
    with_spaces,
    "tabs changed what the text says in {}",
    at
  );
  assert_eq!(
    trailing_whitespace_lines(&with_tabs),
    trailing_whitespace_lines(&with_spaces),
    "tabs left whitespace at the end of a line in {}",
    at
  );
}

fn format(text: &str, config: &Configuration, at: &str) -> String {
  match format_text(text, config, |_, _, _| Ok(None)) {
    Ok(result) => result.unwrap_or_else(|| text.to_string()),
    Err(err) => panic!("failed to format {}: {}", at, err),
  }
}

/// Which lines end with whitespace, so that a tab written where nothing
/// follows it on the line is told from one that indents something.
fn trailing_whitespace_lines(text: &str) -> Vec<usize> {
  text
    .lines()
    .enumerate()
    .filter(|(_, line)| line.ends_with([' ', '\t']))
    .map(|(index, _)| index)
    .collect()
}

fn resolve(spec_config: &SpecConfigMap, use_tabs: bool) -> Configuration {
  let mut spec_config = spec_config.clone();
  // a spec that sets the option itself would otherwise leave the two
  // configurations the same, and nothing here would be comparing anything
  if use_tabs {
    spec_config.insert("useTabs".to_string(), true.into());
  } else {
    spec_config.remove("useTabs");
  }
  let config: ConfigKeyMap = serde_json::from_value(serde_json::Value::Object(spec_config)).unwrap();
  let result = resolve_config(config, &Default::default());
  assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
  result.config
}

fn spec_files(dir: &Path) -> Vec<PathBuf> {
  let mut paths = Vec::new();
  for entry in std::fs::read_dir(dir).unwrap() {
    let path = entry.unwrap().path();
    if path.is_dir() {
      paths.extend(spec_files(&path));
    } else if path.extension().is_some_and(|ext| ext == "txt") {
      paths.push(path);
    }
  }
  paths
}
