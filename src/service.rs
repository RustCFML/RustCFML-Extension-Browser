//! The browser service thread.
//!
//! Obscura's V8 isolate is single-threaded and `!Send`: a `Page` cannot cross a
//! thread boundary, and RustCFML's VM is synchronous and runs each request on a
//! `spawn_blocking` worker. Those two facts are irreconcilable in-place, so all
//! Obscura state lives on one dedicated thread and CFML talks to it by message.
//!
//! What crosses back to CFML is `{ id, tx }` — `Send + Sync`, so a page handle
//! is safe in `application` scope and inside `cfthread`, even though the page
//! itself never leaves this thread.
//!
//! Every command carries a deadline and the caller uses `recv_timeout`, so a
//! wedged page surfaces as a catchable CFML error rather than a hung request.

use std::collections::HashMap;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::OnceLock;
use std::time::Duration;

use obscura_browser::{lifecycle::WaitUntil, page::Page, BrowserContext, InterceptResolution};
use std::sync::Arc;

pub type PageId = u64;

/// Installed on every page before its own scripts run. Obscura has no console
/// capture — obscura-mcp declares a buffer and never fills it — so we keep our
/// own. A page that logs an error during hydration is the single most useful
/// thing to see when a capture comes back blank.
const CONSOLE_HOOK: &str = r#"
(function () {
  if (globalThis.__cfmlConsole) return;
  var buf = globalThis.__cfmlConsole = [];
  var levels = ["log", "info", "warn", "error", "debug"];
  for (var i = 0; i < levels.length; i++) {
    (function (level) {
      var original = console[level];
      console[level] = function () {
        try {
          var parts = [];
          for (var a = 0; a < arguments.length; a++) {
            var v = arguments[a];
            parts.push(typeof v === "string" ? v
                     : (v instanceof Error) ? (v.stack || String(v))
                     : (function () { try { return JSON.stringify(v); }
                                      catch (e) { return String(v); } })());
          }
          // Bounded: a page in a logging loop must not grow this without end.
          if (buf.length < 500) buf.push({ level: level, text: parts.join(" ") });
        } catch (e) {}
        if (original) try { original.apply(console, arguments); } catch (e) {}
      };
    })(levels[i]);
  }
  // Uncaught errors never reach console.error, and they are the ones that
  // matter most when a page fails to render.
  globalThis.addEventListener && globalThis.addEventListener("error", function (e) {
    if (buf.length < 500) buf.push({ level: "uncaught",
      text: String((e && (e.message || e.error)) || "error") });
  });
})();
"#;
pub type BrowserId = u64;

/// Mirrors `obscura_render::paint::MAX_CAPTURE_PIXELS`, which is `pub` but not
/// re-exported from the crate root. Keep in step with upstream.
pub const MAX_CAPTURE_PIXELS: f32 = 16.0 * 1024.0 * 1024.0;

/// What CFML asks the service thread to do. One variant per operation; the
/// reply channel is part of the message so the thread never needs a registry of
/// waiting callers.
pub enum Cmd {
    /// One context per Browser(). Cookies, storage and proxy belong to a
    /// context, so sharing one process-wide would leak a login from one CFML
    /// request into another — the first cut did exactly that.
    NewBrowser {
        opts: Box<BrowserOpts>,
        reply: Sender<Result<BrowserId, String>>,
    },
    CloseBrowser {
        browser: BrowserId,
    },
    Cookies {
        browser: BrowserId,
        reply: Sender<Result<Vec<CookieRow>, String>>,
    },
    SetCookies {
        browser: BrowserId,
        cookies: Vec<CookieRow>,
        reply: Sender<Result<(), String>>,
    },
    ClearCookies {
        browser: BrowserId,
        reply: Sender<Result<(), String>>,
    },
    NewPage {
        browser: BrowserId,
        reply: Sender<Result<PageId, String>>,
    },
    Goto {
        page: PageId,
        url: String,
        wait_until: WaitUntil,
        reply: Sender<Result<(), String>>,
    },
    /// url, title, html, text — one crossing instead of four.
    Snapshot {
        page: PageId,
        reply: Sender<Result<Snapshot, String>>,
    },
    /// Selector-driven reads. One variant, because they all follow the same
    /// borrow-the-dom shape and a variant each would be noise.
    Query {
        page: PageId,
        kind: Query,
        reply: Sender<Result<QueryOut, String>>,
    },
    Evaluate {
        page: PageId,
        script: String,
        reply: Sender<Result<serde_json::Value, String>>,
    },
    Screenshot {
        page: PageId,
        width: f32,
        height: f32,
        full_page: bool,
        /// Reports the height actually captured, so a truncated full-page
        /// capture is visible to the caller instead of silently short.
        reply: Sender<Result<(Vec<u8>, f32, bool), String>>,
    },
    Pdf {
        page: PageId,
        options: Box<obscura_browser::RasterPdfOptions>,
        reply: Sender<Result<Vec<u8>, String>>,
    },
    /// Poll until a condition holds or the budget runs out. Looping here
    /// rather than in CFML keeps a 5s wait to one round trip instead of fifty,
    /// and lets the runtime drive timers between polls.
    WaitFor {
        page: PageId,
        cond: WaitCond,
        timeout_ms: u64,
        poll_ms: u64,
        reply: Sender<Result<(), String>>,
    },
    Settle {
        page: PageId,
        max_ms: u64,
        reply: Sender<Result<(), String>>,
    },
    SetViewport {
        page: PageId,
        width: f32,
        height: f32,
        reply: Sender<Result<(), String>>,
    },
    /// Blocklist for subresources. Also covers stylesheets, because
    /// fetch_stylesheets consults the same table.
    Block {
        page: PageId,
        patterns: Vec<String>,
        reply: Sender<Result<(), String>>,
    },
    /// The passive network log. Returned as rows rather than a live handle:
    /// the log lives on the service thread and must not be borrowed across it.
    Network {
        page: PageId,
        reply: Sender<Result<Vec<NetRow>, String>>,
    },
    /// Obscura exposes no history API, so these are JS plus a settle. reload
    /// re-navigates instead, which is what a caller actually means by it.
    History {
        page: PageId,
        delta: i32,
        reply: Sender<Result<(), String>>,
    },
    /// Start a protocol server on this thread's runtime. The server manages
    /// its own pages: it shares the process and the V8 isolate with the CFML
    /// API, but not the pages you made with newPage().
    StartServer {
        port: u16,
        reply: Sender<Result<(), String>>,
    },
    StopServer {
        port: u16,
        reply: Sender<Result<bool, String>>,
    },
    Mock {
        page: PageId,
        rule: MockRule,
        reply: Sender<Result<(), String>>,
    },
    ClosePage {
        page: PageId,
    },
}

#[derive(Clone)]
pub struct MockRule {
    pub pattern: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// CDP glob matching, the same semantics Puppeteer and Playwright use and the
/// same one obscura applies to `set_blocked_urls`: `*` is a wildcard and there
/// is no implicit one at the end, so `*.json` means ENDS WITH `.json` and will
/// not match `data.json?v=2`.
pub fn matches_cdp_pattern(pattern: &str, url: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let mut rest = url;
    let mut first = true;
    for part in pattern.split('*') {
        if part.is_empty() {
            continue;
        }
        let Some(at) = rest.find(part) else {
            return false;
        };
        if first && !pattern.starts_with('*') && at != 0 {
            return false;
        }
        rest = &rest[at + part.len()..];
        first = false;
    }
    pattern.ends_with('*') || rest.is_empty()
}

pub struct NetRow {
    pub url: String,
    pub method: String,
    pub kind: String,
    pub status: u16,
    pub size: usize,
    pub timestamp: f64,
}

#[derive(Default)]
pub struct BrowserOpts {
    pub storage_dir: Option<std::path::PathBuf>,
    pub proxy: Option<String>,
    pub user_agent: Option<String>,
    pub stealth: bool,
}

#[derive(Clone)]
pub struct CookieRow {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: String,
    pub expires: Option<i64>,
}

pub enum WaitCond {
    Selector(String),
    Text(String),
}

pub enum Query {
    Text(String),
    Count(String),
    Attr(String, String),
    Extract(String),
    Links,
}

pub enum QueryOut {
    Text(Option<String>),
    Count(usize),
    Rows(Vec<(String, Vec<(String, String)>)>),
    List(Vec<String>),
}

#[derive(Default)]
pub struct Snapshot {
    pub url: String,
    pub title: String,
    pub html: String,
    pub text: String,
}

pub struct Service {
    tx: tokio::sync::mpsc::UnboundedSender<Cmd>,
}

static SERVICE: OnceLock<Service> = OnceLock::new();

/// Start the thread on first use. Cheap to call repeatedly.
pub fn service() -> &'static Service {
    SERVICE.get_or_init(|| {
        // Unbounded because its `send` is synchronous: the CFML side is a
        // blocking thread and must not need a runtime just to post a command.
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Cmd>();
        std::thread::Builder::new()
            .name("obscura-service".into())
            // V8 wants a generous stack; the default 2 MiB is not enough for
            // deep layout recursion on real pages.
            .stack_size(64 * 1024 * 1024)
            .spawn(move || run(rx))
            .expect("spawn obscura service thread");
        Service { tx }
    })
}

impl Service {
    /// Send a command and wait for its reply, or fail with a CFML-visible
    /// message. Never blocks forever: a dead service thread shows up as a
    /// disconnected channel, a slow one as a timeout.
    pub fn call<T>(
        &self,
        make: impl FnOnce(Sender<Result<T, String>>) -> Cmd,
        timeout: Duration,
    ) -> Result<T, String> {
        let (rtx, rrx) = channel::<Result<T, String>>();
        self.tx
            .send(make(rtx))
            .map_err(|_| "browser service thread is not running".to_string())?;
        match rrx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err(format!(
                "browser operation timed out after {}ms",
                timeout.as_millis()
            )),
            Err(RecvTimeoutError::Disconnected) => {
                Err("browser service thread stopped unexpectedly".to_string())
            }
        }
    }

    /// Fire-and-forget, for Drop paths that must not block a request.
    pub fn post(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }
}

/// The thread body. A current-thread runtime plus a LocalSet is what lets
/// `!Send` futures run at all: everything Obscura owns is created, used and
/// dropped inside this closure.
fn run(mut rx: tokio::sync::mpsc::UnboundedReceiver<Cmd>) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("browser: could not start service runtime: {e}");
            return;
        }
    };
    let local = tokio::task::LocalSet::new();

    local.block_on(&rt, async move {
        // The bool is "the CFML Browser handle is still alive". A context
        // outlives its handle whenever pages still reference it, because
        // `Browser().newPage()` drops the Browser immediately — the idiomatic
        // chain, and the one in our own README. Closing its pages with it made
        // every such call fail with "page is closed".
        let mut contexts: HashMap<BrowserId, (Arc<BrowserContext>, bool)> = HashMap::new();
        let mut page_owner: HashMap<PageId, BrowserId> = HashMap::new();
        let mut pages: HashMap<PageId, Page> = HashMap::new();
        // Kept so a later query could report what a page blocks. Obscura keeps
        // the patterns itself and init_js re-applies them to each new JS realm,
        // so there is nothing to restore after a navigation — an earlier
        // version of this file claimed otherwise and re-applied on every goto.
        // Verified: two navigations with a block in force, zero requests reach
        // the origin.
        let mut blocked: HashMap<PageId, Vec<String>> = HashMap::new();
        // Our own history stack. Obscura keeps none that survives navigation —
        // history.go(-1) is a no-op, so back() silently did nothing and left
        // the caller on the page they asked to leave.
        let mut history: HashMap<PageId, (Vec<String>, usize)> = HashMap::new();
        // Shared with the per-page interception pump running on this same
        // thread, so a later mock() is picked up without re-enabling anything.
        let mut mocks: HashMap<PageId, std::rc::Rc<std::cell::RefCell<Vec<MockRule>>>> =
            HashMap::new();
        let mut servers: HashMap<u16, tokio::task::JoinHandle<()>> = HashMap::new();
            // Pages whose subresources have already been pulled through the page
        // transport since their last navigation. Re-preparing on every capture
        // cost ~5s per frame on an image-heavy page, because resources that
        // failed are deliberately not negative-cached so a later warmup can
        // retry them — which turns a repeated capture into a repeated retry.
        let mut prepared: std::collections::HashSet<PageId> = std::collections::HashSet::new();
        let mut next_id: PageId = 1;
        let mut next_browser: BrowserId = 1;

        // `recv().await` yields to the runtime, so page timers and network
        // futures keep running while we are idle between commands.
        while let Some(cmd) = rx.recv().await {
            dispatch(
                cmd,
                &mut contexts,
                &mut pages,
                &mut page_owner,
                &mut blocked,
                &mut history,
                &mut mocks,
                &mut servers,
                &mut prepared,
                &mut next_id,
                &mut next_browser,
            )
            .await;
        }
        // Every sender dropped: the process is going away. Pages drop here,
        // on the thread that owns them, which is the only place they may.
    });
}

#[allow(clippy::too_many_arguments)]
async fn dispatch(
    cmd: Cmd,
    contexts: &mut HashMap<BrowserId, (Arc<BrowserContext>, bool)>,
    pages: &mut HashMap<PageId, Page>,
    page_owner: &mut HashMap<PageId, BrowserId>,
    blocked: &mut HashMap<PageId, Vec<String>>,
    history: &mut HashMap<PageId, (Vec<String>, usize)>,
    mocks: &mut HashMap<PageId, std::rc::Rc<std::cell::RefCell<Vec<MockRule>>>>,
    servers: &mut HashMap<u16, tokio::task::JoinHandle<()>>,
    prepared: &mut std::collections::HashSet<PageId>,
    next_id: &mut PageId,
    next_browser: &mut BrowserId,
) {
    match cmd {
        Cmd::NewBrowser { opts, reply } => {
            let id = *next_browser;
            *next_browser += 1;
            let ctx = BrowserContext::with_storage_full(
                format!("rustcfml-{id}"),
                opts.proxy.clone(),
                opts.stealth,
                opts.user_agent.clone(),
                opts.storage_dir.clone(),
            );
            contexts.insert(id, (Arc::new(ctx), true));
            let _ = reply.send(Ok(id));
        }
        Cmd::CloseBrowser { browser } => {
            // The handle is gone, but pages still using this profile keep it
            // alive: they hold its cookie jar, and a page that loses that
            // cannot make an authenticated request. Reclaimed by ClosePage
            // once the last one goes.
            if let Some(entry) = contexts.get_mut(&browser) {
                entry.1 = false;
            }
            reap(contexts, page_owner, browser);
        }
        Cmd::Cookies { browser, reply } => {
            let result = match contexts.get(&browser) {
                Some((ctx, _)) => Ok(ctx
                    .cookie_jar
                    .get_all_cookies()
                    .into_iter()
                    .map(|c| CookieRow {
                        name: c.name,
                        value: c.value,
                        domain: c.domain,
                        path: c.path,
                        secure: c.secure,
                        http_only: c.http_only,
                        same_site: c.same_site,
                        expires: c.expires,
                    })
                    .collect()),
                None => Err("browser is closed".to_string()),
            };
            let _ = reply.send(result);
        }
        Cmd::SetCookies { browser, cookies, reply } => {
            let result = match contexts.get(&browser) {
                Some((ctx, _)) => {
                    ctx.cookie_jar.set_cookies_from_cdp(
                        cookies
                            .into_iter()
                            .map(|c| obscura_net::cookies::CookieInfo {
                                name: c.name,
                                value: c.value,
                                domain: c.domain,
                                path: c.path,
                                secure: c.secure,
                                http_only: c.http_only,
                                same_site: c.same_site,
                                expires: c.expires,
                            })
                            .collect(),
                    );
                    Ok(())
                }
                None => Err("browser is closed".to_string()),
            };
            let _ = reply.send(result);
        }
        Cmd::ClearCookies { browser, reply } => {
            let result = match contexts.get(&browser) {
                Some((ctx, _)) => {
                    ctx.cookie_jar.clear();
                    Ok(())
                }
                None => Err("browser is closed".to_string()),
            };
            let _ = reply.send(result);
        }
        Cmd::NewPage { browser, reply } => {
            let Some((ctx, _)) = contexts.get(&browser) else {
                let _ = reply.send(Err("browser is closed".to_string()));
                return;
            };
            let id = *next_id;
            *next_id += 1;
            let mut page = Page::new(format!("page-{id}"), ctx.clone());
            page.add_preload_script(CONSOLE_HOOK);
            pages.insert(id, page);
            page_owner.insert(id, browser);
            let _ = reply.send(Ok(id));
        }
        Cmd::Goto {
            page,
            url,
            wait_until,
            reply,
        } => {
            let result = match pages.get_mut(&page) {
                Some(p) => {
                    let out = p
                        .navigate_with_wait(&url, wait_until)
                        .await
                        .map_err(|e| e.to_string());
                    // init_js() drops the JS realm on every navigation and does
                    // NOT restore the blocklist, so the JS half (fetch/XHR/img
                    // from scripts) is silently lost while the Rust half
                    // survives. Without this, block() looks applied and stops
                    // almost nothing on a scripted page.
                    if out.is_ok() {
                        prepared.remove(&page);
                        let landed = p.url_string();
                        let entry = history.entry(page).or_insert_with(|| (Vec::new(), 0));
                        // A new navigation truncates anything ahead, exactly as
                        // a browser does after going back and then elsewhere.
                        if !entry.0.is_empty() {
                            entry.0.truncate(entry.1 + 1);
                        }
                        if entry.0.last().map(|u| u != &landed).unwrap_or(true) {
                            entry.0.push(landed);
                            entry.1 = entry.0.len() - 1;
                        }
                    }
                    out
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::Snapshot { page, reply } => {
            let result = match pages.get(&page) {
                Some(p) => {
                    let (html, text) = p
                        .with_dom(|dom| {
                            let root = dom.document();
                            (
                                dom.outer_html(root),
                                crate::dom_util::visible_text(dom, root),
                            )
                        })
                        .unwrap_or_default();
                    Ok(Snapshot {
                        url: p.url_string(),
                        title: p.title.clone(),
                        html,
                        text,
                    })
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::Query { page, kind, reply } => {
            let result = match pages.get(&page) {
                Some(p) => {
                    let base = p.url_string();
                    p.with_dom(|dom| match &kind {
                        Query::Text(sel) => crate::dom_util::select_text(dom, sel).map(QueryOut::Text),
                        Query::Count(sel) => crate::dom_util::select_count(dom, sel).map(QueryOut::Count),
                        Query::Attr(sel, a) => {
                            crate::dom_util::select_attr(dom, sel, a).map(QueryOut::Text)
                        }
                        Query::Extract(sel) => crate::dom_util::extract(dom, sel).map(QueryOut::Rows),
                        Query::Links => Ok(QueryOut::List(crate::dom_util::links(dom, Some(&base)))),
                    })
                    .unwrap_or_else(|| Err("page has no document yet — call goto() first".into()))
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::Evaluate { page, script, reply } => {
            let result = match pages.get_mut(&page) {
                Some(p) => {
                    // BOTH Page::evaluate and Page::evaluate_for_cdp swallow a
                    // JS exception: the first returns Null, the second logs at
                    // debug and hands back an undefined RemoteObjectInfo. The
                    // detail exists only on the private js runtime, so catch it
                    // in JS instead and carry it out as a value. Without this a
                    // syntax error and a legitimate `undefined` are the same
                    // answer.
                    let wrapped = format!(
                        "(async () => {{ try {{ return {{ __ok: true, v: await ({script}) }}; }}                          catch (e) {{ return {{ __ok: false, e: String((e && e.stack) || e) }}; }} }})()"
                    );
                    let info = p.evaluate_for_cdp(&wrapped, true, true).await;
                    match info.value {
                        Some(serde_json::Value::Object(map))
                            if map.get("__ok") == Some(&serde_json::Value::Bool(true)) =>
                        {
                            Ok(map.get("v").cloned().unwrap_or(serde_json::Value::Null))
                        }
                        Some(serde_json::Value::Object(map)) => Err(map
                            .get("e")
                            .and_then(|v| v.as_str())
                            .unwrap_or("JavaScript error")
                            .to_string()),
                        // No value at all means the evaluation itself failed
                        // before our wrapper could run.
                        _ => Err("evaluate() failed — the expression could not be run \
                                  (check for a syntax error, or that the page has loaded)"
                            .to_string()),
                    }
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::Screenshot { page, width, height, full_page, reply } => {
            let result = match pages.get_mut(&page) {
                Some(p) => {
                    // Pull subresources through the page transport, so paint
                    // finds them in cache rather than fetching them through its
                    // own agent, which has no cookies, no proxy and no SSRF
                    // guard. Once per navigation, not once per frame.
                    if prepared.insert(page) {
                        let _ = p.prepare_screenshot_resources(5_000).await;
                        seal_render_cache(p);
                    }
                    let mut truncated = false;
                    // MUST match the capture size to the page's viewport.
                    //
                    // Page::screenshot tries the retained, guarded path first
                    // (screenshot_prepared_*, which disables the render layer's
                    // synchronous compatibility loader). A viewport that does
                    // not match the runtime's render key makes that return None
                    // and fall through to a full relayout from the raw DOM with
                    // a default cache — which refetches every image through the
                    // private agent, on every frame, with no blocklist, no
                    // HTTP cache and no SSRF guard.
                    //
                    // Measured on a Wikipedia article at 1280x800 with 28
                    // images: 19,565ms unmatched vs 89ms matched. Same picture,
                    // 220x, and the fast one is also the safe one.
                    let height = if full_page {
                        // Obscura has no full-page mode; lay the document out at
                        // its own scroll height instead.
                        let info = p
                            .evaluate_for_cdp(
                                "Math.ceil(Math.max(document.documentElement.scrollHeight, \
                                  document.body ? document.body.scrollHeight : 0))",
                                true,
                                false,
                            )
                            .await;
                        let want = info
                            .value
                            .and_then(|v| v.as_f64())
                            .map(|h| h as f32)
                            .unwrap_or(height)
                            .max(height);
                        // The renderer refuses a capture above MAX_CAPTURE_PIXELS
                        // (16 Mpx) and returns None, which surfaced as a baffling
                        // "no renderable document" on any long article. Budget on
                        // AREA, not height: capping height alone still busts the
                        // limit at a wide viewport.
                        let max_h = (MAX_CAPTURE_PIXELS / width.max(1.0)).floor();
                        if want > max_h {
                            truncated = true;
                            max_h
                        } else {
                            want
                        }
                    } else {
                        height
                    };
                    p.set_viewport((width, height));
                    p.screenshot((width, height))
                        .map(|png| (png, height, truncated))
                        .ok_or_else(|| {
                            format!(
                                "screenshot failed at {width:.0}x{height:.0} — the page has no \
                                 renderable document, or the capture exceeded the renderer's \
                                 {MAX_CAPTURE_PIXELS:.0}-pixel limit"
                            )
                        })
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::Pdf { page, options, reply } => {
            let result = match pages.get_mut(&page) {
                Some(p) => {
                    if prepared.insert(page) {
                        let _ = p.prepare_screenshot_resources(5_000).await;
                        seal_render_cache(p);
                    }
                    p.raster_pdf(*options).map_err(|e| e.to_string())
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::WaitFor { page, cond, timeout_ms, poll_ms, reply } => {
            let result = match pages.get_mut(&page) {
                Some(p) => wait_for(p, &cond, timeout_ms, poll_ms).await,
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::Settle { page, max_ms, reply } => {
            let result = match pages.get_mut(&page) {
                Some(p) => {
                    p.settle(max_ms).await;
                    Ok(())
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::SetViewport { page, width, height, reply } => {
            let result = match pages.get_mut(&page) {
                Some(p) => {
                    p.set_viewport((width, height));
                    Ok(())
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::Block { page, patterns, reply } => {
            let result = match pages.get_mut(&page) {
                Some(p) => {
                    blocked.insert(page, patterns.clone());
                    p.set_blocked_urls(patterns);
                    Ok(())
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::Network { page, reply } => {
            let result = match pages.get_mut(&page) {
                Some(p) => {
                    // Without this, fetch/XHR made by scripts are missing and
                    // the log silently shows only the navigation requests.
                    p.sync_js_network_events();
                    Ok(p.network_events
                        .iter()
                        .map(|e| NetRow {
                            url: e.url.clone(),
                            method: e.method.clone(),
                            kind: e.resource_type.clone(),
                            status: e.status,
                            size: e.body_size,
                            timestamp: e.timestamp,
                        })
                        .collect())
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::History { page, delta, reply } => {
            let result = match pages.get_mut(&page) {
                Some(p) => {
                    if delta == 0 {
                        let url = p.url_string();
                        p.navigate_with_wait(&url, WaitUntil::Load)
                            .await
                            .map_err(|e| e.to_string())
                    } else {
                        // Re-navigate rather than ask the page: history.go() is
                        // inert here, so this is the only way back() means what
                        // it says.
                        match history.get_mut(&page) {
                            Some((stack, idx)) => {
                                let target = if delta < 0 {
                                    idx.checked_sub(delta.unsigned_abs() as usize)
                                } else {
                                    let n = *idx + delta as usize;
                                    (n < stack.len()).then_some(n)
                                };
                                match target {
                                    Some(t) => {
                                        let url = stack[t].clone();
                                        let r = p
                                            .navigate_with_wait(&url, WaitUntil::Load)
                                            .await
                                            .map_err(|e| e.to_string());
                                        if r.is_ok() {
                                            if let Some((_, i)) = history.get_mut(&page) {
                                                *i = t;
                                            }
                                        }
                                        r
                                    }
                                    None => Err(if delta < 0 {
                                        "no earlier page in this page's history".to_string()
                                    } else {
                                        "no later page in this page's history".to_string()
                                    }),
                                }
                            }
                            None => Err("this page has not navigated anywhere yet".to_string()),
                        }
                    }
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::StartServer { port, reply } => {
            if servers.contains_key(&port) {
                let _ = reply.send(Err(format!("a browser server is already running on {port}")));
                return;
            }
            // Bind first, so "port already in use" is an error the caller sees
            // rather than a task that dies quietly a moment later.
            match std::net::TcpListener::bind(("127.0.0.1", port)) {
                Ok(probe) => drop(probe),
                Err(e) => {
                    let _ = reply.send(Err(format!("cannot listen on 127.0.0.1:{port}: {e}")));
                    return;
                }
            }
            let handle = tokio::task::spawn_local(async move {
                if let Err(e) = obscura_cdp::start_with_options(port, None, false).await {
                    eprintln!("browser: CDP server on {port} stopped: {e}");
                }
            });
            servers.insert(port, handle);
            let _ = reply.send(Ok(()));
        }
        Cmd::StopServer { port, reply } => {
            let Some(handle) = servers.remove(&port) else {
                let _ = reply.send(Ok(false));
                return;
            };
            handle.abort();
            // Aborting the task is not proof the listener closed, and it turns
            // out it does not: the port kept answering /json/version four
            // seconds after a stop() that returned true. Verify by trying to
            // bind it, and report what actually happened rather than what we
            // asked for.
            let mut closed = false;
            for _ in 0..20 {
                if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                    closed = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if !closed {
                // Put it back so a later start() says "already running" rather
                // than failing to bind for no visible reason.
                servers.insert(port, tokio::task::spawn_local(async {}));
                let _ = reply.send(Err(format!(
                    "the CDP server on port {port} did not shut down — it keeps its listener \
                     open until the process exits. Treat a browser server as process-lifetime."
                )));
                return;
            }
            let _ = reply.send(Ok(true));
        }
        Cmd::Mock { page, rule, reply } => {
            let result = match pages.get_mut(&page) {
                Some(p) => {
                    match mocks.get(&page) {
                        // Interception is already running for this page; the
                        // pump reads the shared rule list, so just add to it.
                        Some(rules) => rules.borrow_mut().push(rule),
                        None => {
                            let rules = std::rc::Rc::new(std::cell::RefCell::new(vec![rule]));
                            let mut rx = p.enable_interception();
                            let pump_rules = rules.clone();
                            // spawn_local, not spawn: InterceptedRequest is not
                            // Send and neither is anything it touches.
                            tokio::task::spawn_local(async move {
                                while let Some(req) = rx.recv().await {
                                    let hit = pump_rules
                                        .borrow()
                                        .iter()
                                        .find(|r| matches_cdp_pattern(&r.pattern, &req.url))
                                        .cloned();
                                    let resolution = match hit {
                                        Some(r) => InterceptResolution::Fulfill {
                                            status: r.status,
                                            headers: r.headers.into_iter().collect(),
                                            // A mock body arrives as CFML text, so the
                                            // base64 view is derived from it rather than
                                            // carried through as it is on the CDP path,
                                            // where the body is already base64 (#912).
                                            body_base64: base64::Engine::encode(
                                                &base64::engine::general_purpose::STANDARD,
                                                r.body.as_bytes(),
                                            ),
                                            body: r.body,
                                        },
                                        // Anything unmatched must go through
                                        // untouched: enabling interception makes
                                        // us responsible for EVERY request, and
                                        // dropping the resolver hangs the page.
                                        None => InterceptResolution::Continue {
                                            url: None,
                                            method: None,
                                            headers: None,
                                            body: None,
                                        },
                                    };
                                    let _ = req.resolver.send(resolution);
                                }
                            });
                            mocks.insert(page, rules);
                        }
                    }
                    Ok(())
                }
                None => Err(format!("page {page} is closed")),
            };
            let _ = reply.send(result);
        }
        Cmd::ClosePage { page } => {
            pages.remove(&page);
            blocked.remove(&page);
            history.remove(&page);
            mocks.remove(&page);
            prepared.remove(&page);
            if let Some(browser) = page_owner.remove(&page) {
                reap(contexts, page_owner, browser);
            }
        }
    }
}

/// Close the door on the render layer's private HTTP client.
///
/// `obscura-render`'s paint step fetches `<img>`, `background-image` and SVG
/// resources through its own blocking `ureq` agent. That agent honours no page
/// policy at all: not the SSRF guard, not `set_blocked_urls`, not the proxy,
/// not the cookie jar. Two consequences we measured: a page could make the
/// renderer fetch `127.0.0.1`, and blocking images made a capture ~20x SLOWER
/// (38s vs ~2s on a Wikipedia article) because paint refetched each one
/// synchronously, with retries.
///
/// `prepare_screenshot_resources` already pulls what it can through the page's
/// own transport. Anything still unknown afterwards is exactly what would fall
/// through to the private agent, so seed it as a known failure. Paint then
/// finds a negative entry and draws a placeholder instead of opening a socket.
///
/// The trade-off is ours to make and is the right way round: a resource the
/// page's own transport could not fetch in the warmup window renders as a
/// placeholder rather than being fetched by a client that ignores our policy.
fn seal_render_cache(page: &mut Page) -> usize {
    let Some(js) = page.js.as_mut() else {
        return 0;
    };
    let pending = js.pending_render_image_urls();
    let mut sealed = 0;
    for (url, profile) in pending {
        if js.render_image_resource_is_known(&url, profile) {
            continue;
        }
        js.seed_render_image_resource(url, profile, None);
        sealed += 1;
    }
    sealed
}

/// Poll a page until `cond` holds. Each round pumps the JS event loop briefly
/// so timers, fetches and framework renders can make progress — a poll that
/// only re-read the DOM would spin without ever letting the page change.
async fn wait_for(
    page: &mut Page,
    cond: &WaitCond,
    timeout_ms: u64,
    poll_ms: u64,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
    let poll = Duration::from_millis(poll_ms.max(10));
    loop {
        let hit = match cond {
            WaitCond::Selector(sel) => page
                .with_dom(|dom| dom.query_selector(sel).map(|o| o.is_some()))
                .unwrap_or(Ok(false))
                .map_err(|e| format!("bad selector [{sel}]: {e}"))?,
            WaitCond::Text(needle) => {
                let needle = needle.to_lowercase();
                page.with_dom(|dom| {
                    crate::dom_util::visible_text(dom, dom.document())
                        .to_lowercase()
                        .contains(&needle)
                })
                .unwrap_or(false)
            }
        };
        if hit {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(match cond {
                WaitCond::Selector(s) => format!(
                    "timed out after {timeout_ms}ms waiting for selector [{s}]"
                ),
                WaitCond::Text(t) => {
                    format!("timed out after {timeout_ms}ms waiting for text [{t}]")
                }
            });
        }
        // settle() both advances the page and provides the delay.
        page.settle(poll.as_millis() as u64).await;
    }
}

/// Drop a context once its CFML handle is gone and no pages reference it.
fn reap(
    contexts: &mut HashMap<BrowserId, (Arc<BrowserContext>, bool)>,
    page_owner: &HashMap<PageId, BrowserId>,
    browser: BrowserId,
) {
    let handle_alive = contexts.get(&browser).map(|e| e.1).unwrap_or(false);
    if handle_alive {
        return;
    }
    if page_owner.values().any(|b| *b == browser) {
        return;
    }
    contexts.remove(&browser);
}
