//! Hunts for html the formatter reads wrongly, by feeding it html built out of
//! the pieces html is written with, with words mixed in.
//!
//! Four things have to hold of every fragment the parser reads, and the
//! formatter would be wrong without each of them:
//!
//! - the ast accounts for every byte of it, so nothing was read and dropped
//!   (see [`super::spec_test::validate_coverage`])
//! - the text that went in comes back out, since formatting decides how html
//!   is written and never what it says
//! - the tags that went in come back out, in the same order and with the same
//!   attributes: the formatter never adds or removes one
//! - what is written out is written the same way the second time, and holds no
//!   blank line that would close the html block it sits in
//!
//! Html the parser refuses is not a failure -- an html block in markdown is
//! very often a fragment, and refusing it is what leaves it as it was written.
//!
//! A tree is generated rather than a run of tags, because html drawn a tag at
//! a time is almost never balanced and so is almost always refused: what the
//! parser reads has to be reached often for the printer to be given anything
//! to lay out. Malformed html is written in every so often, so that what is
//! refused is reached as well.
//!
//! The generator is seeded, so a case that fails is reproduced by its number.
//! `FUZZ_CASES=n` runs a longer sweep than the test itself does, and
//! `FUZZ_FROM=n` starts it at a given case.

use super::ast::Document;
use super::ast::Node;
use super::printer::format_html;
use super::printer::HtmlFormatOptions;
use super::spec_test::validate_coverage;

/// The elements to build a tree out of, which cover each way the printer lays
/// one out: as a block, inline, void, character data, or preformatted.
const ELEMENTS: &[&str] = &[
  "div",
  "p",
  "section",
  "table",
  "tr",
  "td",
  "ul",
  "li",
  "details",
  "summary",
  "h1",
  "blockquote",
  "span",
  "a",
  "em",
  "b",
  "code",
  "small",
  "my-widget",
  "svg",
  "g",
  "pre",
  "textarea",
  "script",
  "style",
  "title",
  "br",
  "hr",
  "img",
  "input",
  "DIV",
  "Span",
];

/// What can be written in an open tag after its name.
const ATTRIBUTES: &[&str] = &[
  "",
  "",
  "",
  " class=\"a\"",
  " align=\"center\"",
  " class=\"a b c\" id=\"i\" data-value=\"v\"",
  " href='u' title='t'",
  " hidden",
  " class=\"\"",
  " class=\"one\ntwo\"",
  " disabled type=\"submit\"",
  " @click=\"a\" :value=\"b\"",
  " src=a.png",
  "\n  class=\"a\"\n  id=\"i\"\n",
  " class=\"a-long-list-of-names that-goes-past the-line-width easily indeed\"",
];

/// The text and markup that can sit between elements.
const PIECES: &[&str] = &[
  " ",
  "  ",
  "\t",
  "\n",
  "\n  ",
  "<!-- c -->",
  "<!--\nc\n-->",
  "<!-- <div> -->",
  "<!DOCTYPE html>",
  "<?php echo 1; ?>",
  "<![CDATA[ a < b ]]>",
  "<!-- dprint-ignore -->",
  "a < b",
  "<",
  "<3",
  "&amp;",
  "&nbsp;",
  "\"",
  "'",
  "=",
  "-",
  ".",
  // the characters that only look like whitespace, which html renders as the
  // text they are rather than as a space
  "\u{a0}",
  "\u{2003}",
  "\u{3000}",
  "\u{200b}",
  "\u{feff}",
  "\u{e9}",
  "\u{4e2d}",
  "\u{1f600}",
];

/// What can be written within an element whose content is character data.
const RAW_TEXT: &[&str] = &[
  "",
  "const a = 1",
  "if (a < b) { }",
  "const a = \"</div>\"",
  ".a {   color: red }",
  "  keep   this  ",
  "a\n  b",
];

/// The html to write in place of a well formed tree every so often, so that
/// what the parser refuses is reached as well as what it reads.
const MALFORMED: &[&str] = &[
  "</div>",
  "<div>",
  "<div/>",
  "<span/>",
  "<div =foo>",
  "<ul><li>a<li>b</ul>",
  "<div><p>a</div></p>",
  "<!-- unterminated",
  "<p title=\"a",
  "<script>a",
  "</ div>",
  "</",
  "/>",
  "<>",
];

#[test]
fn html_is_read_and_written_back_out_whole() {
  let first = env_count("FUZZ_FROM").unwrap_or(0);
  let count = env_count("FUZZ_CASES").unwrap_or(50_000);
  let mut failures = Vec::new();
  let mut laid_out = 0;

  for case in first..first + count {
    let source = generate(case);
    // html the parser refuses is left as it was written, so there is nothing
    // to check of it
    let Ok(document) = super::parser::parse(&source) else {
      continue;
    };
    if let Err(message) = validate_coverage(&document, &source) {
      failures.push(format!("case {}: {:?}\n  {}", case, source, message));
    } else {
      match check_printing(&source, &document) {
        Ok(was_laid_out) => laid_out += u64::from(was_laid_out),
        Err(message) => failures.push(format!("case {}: {:?}\n  {}", case, source, message)),
      }
    }
    if failures.len() >= 10 {
      break;
    }
  }

  assert!(
    failures.is_empty(),
    "the formatter was wrong about {} of {} generated cases:\n\n{}",
    failures.len(),
    count,
    failures.join("\n\n"),
  );
  // Leaving html as it was written is always allowed, so a formatter that did
  // nothing at all would hold every invariant above. Most of what is generated
  // is html the printer has something to say about, and if that stops being so
  // then it is the generator or the printer that has gone wrong rather than
  // the html.
  assert!(
    laid_out * 2 > count,
    "only {} of {} generated cases were laid out, so most of them went unchecked",
    laid_out,
    count,
  );
}

/// Checks what has to hold of the html the printer writes out, giving back
/// whether it had anything to say about it at all.
fn check_printing(source: &str, document: &Document<'_>) -> Result<bool, String> {
  let options = HtmlFormatOptions {
    line_width: 40,
    use_tabs: false,
    indent_width: 2,
    self_closing_space: true,
  };
  // html the printer won't lay out is left as it was written, the same as html
  // the parser refuses
  let Ok(printed) = format_html(source, &options) else {
    return Ok(false);
  };

  if written_letters(&printed) != written_letters(source) {
    return Err(format!(
      "was written out as {:?}, which doesn't say the same thing",
      printed
    ));
  }
  // A blank line closes the html block the text sits in, which would leave the
  // rest of it to be read as markdown. [`format_html`] refuses to hand back
  // text that gains one, but only where the text it was given held none, so
  // what is checked here is the stronger thing the printer holds to: it writes
  // out the blank lines that were written and never one that wasn't.
  if blank_lines(&printed) > blank_lines(source) {
    return Err(format!(
      "was written out with {} blank lines where {} were written: {:?}",
      blank_lines(&printed),
      blank_lines(source),
      printed,
    ));
  }

  let reparsed = super::parser::parse(&printed).map_err(|err| {
    format!(
      "was written out as html the parser won't read: {}\n  {:?}",
      err, printed
    )
  })?;
  if tags(&reparsed.children) != tags(&document.children) {
    return Err(format!(
      "came out with tags that aren't what went in: {:?}\n  in:  {:?}\n  out: {:?}",
      printed,
      tags(&document.children),
      tags(&reparsed.children),
    ));
  }

  let again = format_html(&printed, &options)
    .map_err(|err| format!("was written out as html that can't be written again: {}", err))?;
  if again != printed {
    return Err(format!(
      "came out as {:?} and then as {:?}, so formatting it doesn't settle",
      printed, again
    ));
  }
  Ok(true)
}

/// The tags the nodes were written with, in the order they were written, which
/// is what the formatter has to keep whatever it does to the whitespace.
fn tags(nodes: &[Node<'_>]) -> Vec<String> {
  let mut found = Vec::new();
  collect_tags(nodes, &mut found);
  found
}

fn collect_tags(nodes: &[Node<'_>], found: &mut Vec<String>) {
  for node in nodes {
    let Node::Element(element) = node else {
      continue;
    };
    let attributes: String = element
      .attributes
      .iter()
      .map(|attribute| match &attribute.value {
        Some(value) => format!(" {}={}", attribute.name.to_lowercase(), value.text),
        None => format!(" {}", attribute.name.to_lowercase()),
      })
      .collect();
    found.push(format!("{}{}", element.name.to_lowercase(), attributes));
    collect_tags(&element.children, found);
  }
}

/// The letters and digits of the text, which say what it says however it is
/// written. The whitespace around them is what the formatter decides, so it is
/// left out.
fn written_letters(text: &str) -> String {
  text
    .chars()
    .filter(|c| c.is_alphanumeric() || !c.is_ascii())
    .flat_map(|c| c.to_lowercase())
    .collect()
}

/// How many lines of the text hold nothing but whitespace, which is what
/// closes an html block in markdown.
///
/// The empty line that text ending in a newline leaves behind isn't one of
/// them, because there is no line there at all.
fn blank_lines(text: &str) -> usize {
  let mut lines: Vec<&str> = text.split('\n').collect();
  if lines.last() == Some(&"") {
    lines.pop();
  }
  lines
    .iter()
    .filter(|line| line.trim_matches([' ', '\t', '\r', '\u{000C}']).is_empty())
    .count()
}

/// Builds the html of the numbered case, which is the same every run.
fn generate(case: u64) -> String {
  let mut random = Random::new(case);
  let mut source = String::new();
  let mut word = 0;
  let count = 1 + random.next(3);
  for _ in 0..count {
    write_node(&mut source, &mut random, &mut word, 0);
  }
  source
}

fn write_node(source: &mut String, random: &mut Random, word: &mut u64, depth: u64) {
  // html the parser refuses, so that leaving text as it was written is reached
  // as often as laying it out is
  if random.next(12) == 0 {
    source.push_str(MALFORMED[random.next(MALFORMED.len() as u64) as usize]);
    return;
  }
  // the tree has to stop somewhere, and its leaves are the text and the markup
  // that is written back out as it was
  if depth >= 4 || random.next(3) == 0 {
    if random.next(2) == 0 {
      source.push('w');
      source.push_str(&word.to_string());
      *word += 1;
    } else {
      source.push_str(PIECES[random.next(PIECES.len() as u64) as usize]);
    }
    return;
  }

  let name = ELEMENTS[random.next(ELEMENTS.len() as u64) as usize];
  source.push('<');
  source.push_str(name);
  source.push_str(ATTRIBUTES[random.next(ATTRIBUTES.len() as u64) as usize]);

  // a void element is written with no closing tag at all
  if is_void(name) {
    match random.next(3) {
      0 => source.push_str("/>"),
      1 => source.push_str(" />"),
      _ => source.push('>'),
    }
    return;
  }
  source.push('>');

  // the content of these is character data, which no tree is written into
  if is_raw_text(name) {
    source.push_str(RAW_TEXT[random.next(RAW_TEXT.len() as u64) as usize]);
  } else {
    for _ in 0..random.next(4) {
      write_node(source, random, word, depth + 1);
    }
  }

  source.push_str("</");
  source.push_str(name);
  source.push('>');
}

fn is_void(name: &str) -> bool {
  matches!(name.to_lowercase().as_str(), "br" | "hr" | "img" | "input")
}

fn is_raw_text(name: &str) -> bool {
  matches!(name.to_lowercase().as_str(), "script" | "style" | "textarea" | "title")
}

fn env_count(name: &str) -> Option<u64> {
  std::env::var(name).ok().map(|count| count.parse().unwrap())
}

/// A seeded generator, so that the numbered case is always the same html.
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

/// Hunts for html that changes what the markdown around it says, by putting
/// the generated html in a document and formatting that.
///
/// The html sweep above works the fragment on its own, where the only thing
/// that can go wrong is what the printer writes. In a document there is more:
/// an html block ends where markdown says it does, so text moved onto a line
/// past what closes the block stops being html and starts being markdown, and
/// text moved onto the line of a marker starts being html. Neither shows up
/// until the document is read again.
#[test]
fn markdown_holding_html_is_written_back_out_whole() {
  let first = env_count("FUZZ_FROM").unwrap_or(0);
  let count = env_count("FUZZ_CASES").unwrap_or(50_000);
  let config = crate::configuration::ConfigurationBuilder::new().build();
  let mut failures = Vec::new();

  for case in first..first + count {
    let html = generate(case);
    // An ignore directive turns off the formatting of whatever follows it,
    // which is a tangle of its own that this doesn't hunt for: a document
    // holding one doesn't settle even with the html left alone entirely. The
    // sweep above covers the html a directive is written in.
    if html.contains("dprint-ignore") {
      continue;
    }
    // the html is written as a block of its own, with markdown on either side
    // of it that has to still be there and still be markdown
    let source = format!("before *a*\n\n{}\n\nafter *b*\n", html);

    let Ok(once) = crate::format_text(&source, &config, |_, _, _| Ok(None)) else {
      continue; // a document the formatter won't take is not what is hunted here
    };
    let once = once.unwrap_or_else(|| source.clone());
    let Ok(twice) = crate::format_text(&once, &config, |_, _, _| Ok(None)) else {
      failures.push(format!(
        "case {}: {:?}\n  came out as text that won't format",
        case, source
      ));
      continue;
    };
    let twice = twice.unwrap_or_else(|| once.clone());

    if written_letters(&once) != written_letters(&source) {
      failures.push(format!(
        "case {}: {:?}\n  came out as {:?}, which doesn't say the same thing",
        case, source, once,
      ));
    } else if twice != once {
      failures.push(format!(
        "case {}: {:?}\n  came out as {:?} and then as {:?}, so formatting it doesn't settle",
        case, source, once, twice,
      ));
    }
    if failures.len() >= 10 {
      break;
    }
  }

  assert!(
    failures.is_empty(),
    "the formatter was wrong about {} of {} generated documents:\n\n{}",
    failures.len(),
    count,
    failures.join("\n\n"),
  );
}
