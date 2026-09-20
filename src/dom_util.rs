//! DOM helpers that give CFML what a developer expects rather than what the
//! spec literally says.

use obscura_dom::tree::{DomTree, NodeId};

/// Elements whose text is markup machinery, not page content.
const NON_RENDERED: [&str; 6] = ["style", "script", "noscript", "template", "head", "title"];

/// `innerText`-ish: the text a reader would see.
///
/// `DomTree::text_content` is spec-correct `textContent`, which includes the
/// contents of `<style>` and `<script>`. That is right for the DOM API and
/// wrong for anyone asking a page for its text — the first thing we shipped
/// returned a stylesheet glued to the front of example.com's prose. Skip the
/// non-rendered elements and collapse whitespace the way a browser does.
pub fn visible_text(dom: &DomTree, root: NodeId) -> String {
    let mut out = String::new();
    walk(dom, root, &mut out);
    collapse_ws(&out)
}

fn walk(dom: &DomTree, id: NodeId, out: &mut String) {
    let skip = dom
        .with_node(id, |n| {
            n.as_element()
                .map(|q| NON_RENDERED.contains(&q.local.as_ref()))
                .unwrap_or(false)
        })
        .unwrap_or(false);
    if skip {
        return;
    }
    if let Some(Some(text)) =
        dom.with_node(id, |n| n.text_content_of_text_node().map(str::to_owned))
    {
        out.push_str(&text);
    }
    // A block boundary should not weld two words together ("EndStart").
    let is_block = dom
        .with_node(id, |n| {
            n.as_element()
                .map(|q| {
                    matches!(
                        q.local.as_ref(),
                        "p" | "div"
                            | "br"
                            | "li"
                            | "tr"
                            | "section"
                            | "article"
                            | "h1"
                            | "h2"
                            | "h3"
                            | "h4"
                            | "h5"
                            | "h6"
                    )
                })
                .unwrap_or(false)
        })
        .unwrap_or(false);
    if is_block {
        out.push('\n');
    }
    for child in dom.children(id) {
        walk(dom, child, out);
    }
    if is_block {
        out.push('\n');
    }
}

/// Collapse runs of spaces/tabs, keep at most one blank line, trim the ends.
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut newlines = 0usize;
    let mut pending_space = false;
    for ch in s.chars() {
        match ch {
            '\n' | '\r' => {
                newlines += 1;
                pending_space = false;
            }
            c if c.is_whitespace() => pending_space = true,
            c => {
                if !out.is_empty() {
                    if newlines > 0 {
                        out.push_str(if newlines > 1 { "\n\n" } else { "\n" });
                    } else if pending_space {
                        out.push(' ');
                    }
                }
                newlines = 0;
                pending_space = false;
                out.push(c);
            }
        }
    }
    out
}

/// Text of the first match, or None when the selector matches nothing. An
/// invalid selector is an error, not an empty result.
pub fn select_text(dom: &DomTree, selector: &str) -> Result<Option<String>, String> {
    match dom.query_selector(selector)? {
        Some(id) => Ok(Some(visible_text(dom, id))),
        None => Ok(None),
    }
}

pub fn select_count(dom: &DomTree, selector: &str) -> Result<usize, String> {
    Ok(dom.query_selector_all(selector)?.len())
}

pub fn select_attr(dom: &DomTree, selector: &str, attr: &str) -> Result<Option<String>, String> {
    let Some(id) = dom.query_selector(selector)? else {
        return Ok(None);
    };
    Ok(dom
        .with_node(id, |n| n.get_attribute(attr).map(str::to_owned))
        .flatten())
}

/// One matched element: its text plus every attribute it carries.
pub type ExtractedRow = (String, Vec<(String, String)>);

/// One struct per match: the element's text plus every attribute it carries.
pub fn extract(dom: &DomTree, selector: &str) -> Result<Vec<ExtractedRow>, String> {
    let mut rows = Vec::new();
    for id in dom.query_selector_all(selector)? {
        let attrs = dom
            .with_node(id, |n| {
                n.attrs()
                    .map(|a| {
                        a.iter()
                            .map(|at| (at.name.local.to_string(), at.value.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        rows.push((visible_text(dom, id), attrs));
    }
    Ok(rows)
}

/// Every `href` on the page, resolved against the document URL.
pub fn links(dom: &DomTree, base: Option<&str>) -> Vec<String> {
    let base = base.and_then(|b| url::Url::parse(b).ok());
    let mut out = Vec::new();
    let Ok(nodes) = dom.query_selector_all("a[href]") else {
        return out;
    };
    for id in nodes {
        let Some(Some(href)) = dom.with_node(id, |n| n.get_attribute("href").map(str::to_owned))
        else {
            continue;
        };
        match &base {
            Some(b) => {
                if let Ok(abs) = b.join(&href) {
                    out.push(abs.to_string());
                }
            }
            None => out.push(href),
        }
    }
    out
}
