//! Hunts for text the parser drops, by feeding it markdown built out of the
//! pieces markdown is written with, with words mixed in.
//!
//! Every word is alphanumeric and every markup piece is not, so the ast has to
//! hold each word that went in: see [`super::spec_test::validate_text_coverage`].
//! The generator is seeded, so a case that fails is reproduced by its number.
//!
//! Whole lines are built at a time, out of a prefix (what makes a line a list
//! item, a quote, a heading) and a body, because that is the shape a block
//! parser reads: a run of random characters rarely lands on a table or a list,
//! while a run of random lines lands on one often.
//!
//! `FUZZ_CASES=n` runs a longer sweep than the test itself does, and
//! `FUZZ_FROM=n` starts it at a given case.

use super::spec_test::validate_text_coverage;

/// What can begin a line.
const PREFIXES: &[&str] = &[
  "",
  "",
  "",
  " ",
  "  ",
  "   ",
  "    ",
  "\t",
  "- ",
  "* ",
  "+ ",
  "- [ ] ",
  "- [x] ",
  "> ",
  ">",
  "> > ",
  "# ",
  "## ",
  "###### ",
  "  - ",
  "    - ",
  ": ",
  "~ ",
  "|",
  "[^",
  "[^n1]: ",
  "[r1]: ",
  "  [r1]: /u \"t\"",
];

/// What can make up the body of a line.
const PIECES: &[&str] = &[
  " ",
  "  ",
  "\t",
  "|",
  " | ",
  " |",
  "| ",
  "---",
  "--- | ---",
  ":-- | --:",
  "***",
  "___",
  "===",
  "*",
  "_",
  "**",
  "__",
  "~~",
  "`",
  "``",
  "```",
  "~~~",
  "$",
  "$$",
  "[",
  "]",
  "[]",
  "()",
  "(",
  ")",
  "!",
  "<",
  ">",
  "\\",
  "]:",
  ":",
  ";",
  "&",
  "~",
  "^",
  "+",
  "=",
  "\"",
  "'",
  ".",
  ",",
  "/",
  "%",
  "{",
  "}",
  "@",
  "<br>",
  "</br>",
  "<a>",
  "</a>",
  "<!--",
  "-->",
  "<?",
  "?>",
  "<!",
  "&amp;",
  "&#35;",
  "://",
  "-",
  "#",
  "](",
  "][",
  "![",
  "]: ",
  "[^n1]",
  "<http://a.b>",
  "<a@b.c>",
  "&#x41;",
  "&nbsp;",
  "```rust",
  "~~~js",
  "e",
  // the characters that only look like whitespace, which markdown reads as the
  // text they are rather than as a space
  "\u{a0}",
  "\u{2003}",
  "\u{2028}",
  "\u{3000}",
  "\u{200b}",
  "\u{feff}",
  "\u{e9}",
  "\u{4e2d}",
  "\u{1f600}",
];

/// The numbers a line may be marked with, which are markup rather than text.
const NUMBERS: &[&str] = &["1. ", "2. ", "1) ", "10. "];

/// Whole lines that take more than a lucky draw to land on, so that the blocks
/// they make up are reached often rather than once in a great many cases.
const LINES: &[&str] = &[
  "| --- | --- |",
  "| :-- | --: |",
  "| --- |",
  "| a | b |",
  "---",
  "===",
  "```",
  "~~~",
  "$$",
  "<div>",
  "</div>",
  "",
  "[r1]: /u",
  "[^n1]: note",
  "    code",
  "\ttab",
];

#[test]
fn no_text_is_dropped() {
  let first = env_count("FUZZ_FROM").unwrap_or(0);
  let count = env_count("FUZZ_CASES").unwrap_or(50_000);
  let mut failures = Vec::new();
  for case in first..first + count {
    let source = generate(case);
    let file = super::parse(&source);
    if let Err(message) = validate_text_coverage(&file, &source) {
      failures.push(format!("case {}: {:?}\n  {}", case, source, message));
      if failures.len() >= 10 {
        break;
      }
    }
  }

  assert!(
    failures.is_empty(),
    "the parser dropped text in {} of {} generated cases:\n\n{}",
    failures.len(),
    count,
    failures.join("\n\n"),
  );
}

/// Builds the markdown of the numbered case, which is the same every run.
fn generate(case: u64) -> String {
  let mut random = Random::new(case);
  let lines = 1 + random.next(6);
  let mut source = String::new();
  let mut word = 0;

  for _ in 0..lines {
    // a line that would rarely be landed on by chance
    if random.next(4) == 0 {
      source.push_str(LINES[random.next(LINES.len() as u64) as usize]);
      source.push('\n');
      continue;
    }
    // a numbered marker is markup that holds digits, so it only belongs where
    // a line can begin
    if random.next(8) == 0 {
      source.push_str(NUMBERS[random.next(NUMBERS.len() as u64) as usize]);
    } else {
      source.push_str(PREFIXES[random.next(PREFIXES.len() as u64) as usize]);
    }

    for _ in 0..random.next(8) {
      // a word every so often, so that it is the markup around it that varies
      if random.next(3) == 0 {
        source.push('w');
        source.push_str(&word.to_string());
        word += 1;
      } else {
        source.push_str(PIECES[random.next(PIECES.len() as u64) as usize]);
      }
    }
    source.push('\n');
  }
  source
}

fn env_count(name: &str) -> Option<u64> {
  std::env::var(name).ok().map(|count| count.parse().unwrap())
}

/// A seeded generator, so that the numbered case is always the same markdown.
struct Random(u64);

impl Random {
  fn new(seed: u64) -> Random {
    // an odd number well away from zero, so that the low seeds don't all start
    // out alike
    Random(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407))
  }

  fn next(&mut self, bound: u64) -> u64 {
    self.0 ^= self.0 << 13;
    self.0 ^= self.0 >> 7;
    self.0 ^= self.0 << 17;
    self.0 % bound
  }
}
