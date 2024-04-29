extern crate dprint_development;
extern crate dprint_plugin_markdown;

use std::path::PathBuf;
use std::sync::Arc;

use dprint_core::configuration::*;
use dprint_development::*;
use dprint_plugin_markdown::configuration::*;
use dprint_plugin_markdown::*;

fn main() {
  //debug_here!();
  let global_config = GlobalConfiguration::default();

  run_specs(
    &PathBuf::from("./tests/specs"),
    &ParseSpecOptions {
      default_file_name: "file.md",
    },
    &RunSpecsOptions {
      fix_failures: false,
      format_twice: true,
    },
    {
      let global_config = global_config.clone();
      Arc::new(move |_, file_text, spec_config| {
        let spec_config: ConfigKeyMap = serde_json::from_value(spec_config.clone().into()).unwrap();
        let config_result = resolve_config(spec_config, &global_config);
        ensure_no_diagnostics(&config_result.diagnostics);

        format_text(&file_text, &config_result.config, |tag, file_text, line_width| {
          let end = format!("_formatted_{}", line_width);
          if tag == "format" && !file_text.ends_with(&end) {
            Ok(Some(format!("{}{}\n\n", file_text.to_string(), end)))
          } else {
            Ok(None)
          }
        })
      })
    },
    Arc::new(move |_, _file_text, _spec_config| {
      #[cfg(feature = "tracing")]
      {
        let spec_config: ConfigKeyMap = serde_json::from_value(_spec_config.clone().into()).unwrap();
        let config_result = resolve_config(spec_config, &global_config);
        ensure_no_diagnostics(&config_result.diagnostics);
        return serde_json::to_string(&trace_file(
          &_file_text,
          &config_result.config,
          |tag, file_text, line_width| {
            let end = format!("_formatted_{}", line_width);
            if tag == "format" && !file_text.ends_with(&end) {
              Ok(Some(format!("{}{}\n\n", file_text.to_string(), end)))
            } else {
              Ok(None)
            }
          },
        ))
        .unwrap();
      }

      #[cfg(not(feature = "tracing"))]
      panic!("\n====\nPlease run with `cargo test --features tracing` to get trace output\n====\n")
    }),
  );
}
