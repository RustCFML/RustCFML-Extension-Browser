# RustCFML browser extension

A real headless browser inside your CFML engine. JavaScript executes, CSS lays
out, and you get screenshots and PDFs — with no Chrome, no Selenium and no JVM.

```cfml
page = Browser().newPage()
    .goto( "https://example.com/" )
    .waitForSelector( "h1", 5000 );

writeOutput( page.title() );                          // Example Domain
fileWrite( "shot.png", page.screenshot( { fullPage = true } ) );
fileWrite( "page.pdf", page.pdf( { paper = "A4" } ) );
```

Built on [Obscura](https://obscura.sh), an independent browser engine written in
Rust. It is not Chromium and does not aim to be — see **What it is good at**.

## Install

```sh
rustcfml ext install browser-0.1.0.rcx --user
```

One archive per platform; the file is ~35 MB because it carries V8 and a
complete layout and paint engine.

## Building

This repo has no vendored dependencies: it builds against a checkout of the
obscura fork, which must sit in the same tree.

```
<root>/
  CFMLs/
    RustCFML/              the engine (for crates/rustcfml-module)
    rustcfml-browser/      this repo
  THIRDPARTY/
    obscura/               the fork
```

```sh
rustcfml ext build .
rustcfml ext install browser-0.1.0.rcx --user
./tests/run.sh
```

Two things about the obscura checkout that are not optional:

- **It must carry the outstanding render fixes.** Stock obscura resolves a
  functional `flex-basis` context-free, so `calc(100% - 314px)` becomes a zero
  basis and any flex item using one collapses to its min-content width. Real
  sites lay out wrong in ways that look like a dropped `width`. Fixes are
  upstream as [#749](https://github.com/h4ckf0r0day/obscura/pull/749) and
  [#750](https://github.com/h4ckf0r0day/obscura/pull/750); until they merge, the
  path deps become git deps pinned to a tag rather than sooner.
- **`obscura-render` needs the vendored taffy and cosmic-text**, hence the
  `[patch.crates-io]` block. Stock taffy 0.12.1 lacks `set_calc_resolver`,
  `item_aspect_ratio_is_intrinsic` and `AlignItemsKeyword::Normal`. Cargo
  *silently ignores* a patch whose version matches but whose features do not —
  it warns and carries on — and the failure surfaces as a wall of type errors
  inside a third-party crate.

## What it is good at

Reports, dashboards, invoices, admin pages, HTML email, visual regression
testing, scraping, and driving your own application in tests. Those render well.

It is **not** a pixel-perfect renderer for arbitrary web pages. It is an
independent engine, so complex editorial layouts can diverge — a Wikipedia
article renders correctly at 1000px wide and breaks at 1200px, where the skin
switches to a three-column layout. Test the pages you care about.

## Entry points

```cfml
Browser( [options] )         // a browser with its own cookies and storage
browserFetch( url [, opts] ) // one-shot: { url, title, html, text }
isBrowserObject( value )
browserVersion()

browserServer( "cdp", { port } )         // drive this engine with Playwright/Puppeteer
```

`Browser()` options: `timeout` (ms, default 30000), `storageDir`, `proxy`,
`userAgent`, `stealth`.

**Each `Browser()` is an isolated profile.** Cookies are per-browser, not
per-process, so one request's login never leaks into another's.

## Browser

| Method | Returns | |
|---|---|---|
| `newPage()` | Page | |
| `cookies()` | array of structs | the session as a value |
| `setCookies( array )` | this | restore a saved session |
| `clearCookies()` | this | |
| `close()` | this | drops the profile and its pages |

## Page

Mutators return the page, so calls chain. Terminals return data.

**Navigation and waiting** — `goto( url [, {waitUntil}] )`, `back()`, `forward()`,
`reload()`, `waitForSelector( sel [, ms] )`, `waitForText( text [, ms] )`,
`settle( [ms] )`, `setViewport( w, h )`, `close()`.

`back()`/`forward()` walk a history stack the extension keeps itself, and error
rather than doing nothing when there is nowhere to go.

`waitUntil` is `load` (default), `domcontentloaded`, `networkidle0` or
`networkidle2`. An unrecognised value is an error, not a silent fallback.

**Interaction** — `click( sel )`, `fill( sel, value )`, `type( sel, text )`,
`press( key [, sel] )`, `selectOption( sel, value )`, `scroll( x, y )`.

These dispatch the events frameworks actually listen for, so a React controlled
input updates rather than silently ignoring you.

**Reading** — `url()`, `title()`, `content()`, `text( [sel] )`,
`markdown( [sel] )`, `links()`, `attr( sel, name )`, `count( sel )`,
`exists( sel )`, `extract( sel )`, `extractQuery( sel )`, `boundingBox( sel )`, `evaluate( js )`,
`requests()`, `fonts()`, `consoleMessages()`.

`extractQuery()` returns a real CFML query — one row per match, a `text` column plus a
column per attribute — so results drop straight into `<cfoutput query="rows">` and
Query-of-Queries.

`text()` gives you what a reader sees, not `textContent` — no stylesheets, no
script bodies. `evaluate()` returns real CFML values, and a JavaScript error
raises a catchable `browser.javascript` exception instead of returning null.

**Capture** — `screenshot( [options] )`, `pdf( [options] )`.

**Network** — `block( patterns )`, `mock( pattern, response )`.

## Screenshots

```cfml
png = page.screenshot( { width = 1200, height = 800 } );
png = page.screenshot( { fullPage = true } );
r   = page.screenshot( { detail = true } );
// r.image, r.inkCoverage, r.distinctColours, r.looksUnrendered, r.height, r.truncated
```

`detail` is worth using in anything automated. A blank page produces a perfectly
valid PNG, which is what a failed hydration, a swallowed JavaScript error, or a
`waitForSelector` that matched a loading skeleton all look like. `inkCoverage`
tells them apart:

```cfml
r = page.settle( 500 ).screenshot( { detail = true } );
if ( r.looksUnrendered ) throw( "page painted nothing — did it hydrate?" );
```

`fullPage` lays the document out at its own height, capped at the renderer's
16-megapixel budget. If it had to clip, `truncated` is true — a short capture
never passes as a complete one.

## PDF

```cfml
pdf = page.pdf( { paper = "A4", printBackground = true, landscape = false } );
```

Options: `paper` (A3/A4/A5/Letter/Legal/Tabloid) or `paperWidth`/`paperHeight`
in inches, `margin` (inches), `scale`, `landscape`, `printBackground`,
`pageRanges` (`"1-3,7"`).

> **This is a raster PDF. The text is not selectable, searchable or copyable.**
> It preserves print-media layout and slices it across pages, but there is no
> CSS paged media — no `@page`, no running headers, no page numbers.
>
> If you need a real vector PDF with selectable text, generate it from markup
> with the Typst extension. Use this one when you want *what the page looks
> like*: visual regression, an archive of a rendered report, a screenshot that
> happens to be paginated.

## Fonts

The engine bundles Liberation Sans/Serif/Mono, DejaVu Sans and Noto Color Emoji,
and deliberately never scans system fonts, so output is identical on every host.
`@font-face` webfonts **do** load. A named system face — `Verdana`,
`Segoe UI` — does not, and falls back silently.

`page.fonts()` measures what actually happened rather than guessing:

```cfml
f = page.fonts();
// f.loaded      = [ { family:"fira sans", elements:821, width:468 } ]
// f.substituted = [ { family:"verdana", renderedAs:"Liberation Sans", elements:314 } ]
// f.webFonts    = [ "fira sans" ]

if ( arrayLen( f.substituted ) ) writeLog( "brand fonts fell back: " & f.substituted[1].family );
```

Worth asserting on before shipping anything branded — the failure mode is a
document that looks fine and is in the wrong typeface.

## Console

Every page captures `console.log/info/warn/error/debug` and uncaught errors:

```cfml
for ( m in page.consoleMessages() ) writeOutput( "[#m.level#] #m.text#" );
// [warn]  a warning {"a":1}
// [error] Timer error: ReferenceError: notDefined is not defined ...
```

The first thing to check when a capture comes back blank — a page that threw
during hydration says so here. Bounded at 500 entries so a logging loop cannot
grow it without end.

## Blocking requests

```cfml
page.block( [ "*.png*", "*.jpg*", "*analytics*" ] );
```

Patterns are CDP globs, the same as Puppeteer and Playwright. **`*` is not
implicit at the end**: `*.png` means *ends with* `.png` and will miss every
cache-busted `styles.png?v=3`. Use `*.png*`.

## Mocking responses

```cfml
page.mock( "*/api/flags*", { status = 200, body = '{"feature":"on"}' } );
page.mock( "*/api/slow*",  { status = 500, body = "boom", headers = { "x-test" = "1" } } );
```

Same CDP glob rules as `block()`. `content-type` is guessed from the body when
you don't give one, so mocking a JSON endpoint needs no headers struct.

Requests that match no rule pass through to the real server untouched — worth
saying explicitly, because enabling interception makes the extension
responsible for *every* request the page makes, and a dropped one would hang
the page rather than fail it.

Blocking is currently a correctness tool, not a speed one — blocked images make
a following `screenshot()` markedly slower, because the render layer refetches
them through its own path. Fix pending upstream.

## Driving it with Playwright or Puppeteer

```cfml
// Application.cfc
function onApplicationStart() {
    application.cdp = browserServer( "cdp", { port = 9222 } );
}
```

```js
const browser = await puppeteer.connect({ browserURL: "http://127.0.0.1:9222" });
```

A Chrome DevTools Protocol endpoint inside your CFML process, so an existing
Playwright or Puppeteer suite drives this engine **with no Chrome installed**.

Two things to know. The server manages its own pages: it shares the process and
the V8 isolate with the CFML API, but not the pages you made with `newPage()`.
And it **cannot be stopped** — `stop()` verifies rather than assumes, and raises
an error saying so, because obscura's server keeps its listener open until the
process exits. Treat a browser server as process-lifetime, and start it from
`onApplicationStart` rather than per request.

It binds loopback and does **not** authenticate. Anyone who can reach the port
can drive a browser from inside your network.

## Session reuse

```cfml
// once
b = Browser();
b.newPage().goto( loginUrl ).fill( "##user", u ).fill( "##pass", p ).click( "##go" );
application.session = b.cookies();

// later, in another request
b2 = Browser();
b2.setCookies( application.session );
data = b2.newPage().goto( protectedUrl ).extract( ".row" );
```

## Testing your own app

A CFML page can drive a browser against its own server:

```cfml
p = Browser().newPage()
    .goto( "http://127.0.0.1:#cgi.server_port#/admin/login" )
    .fill( "##username", "sysadmin" ).fill( "##password", "password" )
    .click( "##loginButton" )
    .waitForSelector( ".dashboard", 5000 );

assert( p.exists( ".dashboard" ) );
fileWrite( "login-worked.png", p.screenshot() );
```

Real browser tests, in-process, with no Selenium and nothing to install.

**Loopback needs the opt-in.** The renderer refuses private and internal
addresses by default — an SSRF guard, since a page you render can name any URL
it likes in an `<img>` or a `background-image`. Testing your own app is exactly
the case where you mean it, so say so:

```sh
OBSCURA_ALLOW_PRIVATE_NETWORK=1 rustcfml tests/login.cfm
```

Without it you get `Access to private/internal IP address 127.0.0.1 is not
allowed`, which is the guard doing its job rather than a bug.

## Notes and limits

- All browser work runs on one service thread, so concurrent requests queue.
  Fine for tests, reports and modest scraping.
- Every call has a deadline. A wedged page raises a catchable CFML error rather
  than hanging the request.
- Page handles are safe to keep in `application` scope and to pass to
  `cfthread`.
