//! RustCFML browser extension: Obscura as a `.rcx`.

mod convert;
mod dom_util;
mod fonts;
mod ink;
mod md;
mod page;
mod service;

use std::time::Duration;

use obscura_browser::lifecycle::WaitUntil;
use rustcfml_module::{module, Ctx, Error, Result, Value};
use service::{service, Cmd};

/// Map CFML's `waitUntil` spelling onto Obscura's. Unknown values are an error
/// rather than a silent fallback: a typo that quietly changes when a capture
/// happens is the kind of bug nobody finds.
pub(crate) fn wait_until(name: &str) -> Result<WaitUntil> {
    match name.to_ascii_lowercase().as_str() {
        "load" | "" => Ok(WaitUntil::Load),
        "domcontentloaded" => Ok(WaitUntil::DomContentLoaded),
        "networkidle0" => Ok(WaitUntil::NetworkIdle0),
        "networkidle2" => Ok(WaitUntil::NetworkIdle2),
        other => Err(Error::new(format!(
            "unknown waitUntil [{other}] — expected load, domcontentloaded, networkidle0 or networkidle2"
        ))),
    }
}

/// `browserFetch( url [, options] )` — navigate once and return the page.
///
/// Returns `{ url, title, html, text }`. This is the one-shot path: no page
/// object, no lifecycle to manage.
fn browser_fetch<'a>(ctx: &'a Ctx, args: &[Value<'a>]) -> Result<Value<'a>> {
    let url = match args.first() {
        Some(v) if !v.is_null() => v.to_string(),
        _ => return Err(Error::new("browserFetch() needs a url")),
    };

    let (mut until, mut timeout_ms) = ("load".to_string(), 30_000u64);
    if let Some(opts) = args.get(1) {
        if !opts.is_null() {
            let w = opts.key("waitUntil");
            if !w.is_null() {
                until = w.to_string();
            }
            let t = opts.key("timeout");
            if !t.is_null() {
                timeout_ms = t.as_i64().unwrap_or(30_000).max(1) as u64;
            }
        }
    }
    let until = wait_until(&until)?;

    let svc = service();
    // Give the reply a little more headroom than the navigation itself, so a
    // navigation that fails on its own deadline reports its real error instead
    // of being masked by our timeout.
    let budget = Duration::from_millis(timeout_ms + 5_000);

    // A throwaway context, so a one-shot fetch never shares cookies with
    // anything else in the process.
    let browser = svc
        .call(
            |reply| Cmd::NewBrowser { opts: Box::default(), reply },
            budget,
        )
        .map_err(Error::new)?;
    let page = svc
        .call(|reply| Cmd::NewPage { browser, reply }, budget)
        .map_err(Error::new)?;

    let result = (|| {
        svc.call(
            |reply| Cmd::Goto {
                page,
                url: url.clone(),
                wait_until: until,
                reply,
            },
            budget,
        )?;
        svc.call(|reply| Cmd::Snapshot { page, reply }, budget)
    })();

    // Always release, including on the error path.
    svc.post(Cmd::ClosePage { page });
    svc.post(Cmd::CloseBrowser { browser });

    let snap = result.map_err(Error::new)?;
    let out = ctx.strukt();
    out.put("url", ctx.string(snap.url))?;
    out.put("title", ctx.string(snap.title))?;
    out.put("html", ctx.string(snap.html))?;
    out.put("text", ctx.string(snap.text))?;
    Ok(out)
}

/// `browserVersion()` — what this build can do, for support questions.
fn browser_version<'a>(ctx: &'a Ctx, _args: &[Value<'a>]) -> Result<Value<'a>> {
    let out = ctx.strukt();
    out.put("extension", ctx.string("0.1.0"))?;
    out.put("javascript", ctx.bool(true))?;
    // Both are unconditional in this build: the extension ships one artifact
    // with V8 and the render layer on. Reported anyway so support questions can
    // be answered from CFML rather than from the filename.
    out.put("render", ctx.bool(true))?;
    Ok(out)
}

/// `Browser( [options] )` — a handle you make pages from.
///
/// Options are read here and applied per page, so the object itself stays
/// stateless; there is one Obscura browser context per process either way.
pub struct CfmlBrowser {
    id: service::BrowserId,
    timeout_ms: u64,
}

impl Drop for CfmlBrowser {
    fn drop(&mut self) {
        // Non-blocking: releases the context, its cookie jar and any pages it
        // still owns. A leaked Browser must not pin a profile for the life of
        // the process.
        service().post(Cmd::CloseBrowser { browser: self.id });
    }
}

impl rustcfml_module::NativeClass for CfmlBrowser {
    const CLASS_NAME: &'static str = "Browser";

    fn new(_ctx: &Ctx, args: &[Value]) -> Result<Self> {
        let o = args.first().filter(|v| !v.is_null());
        let get = |k: &str| -> Option<String> {
            o.map(|o| o.key(k))
                .filter(|v| !v.is_null())
                .map(|v| v.to_string())
        };
        let timeout_ms = o
            .map(|o| o.key("timeout"))
            .filter(|v| !v.is_null())
            .and_then(|v| v.as_i64().ok())
            .unwrap_or(30_000)
            .max(1) as u64;
        let opts = service::BrowserOpts {
            storage_dir: get("storageDir").map(std::path::PathBuf::from),
            proxy: get("proxy"),
            user_agent: get("userAgent"),
            stealth: o
                .map(|o| o.key("stealth"))
                .filter(|v| !v.is_null())
                .and_then(|v| v.as_bool().ok())
                .unwrap_or(false),
        };
        let id = service()
            .call(
                |reply| Cmd::NewBrowser { opts: Box::new(opts), reply },
                std::time::Duration::from_millis(timeout_ms + 5_000),
            )
            .map_err(Error::new)?;
        Ok(CfmlBrowser { id, timeout_ms })
    }

    fn method_params(method: &str) -> Option<&'static str> {
        Some(match method.to_ascii_lowercase().as_str() {
            "newpage" | "cookies" | "clearcookies" | "close" => "",
            "setcookies" => "cookies",
            _ => return None,
        })
    }

    fn call<'a>(&self, ctx: &'a Ctx, method: &str, args: &[Value<'a>]) -> Result<Value<'a>> {
        let budget = std::time::Duration::from_millis(self.timeout_ms + 5_000);
        match method.to_ascii_lowercase().as_str() {
            "newpage" => {
                let id = service()
                    .call(
                        |reply| Cmd::NewPage { browser: self.id, reply },
                        budget,
                    )
                    .map_err(Error::new)?;
                Ok(ctx.new_object(page::CfmlPage::new(id, self.timeout_ms)))
            }
            // The session, in a form you can persist and hand back later —
            // log in once, keep the struct, skip the login next time.
            "cookies" => {
                let rows = service()
                    .call(|reply| Cmd::Cookies { browser: self.id, reply }, budget)
                    .map_err(Error::new)?;
                let arr = ctx.array_with_capacity(rows.len());
                for (i, c) in rows.into_iter().enumerate() {
                    let s = ctx.strukt();
                    s.put("name", ctx.string(c.name))?;
                    s.put("value", ctx.string(c.value))?;
                    s.put("domain", ctx.string(c.domain))?;
                    s.put("path", ctx.string(c.path))?;
                    s.put("secure", ctx.bool(c.secure))?;
                    s.put("httpOnly", ctx.bool(c.http_only))?;
                    s.put("sameSite", ctx.string(c.same_site))?;
                    match c.expires {
                        Some(e) => s.put("expires", ctx.int(e))?,
                        None => s.put("expires", ctx.null())?,
                    }
                    arr.set(i, s)?;
                }
                Ok(arr)
            }
            "setcookies" => {
                let mut cookies = Vec::new();
                if let Some(v) = args.first().filter(|v| !v.is_null()) {
                    let n = v.len().map_err(|_| {
                        Error::new("setCookies() takes an array of cookie structs")
                    })?;
                    for i in 0..n {
                        let c = v.get(i);
                        let s = |k: &str| {
                            let f = c.key(k);
                            if f.is_null() { String::new() } else { f.to_string() }
                        };
                        let b = |k: &str| c.key(k).as_bool().unwrap_or(false);
                        if s("name").is_empty() {
                            return Err(Error::new(format!(
                                "setCookies(): cookie {} has no name", i + 1
                            )));
                        }
                        cookies.push(service::CookieRow {
                            name: s("name"),
                            value: s("value"),
                            domain: s("domain"),
                            path: if s("path").is_empty() { "/".into() } else { s("path") },
                            secure: b("secure"),
                            http_only: b("httpOnly"),
                            same_site: s("sameSite"),
                            expires: c.key("expires").as_i64().ok(),
                        });
                    }
                }
                service()
                    .call(
                        |reply| Cmd::SetCookies { browser: self.id, cookies, reply },
                        budget,
                    )
                    .map_err(Error::new)?;
                Ok(ctx.this())
            }
            "clearcookies" => {
                service()
                    .call(|reply| Cmd::ClearCookies { browser: self.id, reply }, budget)
                    .map_err(Error::new)?;
                Ok(ctx.this())
            }
            "close" => {
                service().post(Cmd::CloseBrowser { browser: self.id });
                Ok(ctx.this())
            }
            other => Err(Error::new(format!("Browser has no method [{other}]"))),
        }
    }
}

/// `Browser( [options] )`
fn browser<'a>(ctx: &'a Ctx, args: &[Value<'a>]) -> Result<Value<'a>> {
    Ok(ctx.new_object(<CfmlBrowser as rustcfml_module::NativeClass>::new(ctx, args)?))
}

/// `browserServer( "cdp", { port = 9222 } )`
///
/// Starts a Chrome DevTools Protocol endpoint in this process, so Puppeteer or
/// Playwright can drive the same engine with no Chrome installed. The server
/// manages its own pages: it shares the process and the V8 isolate with the
/// CFML API, not the pages you made with newPage().
pub struct CfmlServer {
    port: u16,
}

impl rustcfml_module::NativeClass for CfmlServer {
    const CLASS_NAME: &'static str = "BrowserServer";

    fn new(_ctx: &Ctx, _args: &[Value]) -> Result<Self> {
        Err(Error::new("start a server with browserServer( kind, options )"))
    }

    fn method_params(method: &str) -> Option<&'static str> {
        Some(match method.to_ascii_lowercase().as_str() {
            "stop" | "port" | "url" => "",
            _ => return None,
        })
    }

    fn call<'a>(&self, ctx: &'a Ctx, method: &str, _args: &[Value<'a>]) -> Result<Value<'a>> {
        match method.to_ascii_lowercase().as_str() {
            "port" => Ok(ctx.int(self.port as i64)),
            "url" => Ok(ctx.string(format!("ws://127.0.0.1:{}", self.port))),
            "stop" => {
                let stopped = service()
                    .call(
                        |reply| Cmd::StopServer { port: self.port, reply },
                        std::time::Duration::from_millis(5_000),
                    )
                    .map_err(Error::new)?;
                Ok(ctx.bool(stopped))
            }
            other => Err(Error::new(format!("BrowserServer has no method [{other}]"))),
        }
    }
}

fn browser_server<'a>(ctx: &'a Ctx, args: &[Value<'a>]) -> Result<Value<'a>> {
    let kind = args
        .first()
        .filter(|v| !v.is_null())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "cdp".into());
    if !kind.eq_ignore_ascii_case("cdp") {
        return Err(Error::new(format!(
            "browserServer(): unknown kind [{kind}] — only \"cdp\" is supported in this build"
        )));
    }
    let port = args
        .get(1)
        .filter(|v| !v.is_null())
        .map(|o| o.key("port"))
        .filter(|v| !v.is_null())
        .and_then(|v| v.as_i64().ok())
        .unwrap_or(9222);
    if !(1..=65535).contains(&port) {
        return Err(Error::new(format!("browserServer(): port {port} is out of range")));
    }
    let port = port as u16;
    service()
        .call(
            |reply| Cmd::StartServer { port, reply },
            std::time::Duration::from_millis(10_000),
        )
        .map_err(Error::new)?;
    Ok(ctx.new_object(CfmlServer { port }))
}

/// `isBrowserObject( value )`
fn is_browser_object<'a>(ctx: &'a Ctx, args: &[Value<'a>]) -> Result<Value<'a>> {
    let is = args
        .first()
        .map(|v| {
            let n = v.native_class_name().unwrap_or("");
            n == "Browser" || n == "Page"
        })
        .unwrap_or(false);
    Ok(ctx.bool(is))
}

module! {
    name: "browser",
    version: "0.1.0",
    bifs: {
        "browserFetch"     => browser_fetch,
        "browserVersion"   => browser_version,
        "Browser"          => browser,
        "isBrowserObject"  => is_browser_object,
        "browserServer"    => browser_server,
    },
    classes: { CfmlBrowser, page::CfmlPage, CfmlServer },
}
