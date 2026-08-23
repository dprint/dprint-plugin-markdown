use std::path::PathBuf;

#[test]
fn dumps_a_corpus() {
  let root = PathBuf::from(std::env::var("CORPUS").unwrap());
  let out = PathBuf::from(std::env::var("CORPUS_OUT").unwrap());
  let mut files = Vec::new();
  collect(&root, &mut files);
  files.sort();
  let config = dprint_plugin_markdown::configuration::ConfigurationBuilder::new().build();
  let mut result = String::new();
  std::panic::set_hook(Box::new(|_| {}));
  for file in &files {
    let Ok(text) = std::fs::read_to_string(file) else { continue };
    if text.len() > 400_000 { continue }
    let formatted = match std::panic::catch_unwind(|| {
      dprint_plugin_markdown::format_text(&text, &config, |_, _, _| Ok(None))
    }) {
      Ok(Ok(Some(formatted))) => formatted,
      Ok(Ok(None)) => text.clone(),
      Ok(Err(err)) => format!("<<error: {}>>\n", err),
      Err(_) => "<<panic>>\n".to_string(),
    };
    result.push_str(&format!("===== {}\n{}", file.strip_prefix(&root).unwrap().display(), formatted));
  }
  std::fs::write(out, result).unwrap();
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
