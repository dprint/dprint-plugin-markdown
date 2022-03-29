extern crate dprint_development;
extern crate dprint_plugin_markdown;

use std::path::PathBuf;

use dprint_core::configuration::*;
use dprint_development::*;
use dprint_plugin_markdown::configuration::*;
use dprint_plugin_markdown::*;

#[test]
fn test_specs() {
  //debug_here!();
  let global_config = resolve_global_config(ConfigKeyMap::new(), &Default::default()).config;

  run_specs(
    &PathBuf::from("./tests/specs"),
    &ParseSpecOptions { default_file_name: "file.md" },
    &RunSpecsOptions {
      fix_failures: false,
      format_twice: true,
    },
    {
      let global_config = global_config.clone();
      move |_, file_text, spec_config| {
        let config_result = resolve_config(parse_config_key_map(spec_config), &global_config);
        ensure_no_diagnostics(&config_result.diagnostics);

        format_text(&file_text, &config_result.config, |tag, file_text, line_width| {
          let end = format!("_formatted_{}", line_width);
          if tag == "format" && !file_text.ends_with(&end) {
            Ok(Some(format!("{}{}\n\n", file_text.to_string(), end)))
          } else {
            Ok(None)
          }
        })
      }
    },
    move |_, _file_text, _spec_config| {
      #[cfg(feature = "tracing")]
      {
        let config_result = resolve_config(parse_config_key_map(_spec_config), &global_config);
        ensure_no_diagnostics(&config_result.diagnostics);
        return serde_json::to_string(&trace_file(&_file_text, &config_result.config, |tag, file_text, line_width| {
          let end = format!("_formatted_{}", line_width);
          if tag == "format" && !file_text.ends_with(&end) {
            Ok(Some(format!("{}{}\n\n", file_text.to_string(), end)))
          } else {
            Ok(None)
          }
        }))
        .unwrap();
      }

      #[cfg(not(feature = "tracing"))]
      panic!("\n====\nPlease run with `cargo test --features tracing` to get trace output\n====\n")
    },
  );
}
