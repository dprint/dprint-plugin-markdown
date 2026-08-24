use crate::parser::SPACES;
use std::borrow::Cow;

use regex::Regex;

/// Whether the character belongs to a script written without spaces between
/// its words, where a line break carries no meaning of its own.
///
/// Han, kana, and the punctuation and fullwidth forms written alongside them
/// qualify. Hangul does not: Korean is written with spaces between its words,
/// so a line break within it reads as one the way it does in English.
///
/// See <https://drafts.csswg.org/css-text-4/#line-break-transform>.
pub fn is_unspaced_script(c: char) -> bool {
  matches!(
    c as u32,
    // CJK radicals and Kangxi radicals
    0x2E80..=0x2FDF
    // CJK symbols and punctuation, hiragana, and katakana
    | 0x3000..=0x30FF
    // katakana phonetic extensions
    | 0x31F0..=0x31FF
    // CJK unified ideographs, and the extension and compatibility blocks
    | 0x3400..=0x4DBF
    | 0x4E00..=0x9FFF
    | 0xF900..=0xFAFF
    | 0x20000..=0x3FFFD
    // vertical forms, CJK compatibility forms, and small form variants
    | 0xFE10..=0xFE19
    | 0xFE30..=0xFE6F
    // fullwidth forms and halfwidth katakana, stopping before halfwidth hangul
    | 0xFF01..=0xFF9F
    | 0xFFE0..=0xFFE6
  )
}

/// Whether a line can't be broken before the character, which is what keeps
/// the marks that close or trail a phrase off the start of a line.
///
/// This is the part of Japanese line breaking (kinsoku shori) that every
/// renderer applies. See <https://www.w3.org/TR/jlreq/#characters_not_starting_a_line>.
pub fn forbids_line_break_before(c: char) -> bool {
  matches!(
    c,
    // marks that close or trail a phrase
    '、' | '。' | '，' | '．' | '：' | '；' | '？' | '！' | '‼' | '⁇' | '⁈' | '⁉'
    | '〕' | '〉' | '》' | '」' | '』' | '】' | '〙' | '〛' | '〞' | '〟' | '）'
    | '｝' | '］' | '｠' | '＞' | '»' | '›'
    // the middle dot, which separates the words within a phrase
    | '・'
    // the halfwidth forms of the same
    | '｡' | '｣' | '､' | '･'
    // iteration marks, which stand for the character before them
    | '々' | '〻' | 'ゝ' | 'ゞ' | 'ヽ' | 'ヾ'
    // the marks that voice the character before them, combining or not
    | '゛' | '゜' | 'ﾞ' | 'ﾟ' | '\u{3099}' | '\u{309A}'
    // the prolonged sound marks and the small kana, which belong to the
    // syllable written before them
    | 'ー' | 'ｰ' | '〜' | '～'
    | 'ぁ' | 'ぃ' | 'ぅ' | 'ぇ' | 'ぉ' | 'っ' | 'ゃ' | 'ゅ' | 'ょ' | 'ゎ' | 'ゕ' | 'ゖ'
    | 'ァ' | 'ィ' | 'ゥ' | 'ェ' | 'ォ' | 'ッ' | 'ャ' | 'ュ' | 'ョ' | 'ヮ' | 'ヵ' | 'ヶ'
    // the small katakana written for the sounds of other languages
    | '\u{31F0}'..='\u{31FF}'
    // the halfwidth small kana
    | '\u{FF67}'..='\u{FF6F}'
  )
}

/// Whether a line can't be broken after the character, which is what keeps the
/// marks that open a phrase off the end of a line.
///
/// See <https://www.w3.org/TR/jlreq/#characters_not_ending_a_line>.
pub fn forbids_line_break_after(c: char) -> bool {
  matches!(
    c,
    '〔' | '〈' | '《' | '「' | '『' | '【' | '〘' | '〚' | '〝' | '（'
    | '｛' | '［' | '｟' | '＜' | '«' | '‹'
    // the halfwidth form of the same
    | '｢'
  )
}

/// Whether the text would start a block of its own, or turn the line above it
/// into one, if it were moved to the start of a line. Such text can't be moved
/// there by wrapping without changing what the document means.
///
/// Takes the text from the word that would begin the line through to the end
/// of it, since what starts a block is decided by the whole line: `1.` opens a
/// list only when something follows it, and `|-|` turns the line above it into
/// a table header only in its entirety.
///
/// `word_can_be_left_alone` says whether the text after the first word can be
/// wrapped onto the line below and leave that word by itself, which is what
/// makes what the word would start on its own count as well. Where text isn't
/// being wrapped it can't, and holding the word back would only join lines the
/// document was written with.
pub fn starts_block_at_line_start(line_text: &str, word_can_be_left_alone: bool) -> bool {
  let leading_word = line_text.split(' ').next().unwrap_or(line_text);
  word_can_be_left_alone && crate::parser::starts_block_in_paragraph(leading_word)
    || crate::parser::starts_block_in_paragraph(line_text)
}

/// Whether the text would start a block of its own if the word that begins it
/// were moved to the start of a line by wrapping.
///
/// A word that would become a list marker counts as well as what
/// [`starts_block_at_line_start`] finds, since the text that wraps onto the
/// line after the marker is what would turn it into a real one.
pub fn wrapping_word_starts_block(line_text: &str, word_can_be_left_alone: bool) -> bool {
  let leading_word = line_text.split(' ').next().unwrap_or(line_text);
  is_list_word(leading_word) || starts_block_at_line_start(line_text, word_can_be_left_alone)
}

/// Checks if the provided word is a word that could be a list.
/// Assumes the provided string is one word, which is to say it has no space
/// or line ending in it. Other whitespace (ex. a tab) may show up within a
/// word because only those two break one.
pub fn is_list_word(word: &str) -> bool {
  debug_assert!(!word.chars().any(|c| c == ' ' || c == '\n'));

  if word == "*" || word == "-" || word == "+" {
    true
  } else {
    let mut had_number = false;
    let mut had_end_char = false;
    for c in word.chars() {
      if had_end_char {
        return false;
      }

      if !had_number {
        if c.is_numeric() {
          had_number = true;
        } else {
          return false;
        }
      } else if !c.is_numeric() {
        if c == '.' || c == ')' {
          had_end_char = true;
        } else {
          return false;
        }
      }
    }

    had_end_char
  }
}

/// The number of blank lines that precede the position at `index`, counting no
/// further than `max` of them.
///
/// When `in_block_quote` is `true`, the block quote markers (`>`) that prefix an
/// otherwise blank line are skipped while scanning backwards. Without this a blank
/// line inside a block quote (which is written as `>`) would not be recognized as
/// blank because of the leading `>` character.
pub fn get_leading_blank_lines(index: usize, text: &str, in_block_quote: bool, max: u32) -> u32 {
  let mut newline_count = 0;
  // whether the character to the right of this one was a newline, which makes
  // a carriage return here the start of the line ending it already counted
  let mut after_newline = false;
  for c in text[0..index].chars().rev() {
    let ends_line = c == '\n' || (c == '\r' && !after_newline);
    after_newline = c == '\n';
    if ends_line {
      newline_count += 1;
      // the first line ending found is the one the line above ends with
      // rather than a blank line, so it takes one more than `max` of them
      // before the maximum is passed
      if newline_count > max {
        break;
      }
    } else if c == '\r' {
      continue;
    } else if in_block_quote && c == '>' {
      // a blank line inside a block quote is written as `>`, so skip the block
      // quote markers while looking for consecutive newlines
      continue;
    } else if !SPACES.contains(&c) {
      break;
    }
  }
  newline_count.saturating_sub(1)
}

pub fn file_has_ignore_file_directive(file_text: &str, directive_inner_text: &str) -> bool {
  let ignore_regex = get_ignore_comment_regex(directive_inner_text);
  ignore_regex.is_match(file_text)
}

pub fn get_ignore_comment_regex(inner_text: &str) -> Regex {
  // todo: don't use regex
  let text = format!(r"^\s*<!\-\-\s*{}\s*\-\->\s*", inner_text);
  Regex::new(&text).unwrap()
}

pub fn get_leading_non_space_tab_byte_pos(text: &str, pos: usize) -> usize {
  let text_bytes = text.as_bytes();
  for i in (0..pos).rev() {
    let current_char = text_bytes.get(i);
    if current_char != Some(&(b' ')) && current_char != Some(&(b'\t')) {
      return i + 1;
    }
  }

  0
}

pub fn unindent(text: &str) -> Cow<'_, str> {
  let lines = text.split('\n').collect::<Vec<_>>();
  let mut lines_with_indent = Vec::with_capacity(lines.len());
  for line in lines.into_iter() {
    // a character that only looks like a space, such as a non-breaking one, is
    // text of the code rather than the indentation written before it
    let line_indent = line.chars().take_while(|c| SPACES.contains(c)).count();
    if line_indent == 0 {
      return Cow::Borrowed(text);
    }
    lines_with_indent.push((line, line_indent));
  }
  let min_indent = lines_with_indent.iter().map(|(_, indent)| indent).min().copied();
  if let Some(min_indent) = min_indent {
    Cow::Owned(
      lines_with_indent
        .into_iter()
        .map(|(l, indent)| {
          if indent >= min_indent {
            let mut chars = l.chars();
            for _ in 0..min_indent {
              chars.next();
            }
            chars.as_str()
          } else {
            l
          }
        })
        .collect::<Vec<_>>()
        .join("\n"),
    )
  } else {
    Cow::Borrowed(text)
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn it_should_find_unspaced_scripts() {
    // han, hiragana, and katakana
    assert_eq!(is_unspaced_script('漢'), true);
    assert_eq!(is_unspaced_script('は'), true);
    assert_eq!(is_unspaced_script('カ'), true);
    // the punctuation and fullwidth forms written alongside them
    assert_eq!(is_unspaced_script('。'), true);
    assert_eq!(is_unspaced_script('，'), true);
    assert_eq!(is_unspaced_script('）'), true);
    // and the halfwidth forms of the kana and punctuation written with them
    assert_eq!(is_unspaced_script('ｶ'), true);
    assert_eq!(is_unspaced_script('｡'), true);
    // korean is written with spaces between its words
    assert_eq!(is_unspaced_script('한'), false);
    assert_eq!(is_unspaced_script('ᄀ'), false);
    // and neither is anything else
    assert_eq!(is_unspaced_script('a'), false);
    assert_eq!(is_unspaced_script(' '), false);
    assert_eq!(is_unspaced_script(','), false);
    assert_eq!(is_unspaced_script('é'), false);
  }

  #[test]
  fn it_should_find_characters_that_cant_start_a_line() {
    // marks that close or trail a phrase
    assert_eq!(forbids_line_break_before('。'), true);
    assert_eq!(forbids_line_break_before('、'), true);
    assert_eq!(forbids_line_break_before('」'), true);
    assert_eq!(forbids_line_break_before('）'), true);
    assert_eq!(forbids_line_break_before('・'), true);
    // the small kana and the marks that belong to the syllable before them
    assert_eq!(forbids_line_break_before('っ'), true);
    assert_eq!(forbids_line_break_before('ョ'), true);
    assert_eq!(forbids_line_break_before('ー'), true);
    assert_eq!(forbids_line_break_before('\u{3099}'), true);
    assert_eq!(forbids_line_break_before('\u{FF6F}'), true);
    // a line can start with anything else
    assert_eq!(forbids_line_break_before('あ'), false);
    assert_eq!(forbids_line_break_before('漢'), false);
    assert_eq!(forbids_line_break_before('「'), false);
    assert_eq!(forbids_line_break_before('a'), false);
  }

  #[test]
  fn it_should_find_characters_that_cant_end_a_line() {
    assert_eq!(forbids_line_break_after('「'), true);
    assert_eq!(forbids_line_break_after('（'), true);
    assert_eq!(forbids_line_break_after('【'), true);
    assert_eq!(forbids_line_break_after('｢'), true);
    // a line can end with anything else
    assert_eq!(forbids_line_break_after('」'), false);
    assert_eq!(forbids_line_break_after('。'), false);
    assert_eq!(forbids_line_break_after('漢'), false);
    assert_eq!(forbids_line_break_after('a'), false);
  }

  #[test]
  fn it_should_find_list_words() {
    assert_eq!(is_list_word("test"), false);
    assert_eq!(is_list_word("*"), true);
    assert_eq!(is_list_word("+"), true);
    assert_eq!(is_list_word("-"), true);
    assert_eq!(is_list_word("1."), true);
    assert_eq!(is_list_word("99."), true);
    assert_eq!(is_list_word("10)"), true);
    assert_eq!(is_list_word("9999)"), true);
    assert_eq!(is_list_word("9999)."), false);
  }

  #[test]
  fn it_should_find_text_that_starts_a_block_at_a_line_start() {
    assert_eq!(starts_block_at_line_start("test", true), false);
    assert_eq!(wrapping_word_starts_block("-", true), true);
    assert_eq!(wrapping_word_starts_block("1.", true), true);
    assert_eq!(starts_block_at_line_start(">", true), true);
    assert_eq!(starts_block_at_line_start(">text", true), true);
    assert_eq!(starts_block_at_line_start("#", true), true);
    assert_eq!(starts_block_at_line_start("######", true), true);
    assert_eq!(starts_block_at_line_start("#######", true), false);
    assert_eq!(starts_block_at_line_start("#text", true), false);
    assert_eq!(starts_block_at_line_start("=", true), true);
    assert_eq!(starts_block_at_line_start("===", true), true);
    assert_eq!(starts_block_at_line_start("=text", true), false);
    assert_eq!(starts_block_at_line_start("---", true), true);
    assert_eq!(starts_block_at_line_start("***", true), true);
    assert_eq!(starts_block_at_line_start("___", true), true);
    assert_eq!(starts_block_at_line_start("~~~", true), true);
    assert_eq!(starts_block_at_line_start("```", true), true);
    assert_eq!(starts_block_at_line_start("~~", true), false);
    assert_eq!(starts_block_at_line_start("---a", true), false);
    // two dashes underline the line above into a heading
    assert_eq!(starts_block_at_line_start("--", true), true);
    // html, a footnote definition, and a table's delimiter row
    assert_eq!(starts_block_at_line_start("<div>", true), true);
    assert_eq!(starts_block_at_line_start("<!-- x -->", true), true);
    assert_eq!(starts_block_at_line_start("[^note]:", true), true);
    assert_eq!(starts_block_at_line_start("|---|", true), true);
    // the word decides it even where the rest of the line reads as text, since
    // that text can wrap onto the line below and leave the word alone
    assert_eq!(starts_block_at_line_start("-- text", true), true);
    assert_eq!(starts_block_at_line_start("--- text", true), true);
    assert_eq!(starts_block_at_line_start("text --", true), false);
    // a list marker is only a marker once text follows it on the line, so it
    // counts for a word that wrapping would move but not for a node that
    // already begins a line
    assert_eq!(starts_block_at_line_start("40.", true), false);
    assert_eq!(wrapping_word_starts_block("40.", true), true);
    assert_eq!(starts_block_at_line_start("6. Test", true), false);
    assert_eq!(wrapping_word_starts_block("6. Test", true), true);
    // where the text after the word can't be wrapped away the word is never
    // left alone, so only what the whole line starts counts
    assert_eq!(starts_block_at_line_start("-- text", false), false);
    assert_eq!(starts_block_at_line_start("--", false), true);
    assert_eq!(starts_block_at_line_start("=== not a heading", false), false);
  }

  #[test]
  fn should_unindent() {
    assert_eq!(unindent("  1\r\n  2"), "1\r\n2");
    assert_eq!(unindent("  1\n 2"), " 1\n2");
    assert_eq!(unindent(" 1\n  2"), "1\n 2");
    assert_eq!(unindent("1\n2"), "1\n2");
    // a character that only looks like a space is text of the code rather than
    // the indentation written before it
    assert_eq!(unindent("\u{3000}1\n\u{3000}\u{3000}2"), "\u{3000}1\n\u{3000}\u{3000}2");
    assert_eq!(unindent("  \u{3000}1\n  \u{3000}2"), "\u{3000}1\n\u{3000}2");
  }
}
