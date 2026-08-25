//! Runs the html parser's spec tests, one per file under
//! `tests/html_parser_specs`, with a sub test for each case the file holds.
//! See [`html::spec_test`] for what a case looks like and what is checked of
//! it.
//!
//! Set `UPDATE=1` to rewrite the files with what the parser currently
//! produces, which is how a new case is filled in. Always read the result
//! before committing it.

// the html parser is not part of what the crate hands out, so the tests that
// read its ast are built with it rather than against it
#[path = "../src/html/mod.rs"]
#[allow(unused_imports, dead_code)]
mod html;

use std::path::PathBuf;

use file_test_runner::collect_and_run_tests;
use file_test_runner::collection::strategies::TestPerFileCollectionStrategy;
use file_test_runner::collection::CollectOptions;
use file_test_runner::RunOptions;
use file_test_runner::TestResult;

use html::spec_test::parse_spec_file;
use html::spec_test::run_case;
use html::spec_test::SpecCase;

fn main() {
  collect_and_run_tests(
    CollectOptions {
      base: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/html_parser_specs"),
      strategy: Box::new(TestPerFileCollectionStrategy { file_pattern: None }),
      filter_override: None,
    },
    RunOptions::default(),
    run_file,
  );
}

fn run_file(test: &file_test_runner::collection::CollectedTest) -> TestResult {
  let path = test.path.clone();
  TestResult::from_maybe_panic(move || {
    let file_text = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
    let cases = parse_spec_file(&file_text, &path);
    assert!(!cases.is_empty(), "no cases in {}", path.display());

    if std::env::var("UPDATE").is_ok() {
      let updated = cases
        .iter()
        .map(|case| {
          let outcome = run_case(case);
          assert!(
            outcome.failures.is_empty(),
            "{}: {}",
            case.name,
            outcome.failures.join("\n")
          );
          format!("{}\n{}\n[[ast]]\n{}", case.header(), case.input, outcome.actual)
        })
        .collect::<String>();
      std::fs::write(&path, updated).unwrap();
      return;
    }

    let failures = cases.iter().filter_map(check_case).collect::<Vec<_>>();
    assert!(
      failures.is_empty(),
      "{} of {} cases failed:\n\n{}\n\nRe-run with UPDATE=1 to accept the new output.",
      failures.len(),
      cases.len(),
      failures.join("\n\n"),
    );
  })
}

fn check_case(case: &SpecCase) -> Option<String> {
  let outcome = run_case(case);
  if !outcome.failures.is_empty() {
    return Some(format!("case: {}\n  {}", case.name, outcome.failures.join("\n  ")));
  }
  if outcome.actual != case.expected {
    return Some(format!(
      "case: {}\n  --- expected ---\n{}\n  --- actual ---\n{}",
      case.name, case.expected, outcome.actual,
    ));
  }
  None
}
