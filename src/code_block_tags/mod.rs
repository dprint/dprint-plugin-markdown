mod linguist_generated;

use linguist_generated::LINGUIST_TAG_EXTENSIONS;

/// Gets the primary file extension of the language GitHub Linguist knows by a
/// lowercase code block tag, which is the language's name or one of its
/// aliases (ex. `starlark` and `bzl` are `bzl`).
#[cfg_attr(
  not(all(feature = "wasm", target_arch = "wasm32", target_os = "unknown")),
  allow(dead_code)
)]
pub(crate) fn linguist_tag_extension(tag: &str) -> Option<&'static str> {
  LINGUIST_TAG_EXTENSIONS
    .binary_search_by(|(entry_tag, _)| (*entry_tag).cmp(tag))
    .ok()
    .map(|index| LINGUIST_TAG_EXTENSIONS[index].1)
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn tags_are_sorted_and_unique() {
    for pair in LINGUIST_TAG_EXTENSIONS.windows(2) {
      assert!(pair[0].0 < pair[1].0, "{:?} isn't before {:?}", pair[0], pair[1]);
    }
  }

  #[test]
  fn finds_names_and_aliases() {
    assert_eq!(linguist_tag_extension("starlark"), Some("bzl"));
    assert_eq!(linguist_tag_extension("bzl"), Some("bzl"));
    assert_eq!(linguist_tag_extension("bazel"), Some("bzl"));
    assert_eq!(linguist_tag_extension("protobuf"), Some("proto"));
    assert_eq!(linguist_tag_extension("4d"), Some("4dm"));
    assert_eq!(linguist_tag_extension("not-a-language"), None);
  }
}
