// Hunts for text the parser drops, by feeding it markdown built out of markup
// pieces with words mixed in.
//
// Every word is alphanumeric and every markup piece is not, so the ast has to
// hold each word that went in: see [`super::spec_test::validate_text_coverage`].
// The generator is seeded, so a case that fails is reproduced by its number.
//
// `FUZZ_CASES=n` runs a longer sweep than the one the test itself does.

use super::spec_test::validate_text_coverage;

/// The pieces markdown is written with, none of them alphanumeric.
const MARKUP: &[&str] = &[
  "\n", "\n", "\n", " ", "  ", "    ", "\t", "- ", "* ", "+ ", "1. ", "2) ", "> ", ">", "# ", "###### ", "```", "~~~",
  "|", " | ", "---", ":--", "--:", "***", "___", "*", "_", "**", "__", "~~", "`", "``", "[", "]", "(", ")", "!", "<",
  ">", "\\", "[^", "]:", "[ ] ", "[x] ", "<!--", "-->", "&", ";", ":", "~", "^", "$", "$$", "+", "=", "\"", "'", ".",
  ",", "/", "%", "{", "}", "@", "<br>", "</br>", "<a>", "\r\n",
];

#[test]
fn no_text_is_dropped() {
  let count = match std::env::var("FUZZ_CASES") {
    Ok(count) => count.parse().unwrap(),
    Err(_) => 20_000,
  };
  let mut failures = Vec::new();
  for case in 0..count {
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
  let pieces = 1 + random.next(24) as usize;
  let mut source = String::new();
  let mut word = 0;
  for _ in 0..pieces {
    // a word every so often, so that the markup around it is what varies
    if random.next(3) == 0 {
      source.push_str(&format!("w{}", word));
      word += 1;
    } else {
      source.push_str(MARKUP[random.next(MARKUP.len() as u64) as usize]);
    }
  }
  source
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
