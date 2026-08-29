//! HTML -> Markdown.
//!
//! htmd in every build, never Obscura's V8 converter: that one is cruder, and
//! routing through it would make output depend on which build you installed.

/// Tags whose content is machinery, not prose. htmd walks whatever it is given,
/// so a `<style>` left in the tree comes out as a paragraph of CSS — which is
/// exactly what the first version of `markdown()` shipped.
const SKIP: [&str; 6] = ["style", "script", "noscript", "template", "head", "title"];

pub fn to_markdown(html: &str) -> std::io::Result<String> {
    htmd::HtmlToMarkdown::builder()
        .skip_tags(SKIP.to_vec())
        .build()
        .convert(html)
}
