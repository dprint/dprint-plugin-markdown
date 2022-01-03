pub fn parse_link_reference(text: &str) -> String {
  // very crude...
  let chars = text.trim().chars();
  let mut text = Vec::new();
  for c in chars.into_iter().rev().skip(1) {
    if c == '[' {
      break;
    }
    text.push(c);
  }
  text.reverse();
  text.into_iter().collect()
}
