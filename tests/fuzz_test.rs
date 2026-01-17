use dprint_plugin_markdown::configuration::ConfigurationBuilder;
use dprint_plugin_markdown::format_text;
use quickcheck::TestResult;
use quickcheck_macros::quickcheck;

#[quickcheck]
#[ignore] // Due to randomness, this can lead to sporadic errors.
fn check_idempotency(input: String) -> TestResult {
  if !input.chars().all(|c| c.is_ascii_graphic() || c == '\n' || c == '\r') {
    // Non-graphical characters (like NUL bytes) are known to break
    // idempotency.
    return TestResult::discard();
  }

  TestResult::from_bool(is_idempotent(&input))
}

#[test]
fn test_simple() {
  assert!(is_idempotent("Lorem ipsum\n\n\nFoo bar"));
}

#[test]
fn test_list() {
  assert!(is_idempotent("* Lorem ipsum\n* Foo bar"));
}

#[test]
fn test_tab() {
  assert!(is_idempotent("\t"));
}

#[test]
#[should_panic]
fn test_vertical_tab() {
  assert!(is_idempotent("*\u{000b}"));
}

#[test]
fn test_nul_tab() {
  assert!(is_idempotent("\0\t\0"));
}

#[test]
fn test_paragraph_blockquote() {
  assert!(is_idempotent("Lorem ipsum.\n>"));
}

/// Can `input` be format twice and give the same result?
fn is_idempotent(input: &str) -> bool {
  let config = ConfigurationBuilder::new().build();

  let format = |input| format_text(input, &config, |_, _, _| Ok(None));

  let Ok(Some(text1)) = format(input) else {
    // Input was already formatted, or we cannot format this input.
    return true;
  };

  match format(&text1) {
    Ok(None) => true, // Reached fix point => success.
    Ok(Some(text2)) => {
      // Second pass changed something => failure.
      eprintln!("Input: {input:?}");
      eprintln!("Pass 1: {text1:?}");
      eprintln!("Pass 2: {text2:?}");
      false
    }
    Err(err) => {
      // Second pass failed => failure.
      eprintln!("Input: {input:?}");
      eprintln!("Pass 1: {text1:?}");
      eprintln!("Pass 2: {err}");
      false
    }
  }
}
