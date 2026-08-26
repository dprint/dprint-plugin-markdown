//! Formats every `.md` file of a corpus twice over, reporting the ones that
//! panic, fail, or come out differently the second time.
//!
//! Set `CORPUS=<dir>` to walk a directory, or `CORPUS_LIST=<file>` to read the
//! paths from a file, one per line, which is what to use when walking the tree
//! costs more than the checking does.

#[test]
fn checks_a_corpus() {
  let Some(files) = corpus_files() else {
    return;
  };
  let config = dprint_plugin_markdown::configuration::ConfigurationBuilder::new().build();
  for path in &files {
    let Ok(text) = std::fs::read_to_string(path) else {
      continue;
    };
    let started = std::time::Instant::now();
    let once =
      match std::panic::catch_unwind(|| dprint_plugin_markdown::format_text(&text, &config, |_, _, _| Ok(None))) {
        Ok(Ok(result)) => result.unwrap_or_else(|| text.clone()),
        Ok(Err(err)) => {
          report(format_args!("ERROR {} {}", path.display(), err));
          continue;
        }
        Err(_) => {
          report(format_args!("PANIC {}", path.display()));
          continue;
        }
      };
    match std::panic::catch_unwind(|| dprint_plugin_markdown::format_text(&once, &config, |_, _, _| Ok(None))) {
      Ok(Ok(Some(_))) => report(format_args!("NOTIDEMPOTENT {}", path.display())),
      Ok(Ok(None)) => {}
      Ok(Err(err)) => report(format_args!("ERROR2 {} {}", path.display(), err)),
      Err(_) => report(format_args!("PANIC2 {}", path.display())),
    }
    // formatting decides how the text is written, never what it says, so every
    // letter that went in comes back out
    if written_letters(&text) != written_letters(&once) {
      report(format_args!("LOSTTEXT {}", path.display()));
    }
    let elapsed = started.elapsed();
    if elapsed.as_millis() > 200 {
      report(format_args!(
        "SLOW {} ({} bytes) {:?}",
        path.display(),
        text.len(),
        elapsed
      ));
    }
  }
  report(format_args!("checked {} files", files.len()));
}

fn corpus_files() -> Option<Vec<std::path::PathBuf>> {
  if let Ok(list) = std::env::var("CORPUS_LIST") {
    let text = std::fs::read_to_string(list).unwrap();
    return Some(text.lines().map(std::path::PathBuf::from).collect());
  }
  let root = std::env::var("CORPUS").ok()?;
  let mut files = Vec::new();
  collect(std::path::Path::new(&root), &mut files);
  Some(files)
}

fn collect(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
  let Ok(entries) = std::fs::read_dir(directory) else {
    return;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      collect(&path, files);
    } else if path.extension().is_some_and(|extension| extension == "md") {
      files.push(path);
    }
  }
}

/// The letters of the text in the order they are written, which is what
/// survives however the markup around them is written.
///
/// Digits are left out because a list's numbers are markup, which formatting
/// is free to write again from one.
fn written_letters(text: &str) -> String {
  text
    .chars()
    // formatting strips the byte order mark, so it is not a letter that has to come back
    .filter(|c| (c.is_alphabetic() || !c.is_ascii()) && *c != '\u{feff}')
    .flat_map(|c| c.to_lowercase())
    .collect()
}

fn report(message: std::fmt::Arguments) {
  use std::io::Write;
  let mut stderr = std::io::stderr();
  writeln!(stderr, "{}", message).unwrap();
  stderr.flush().unwrap();
}
