# Investigation: a specified `width` is ignored on some flex items

**Status: both solved.** Two separate bugs, two upstream PRs:
[#749](https://github.com/h4ckf0r0day/obscura/pull/749) (functional `flex-basis`) and
[#750](https://github.com/h4ckf0r0day/obscura/pull/750) (overflowing inline-block never wraps).

## Resolution (2026-08-29) — readymembership.com

The site is **Bootstrap 3.4.1** (float grid: `.col-lg-8{width:66.67%}`), not
Bootstrap 4/5 — so the Bootstrap fixture in "ruled out" #10 was testing the
wrong thing. The declaration that actually matters is in the site's own CSS:

```css
@media (min-width:992px){
  .header-layout-single .header-inner .main-nav-holder{
    flex-basis:calc(100% - 314px); max-width:calc(100% - 314px)
  }
}
```

`obscura-render` parsed the `flex-basis` through `dimension_value`, which
evaluates `calc()` **context-free** (`100%` → 0), giving a basis of −314px that
taffy clamped to 0. With a definite (zero) basis and `flex-grow:0`, the item sat
at its min-content width (145px) and `width` — inline, `!important`, anything —
was correctly irrelevant, because a non-`auto` flex-basis takes precedence over
`width`. That is why only the `flexBasis` lever moved it. `max-width` from the
same block worked because width/min/max keep the raw expression in
`size_expressions` and resolve it later against the containing block; flex-basis
had no such slot.

Fix, in the obscura fork (`crates/obscura-render`):

- `lib.rs`: new `LayoutStyle::flex_basis_expression: Option<String>`.
- `style.rs`: the `flex-basis` longhand and the `flex` shorthand store the
  functional expression there; the shorthand now tokenises with
  `split_top_level(value, ' ')` so `flex:0 0 calc(100% - 314px)` is one basis
  token instead of three garbage ones.
- `dom.rs` (`layout_dom` top-down pass, next to the width/height resolution):
  resolve the expression with `resolve_contextual_length` against
  `inh.cb_width`.
- Regression test `functional_flex_basis_resolves_against_the_containing_block`
  in `tests/layout_test.rs`.

Minimal repro (rendered 26px before, 886px after — `width:calc(...)` on the same
item was already 886):

```html
<div style="display:flex;width:1200px">
  <div style="flex:0 0 300px">a</div>
  <div style="flex-basis:calc(100% - 314px)">nav</div>
</div>
```

Live: `.main-nav-holder` went 145 → 1046 and the header nav lays out
horizontally. (`p.evaluate` returning `width800:1046` is *correct* — Chrome
also ignores `width` when `flex-basis` is definite.)

## Resolution (2026-08-29) — en.wikipedia.org `.infobox` (641px → 310px)

Not flex-related, and not the `width:22em` rule either. A table's `width` is a
*floor* — it grows if its columns' min-content exceeds it — and the engine's
min-content for this table was 641px. Walking the infobox cells found the wide
one: the "Influenced by" `ul.cslist`, an `inline-block` `<ul>` of
`inline-block` `<li>`s with `::after` commas, measuring **1128px on one line**.

Root cause in `dom.rs` (inline-block branch of the node builder): an auto-width
inline-block without block children is built as a `NoWrap` flex row so a short
control shrink-fits to its max-content line. Nothing enforced the `available`
half of shrink-to-fit's `min(max-content, available)`, so a long inline-block
list never wrapped and overflowed its container. Reproduces in a plain
`<div style="width:352px">` — `ul` 1023px on one line — so it was never
table-specific; the table just made the overflow visible as a width.

Fix (PR #750, `crates/obscura-render/src/dom.rs`):

- `wrap_overflowing_inline_blocks`: post-preliminary-layout repair pass — any
  such `NoWrap` inline-block whose margin box exceeds the parent's content box
  is switched to `Wrap`, then Taffy re-runs. Boxes that fit are untouched.
- Table sizing measures table/cell **min-content** with those inline-blocks
  temporarily `Wrap` (`nowrap_inline_block_rows` / `set_flex_wrap`), then
  restores `NoWrap`. Without this the floor is computed before the repair.
- Regression test `overflowing_inline_block_list_wraps_inside_its_container`.

Live: `.infobox` 641 → **310px**. Chrome on the same page/viewport: 309.76px
(`22em` × the infobox's 14.08px computed font-size). The 352px in the issue
assumed a 16px root — do not chase the missing 42px, it does not exist.

Harness caveat: on this machine `main` already fails
`table_width_uses_local_space_and_keeps_width_hints_shrinkable` (a
`width:300px` table lays out at the viewport width in `layout_dom` unit tests),
so table widths can only be validated through the extension, not `cargo test`.

---

# Original write-up (kept for the dead-ends list)

---

## The symptom, in two places

**1. en.wikipedia.org** — at a viewport ≥1150px the article text collapses to
roughly one character per line. The paragraph is *correct* for the space it is
given: the floated `.infobox` beside it is 641px when its rule says `width:22em`
(352px), leaving 97px of a 752px column.

**2. readymembership.com** — on every page the header nav stacks vertically over
the hero. Same shape: `div.col-12.col-lg-8.main-nav-holder` lays out at **145px**
inside a 1230px row, where Bootstrap's `col-lg-8` should give it ~820px.

Both are "an element that should be a specified width is instead sized to its
content". 641px and 145px are both shrink-to-fit values.

## The measurement that matters

On the live readymembership.com page, against `.main-nav-holder`:

| lever | resulting width |
|---|---|
| (untouched) | 145 |
| `el.style.width = '800px'` | **145 — ignored** |
| injected `.main-nav-holder{width:700px !important}` | **145 — ignored** |
| `el.style.flex = '0 0 66.6667%'` | **820 — honoured** |
| `el.style.flexBasis = '66.6667%'` | **820 — honoured** |

`width` is inert on this element — inline and `!important` both — while
`flex-basis` works. Its computed `max-width` *is* correct (926px), so the
`@media` block the rule lives in is matching; it is specifically `width` that
does not survive.

Per spec a flex item with `flex-basis:auto` resolves its basis from `width`. The
observed behaviour is consistent with `width` never reaching the basis, so the
item falls back to content sizing.

**This is the thread to pull.**

## Ruled out — do not redo these

Each was built as a fixture and rendered **correctly**, so none is the cause on
its own:

1. `em` widths inside `@media (min-width:…)`, with and without a space after `@media`
2. Descendant selectors inside `@media`
3. `@media all and (max-width:calc(640px - 1px))` — Wikipedia ships this; rules after it still apply
4. `@media` rules in an **external** stylesheet (linked, not inline)
5. A table honouring a specified width, and its cell text wrapping to it
6. `float:right` combined with a specified width
7. `!important` beating an earlier rule, on a table and a div
8. `width:100%` on a table in plain / `overflow:auto` / `max-height` / flex-column containers
9. `table-layout:fixed` with sticky headers
10. Bootstrap 4.6.2's real grid — `col-12 col-lg-8` + `col-12 col-lg-4` at 1400px gives a correct 760/380 split
11. `width` on a flex item: plain div, percentage, and a `<table>` — all correct
12. `flex-wrap:wrap` containers, and flex items that are themselves `display:flex`
13. A `<nav style="display:flex">` of links inside a fixed-width flex item

## Tooling notes — what does and does not work for bisecting

- **`styleSheet.disabled = true` is a no-op for layout.** Toggling every sheet on
  readymembership.com changed nothing (stayed 145). Obscura's CSSOM is a shim;
  it does not drive the cascade. This kills the obvious bisect strategy.
- **`@media` rules are not modelled in the CSSOM.** They come back as bare
  `CSSRule` (type 0) with only `cssText` — no `.cssRules`, no `.conditionText` —
  so you cannot walk the cascade to see which declarations won.
- Same for `@font-face`: type 0, no `.style`, `cssText` only.
- **`getComputedStyle` does not report flex properties.** `flexGrow`,
  `flexShrink` and `flexBasis` all return `""`, even on a page where flex layout
  is demonstrably working. Do not read anything into those being empty.
- `getComputedStyle().width` *does* report the used width, and
  `getBoundingClientRect()` agrees with layout. Those two are trustworthy.
- **Inline `element.style.flexBasis` is a working lever** and re-lays-out
  immediately. It is the one reliable way found to move this element.

## How to reproduce in five seconds

With the extension installed:

```cfml
p = Browser({timeout:90000}).newPage()
     .goto( "https://readymembership.com/", { waitUntil="networkidle2" } ).settle( 1200 );
p.setViewport( 1400, 900 );
writeOutput( p.evaluate( "
 (function(){
   function w(){ return Math.round(document.querySelector('.main-nav-holder').getBoundingClientRect().width); }
   var e=document.querySelector('.main-nav-holder'), o={ base:w() };
   e.style.width='800px';        o.width800  = w();   e.style.width='';
   e.style.flexBasis='66.6667%'; o.flexBasis = w();   e.style.flexBasis='';
   return o;
 })()" ) );
```

Expect `{base:145, width800:145, flexBasis:820}`. A fixed engine gives
`width800:800`.

Wikipedia's variant, for a second data point:

```cfml
p.goto( "https://en.wikipedia.org/wiki/Rust_(programming_language)" ).settle( 400 );
p.setViewport( 1280, 900 );
p.evaluate( "document.querySelector('.infobox').getBoundingClientRect().width" );  // 641, should be 352
```

## Where I would look next, in order

1. **Find where a flex item's base size is computed** in `obscura-render`
   (`dom.rs` builds the taffy tree; `style.rs` computes style). Check whether the
   used `width` is written into taffy's `Dimension` for an item whose parent is a
   flex container, or whether only an explicit `flex-basis` is. Taffy models
   `flex_basis` and `size.width` separately, and CSS requires `width` to feed the
   basis when `flex-basis` is `auto`. **That mapping is the prime suspect.**
2. If that is correct, check the same path when the item is a **grid** or
   **table** child, since the Wikipedia case is a `<table>`.
3. Bisect the real page **outside** the CSSOM: save `readymembership.com` and its
   stylesheets to disk with the extension, reproduce locally, then delete CSS
   rules mechanically until the width appears. Slower than the CSSOM route but it
   actually converges, and every fixture-based reduction has failed.
4. Compare against `aginxbrowser`, an obscura-descended fork whose maintainer
   commented on issue #737 that they lay the infobox out at exactly 352px. Their
   divergence from obscura on this path may be a very short diff.

## What this is not

Not a media-query bug — `max-width` from the same block applies. Not table
sizing — it reproduces on a plain `<div>`. Not `!important` or cascade ordering —
inline styles fail too. Not viewport-dependent in itself: the Wikipedia infobox
is 641px at *every* viewport; what changes at 1150px is only the column it has to
share with.
