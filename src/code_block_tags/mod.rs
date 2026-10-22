mod linguist_generated;

use linguist_generated::LINGUIST_TAG_EXTENSIONS;

/// Gets the file extension to format a code block with, from its lowercase
/// tag. The tags GitHub Linguist knows map to the primary extension of their
/// language (ex. `starlark` and `bzl` are `bzl`), except for the ones below.
#[cfg_attr(
  not(all(feature = "wasm", target_arch = "wasm32", target_os = "unknown")),
  allow(dead_code)
)]
pub(crate) fn tag_to_extension(tag: &str) -> Option<&'static str> {
  match tag {
    // Linguist's extension for these is `sh`, but shell formatters pick the
    // dialect from the extension
    "sh" | "shell" | "shellscript" | "shell-script" | "bash" => Some("bash"),
    "zsh" => Some("zsh"),
    // `tf` gets the rewrites of `tofu fmt`, which `hcl` (Linguist's) doesn't
    "terraform" | "tf" => Some("tf"),
    "yml" | "yaml" => Some("yaml"),
    // ones Linguist doesn't know
    "jsx" => Some("jsx"),
    "cs" => Some("cs"),
    "visualbasic" | "vb" => Some("vb"),
    "cedar" => Some("cedar"),
    "jl" => Some("jl"),
    "bib" => Some("bib"),
    _ => linguist_tag_extension(tag),
  }
}

fn linguist_tag_extension(tag: &str) -> Option<&'static str> {
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
  fn finds_linguist_names_and_aliases() {
    assert_eq!(tag_to_extension("starlark"), Some("bzl"));
    assert_eq!(tag_to_extension("bzl"), Some("bzl"));
    assert_eq!(tag_to_extension("bazel"), Some("bzl"));
    assert_eq!(tag_to_extension("protobuf"), Some("proto"));
    assert_eq!(tag_to_extension("4d"), Some("4dm"));
    assert_eq!(tag_to_extension("not-a-language"), None);
  }

  // every tag the plugin mapped itself before it used Linguist's, which have
  // to keep their extension when the table is generated from a newer Linguist
  #[test]
  fn keeps_the_extensions_of_the_tags_mapped_before_linguist() {
    let tags = [
      ("typescript", "ts"),
      ("ts", "ts"),
      ("tsx", "tsx"),
      ("javascript", "js"),
      ("js", "js"),
      ("jsx", "jsx"),
      ("json", "json"),
      ("jsonc", "jsonc"),
      ("rust", "rs"),
      ("rs", "rs"),
      ("c", "c"),
      ("cpp", "cpp"),
      ("c++", "cpp"),
      ("objective-c", "m"),
      ("objc", "m"),
      ("objective-c++", "mm"),
      ("objc++", "mm"),
      ("csharp", "cs"),
      ("cs", "cs"),
      ("c#", "cs"),
      ("visualbasic", "vb"),
      ("vb", "vb"),
      ("html", "html"),
      ("css", "css"),
      ("less", "less"),
      ("toml", "toml"),
      ("scss", "scss"),
      ("svelte", "svelte"),
      ("vue", "vue"),
      ("astro", "astro"),
      ("xml", "xml"),
      ("yml", "yaml"),
      ("yaml", "yaml"),
      ("graphql", "graphql"),
      ("python", "py"),
      ("py", "py"),
      ("dockerfile", "dockerfile"),
      ("cue", "cue"),
      ("sh", "bash"),
      ("shell", "bash"),
      ("shellscript", "bash"),
      ("shell-script", "bash"),
      ("bash", "bash"),
      ("zsh", "zsh"),
      ("nix", "nix"),
      ("kdl", "kdl"),
      ("typst", "typ"),
      ("typ", "typ"),
      ("php", "php"),
      ("sql", "sql"),
      ("cedar", "cedar"),
      ("lua", "lua"),
      ("luau", "luau"),
      ("zig", "zig"),
      ("terraform", "tf"),
      ("tf", "tf"),
      ("hcl", "hcl"),
      ("go", "go"),
      ("golang", "go"),
      ("swift", "swift"),
      ("r", "r"),
      ("julia", "jl"),
      ("jl", "jl"),
      ("latex", "tex"),
      ("tex", "tex"),
      ("bibtex", "bib"),
      ("bib", "bib"),
      ("cmake", "cmake"),
    ];
    for (tag, extension) in tags {
      assert_eq!(tag_to_extension(tag), Some(extension), "{tag}");
    }
  }
}
