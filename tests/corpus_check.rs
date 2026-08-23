use std::path::PathBuf;

#[test]
fn formats_a_corpus() {
  let Ok(root) = std::env::var("CORPUS") else { return };
  let mut files = Vec::new();
  collect(&PathBuf::from(root), &mut files);
  files.sort();
  let config = dprint_plugin_markdown::configuration::ConfigurationBuilder::new().build();
  let mut count = 0;
  std::panic::set_hook(Box::new(|_| {}));
  for file in &files {
    let Ok(text) = std::fs::read_to_string(file) else { continue };
    if text.len() > 400_000 { continue }
    count += 1;
    let once = match std::panic::catch_unwind(|| dprint_plugin_markdown::format_text(&text, &config, |_, _, _| Ok(None))) {
      Ok(Ok(out)) => out.unwrap_or_else(|| text.clone()),
      Ok(Err(e)) => { eprintln!("ERROR {} {}", file.display(), e); continue }
      Err(_) => { eprintln!("PANIC {}", file.display()); continue }
    };
    match std::panic::catch_unwind(|| dprint_plugin_markdown::format_text(&once, &config, |_, _, _| Ok(None))) {
      Ok(Ok(Some(twice))) if twice != once => eprintln!("NOTIDEMPOTENT {}", file.display()),
      Ok(Err(e)) => eprintln!("ERROR2 {} {}", file.display(), e),
      Err(_) => eprintln!("PANIC2 {}", file.display()),
      _ => {}
    }
  }
  eprintln!("checked {} files", count);
}

fn collect(dir: &PathBuf, files: &mut Vec<PathBuf>) {
  let Ok(entries) = std::fs::read_dir(dir) else { return };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      if !matches!(path.file_name().and_then(|n| n.to_str()), Some("node_modules") | Some(".git") | Some("target")) {
        collect(&path, files);
      }
    } else if path.extension().is_some_and(|e| e == "md") {
      files.push(path);
    }
  }
}
