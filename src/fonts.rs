//! Which fonts did the page ask for, and which did it actually get?
//!
//! The renderer bundles four families and never scans system fonts, so a page
//! asking for `font-family: "Brand Sans"` can silently render in Liberation
//! Sans: correct layout, plausible output, wrong brand, no warning. That is the
//! failure this reports.
//!
//! **It measures rather than assumes, and that distinction was earned.** The
//! first version reasoned that since only four families are bundled, any other
//! named family must be substituted. That is wrong: obscura loads `@font-face`
//! webfonts into the render layer during settle, so the report confidently
//! flagged fonts that had in fact loaded. Proof on blog.rust-lang.org, where the
//! same 40px probe string measures 468px in Fira Sans, 565px in Alfa Slab One
//! and 472px in `sans-serif` — three different faces, all really present.
//!
//! So: render a probe string in the requested family and in each bundled face.
//! If the widths coincide, the request fell back. A webfont whose metrics match
//! a bundled face exactly would read as substituted, which is a false positive
//! we accept — the probe is long and mixes wide and narrow glyphs to make that
//! unlikely.

/// Faces compiled into obscura-render. Keep in step with
/// `obscura-render/assets/`.
pub const BUNDLED: [&str; 5] = [
    "Liberation Sans",
    "Liberation Serif",
    "Liberation Mono",
    "DejaVu Sans",
    "Noto Color Emoji",
];

pub fn report_js() -> String {
    format!(
        r#"(function(){{
  var GENERIC = ["serif","sans-serif","monospace","cursive","fantasy","system-ui",
                 "ui-serif","ui-sans-serif","ui-monospace","ui-rounded","math","emoji",
                 "fangsong","inherit","initial","unset","revert",""];
  var BUNDLED = {bundled};
  var lower = function(s){{ return String(s||"").trim().replace(/^['"]|['"]$/g,"").toLowerCase(); }};

  // Mixes wide/narrow/round glyphs and digits so two unrelated faces are
  // unlikely to agree on total advance width.
  var PROBE = "Handgloves 12345 mmmiiiWWWlll@#%";
  var probe = document.createElement("span");
  probe.style.cssText = "position:absolute;left:-99999px;top:-99999px;" +
                        "font-size:40px;white-space:pre;visibility:hidden";
  probe.textContent = PROBE;
  if (document.body) document.body.appendChild(probe);
  var widthOf = function(stack){{
    if (!document.body) return 0;
    probe.style.fontFamily = stack;
    return Math.round(probe.getBoundingClientRect().width * 100) / 100;
  }};

  // Reference widths for every face the engine can fall back to.
  var refs = {{}};
  for (var b = 0; b < BUNDLED.length; b++) refs[BUNDLED[b]] = widthOf('"' + BUNDLED[b] + '"');
  var generics = ["sans-serif","serif","monospace","system-ui"];
  for (var g = 0; g < generics.length; g++) refs[generics[g]] = widthOf(generics[g]);

  // Distinct first-choice families in force on elements that actually have text.
  var firsts = {{}};
  var all = document.body ? document.body.querySelectorAll("*") : [];
  for (var i = 0; i < all.length; i++) {{
    var el = all[i];
    if (el === probe) continue;
    var hasText = false;
    for (var n = el.firstChild; n; n = n.nextSibling) {{
      if (n.nodeType === 3 && n.nodeValue && n.nodeValue.trim()) {{ hasText = true; break; }}
    }}
    if (!hasText) continue;
    var ff = "";
    try {{ ff = getComputedStyle(el).fontFamily || ""; }} catch (e) {{ continue; }}
    var first = lower(ff.split(",")[0]);
    if (!first || GENERIC.indexOf(first) !== -1) continue;
    // Keep the WHOLE stack, not just the first token. Probing the first token
    // alone reports where *it* would land, not where the declaration lands:
    // `"Fira Code", monospace` falls through to Liberation Mono, but the bare
    // token resolves to Liberation Sans, so the report named the wrong face.
    if (!firsts[first]) firsts[first] = {{ count: 0, stack: ff }};
    firsts[first].count++;
  }}

  var isBundled = function(name) {{
    for (var b = 0; b < BUNDLED.length; b++) if (BUNDLED[b].toLowerCase() === name) return true;
    return false;
  }};

  var substituted = [], loaded = [];
  for (var f in firsts) {{
    if (isBundled(f)) continue;                       // asked for a bundled face: exact
    var w = widthOf(firsts[f].stack);
    var matched = null;
    for (var key in refs) {{
      if (Math.abs(refs[key] - w) < 0.5) {{ matched = key; break; }}
    }}
    var row = {{ family: f, elements: firsts[f].count, width: w, stack: firsts[f].stack }};
    if (matched) {{ row.renderedAs = matched; substituted.push(row); }}
    else loaded.push(row);
  }}

  // @font-face families. Obscura's CSSOM does not model CSSFontFaceRule: the
  // rule arrives as a bare CSSRule with type 0 and no .style, only cssText.
  // Reading .style alone reported "no webfonts" on a page that declared one.
  var faces = [];
  for (var s = 0; s < document.styleSheets.length; s++) {{
    var rules = null;
    try {{ rules = document.styleSheets[s].cssRules; }} catch (e) {{ continue; }}
    if (!rules) continue;
    for (var r = 0; r < rules.length; r++) {{
      var rule = rules[r];
      if (!rule) continue;
      if (rule.type === 5 && rule.style) {{
        var fam = lower(rule.style.getPropertyValue("font-family"));
        if (fam && faces.indexOf(fam) === -1) faces.push(fam);
        continue;
      }}
      var txt = rule.cssText || "";
      if (txt.indexOf("@font-face") === 0) {{
        var m = /font-family\s*:\s*([^;}}]+)/i.exec(txt);
        if (m) {{ var f2 = lower(m[1]); if (f2 && faces.indexOf(f2) === -1) faces.push(f2); }}
      }}
    }}
  }}

  if (probe.parentNode) probe.parentNode.removeChild(probe);
  substituted.sort(function(a,b){{ return b.elements - a.elements; }});
  loaded.sort(function(a,b){{ return b.elements - a.elements; }});
  return {{ substituted: substituted, loaded: loaded,
            webFonts: faces, bundled: BUNDLED }};
}})()"#,
        bundled = serde_json::to_string(&BUNDLED).unwrap_or_else(|_| "[]".into())
    )
}
