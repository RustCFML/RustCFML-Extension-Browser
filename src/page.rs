//! The CFML-facing `Page` object.
//!
//! Holds nothing but an id: the real `Page` lives on the service thread and
//! never moves. That is what makes this `Send + Sync`, so a page handle is safe
//! in `application` scope and inside `cfthread`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use obscura_browser::{RasterPdfOptions, RasterPdfPageRange};
use rustcfml_module::{Ctx, Error, NativeClass, Result, Value};

use crate::service::{service, Cmd, PageId, Query, QueryOut, WaitCond};
use crate::wait_until;

pub struct CfmlPage {
    pub id: PageId,
    closed: AtomicBool,
    /// Default per-command budget, in ms.
    timeout_ms: u64,
}

impl CfmlPage {
    pub fn new(id: PageId, timeout_ms: u64) -> Self {
        Self {
            id,
            closed: AtomicBool::new(false),
            timeout_ms,
        }
    }

    fn budget(&self) -> Duration {
        Duration::from_millis(self.timeout_ms + 5_000)
    }

    fn live(&self) -> Result<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(Error::new("this page has been closed"));
        }
        Ok(())
    }

    fn query(&self, kind: Query) -> Result<QueryOut> {
        self.live()?;
        service()
            .call(
                |reply| Cmd::Query {
                    page: self.id,
                    kind,
                    reply,
                },
                self.budget(),
            )
            .map_err(Error::new)
    }
}

impl CfmlPage {
    /// Run a snippet that returns "ok" or "error:<why>". Interaction verbs all
    /// share this shape, so a missing element is one error message written once.
    fn act(&self, js: String, what: &str, selector: &str) -> Result<()> {
        self.live()?;
        let out = service()
            .call(
                |reply| Cmd::Evaluate {
                    page: self.id,
                    script: js,
                    reply,
                },
                self.budget(),
            )
            .map_err(|e| Error::custom("browser.javascript", e))?;
        match out.as_str() {
            Some("ok") => Ok(()),
            Some("error:notfound") => Err(Error::new(format!(
                "{what}() found no element matching [{selector}]"
            ))),
            other => Err(Error::new(format!(
                "{what}() failed on [{selector}]: {}",
                other.unwrap_or("no result")
            ))),
        }
    }
}

fn js_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

impl Drop for CfmlPage {
    fn drop(&mut self) {
        // Best-effort and non-blocking: a page that outlives its CFML handle
        // must not pin memory on the service thread, but a Drop that waits on a
        // reply would stall whichever request happened to release the last
        // reference.
        if !self.closed.load(Ordering::SeqCst) {
            service().post(Cmd::ClosePage { page: self.id });
        }
    }
}

fn opt_str(v: &Value, key: &str) -> Option<String> {
    let f = v.key(key);
    if f.is_null() {
        None
    } else {
        Some(f.to_string())
    }
}

fn opt_f32(v: &Value, key: &str) -> Option<f32> {
    let f = v.key(key);
    if f.is_null() {
        None
    } else {
        f.as_f64().ok().map(|d| d as f32)
    }
}

fn opt_bool(v: &Value, key: &str) -> Option<bool> {
    let f = v.key(key);
    if f.is_null() {
        None
    } else {
        Some(f.as_bool().unwrap_or(false))
    }
}

impl NativeClass for CfmlPage {
    const CLASS_NAME: &'static str = "Page";

    fn new(_ctx: &Ctx, _args: &[Value]) -> Result<Self> {
        Err(Error::new(
            "create a Page with Browser().newPage(), not createObject()",
        ))
    }

    /// Declared so named arguments bind by name. Returning None for a method
    /// makes the engine refuse named args rather than bind them positionally,
    /// which would be a silent wrong answer.
    fn method_params(method: &str) -> Option<&'static str> {
        Some(match method.to_ascii_lowercase().as_str() {
            "goto" => "url,options",
            "evaluate" => "script",
            "text" | "count" | "exists" | "extract" | "extractquery" => "selector",
            "attr" => "selector,name",
            "screenshot" | "pdf" => "options",
            "waitforselector" => "selector,timeout",
            "waitfortext" => "text,timeout",
            "click" => "selector",
            "fill" => "selector,value",
            "type" => "selector,text",
            "press" => "key,selector",
            "selectoption" => "selector,value",
            "scroll" => "x,y",
            "settle" => "timeout",
            "markdown" => "selector",
            "boundingbox" => "selector",
            "block" => "patterns",
            "mock" => "pattern,response",
            "fonts" => "",
            "requests" | "consolemessages" | "back" | "forward" | "reload" => "",
            "setviewport" => "width,height",
            "url" | "title" | "content" | "links" | "close" => "",
            _ => return None,
        })
    }

    fn call<'a>(&self, ctx: &'a Ctx, method: &str, args: &[Value<'a>]) -> Result<Value<'a>> {
        match method.to_ascii_lowercase().as_str() {
            // ---- mutators: return the receiver so calls chain ----
            "goto" => {
                self.live()?;
                let url = args
                    .first()
                    .filter(|v| !v.is_null())
                    .map(|v| v.to_string())
                    .ok_or_else(|| Error::new("goto() needs a url"))?;
                let until = args
                    .get(1)
                    .filter(|v| !v.is_null())
                    .and_then(|o| opt_str(o, "waitUntil"))
                    .unwrap_or_else(|| "load".into());
                let until = wait_until(&until)?;
                service()
                    .call(
                        |reply| Cmd::Goto {
                            page: self.id,
                            url,
                            wait_until: until,
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                Ok(ctx.this())
            }
            // ---- waits ----
            "waitforselector" | "waitfortext" => {
                self.live()?;
                let target = args
                    .first()
                    .filter(|v| !v.is_null())
                    .map(|v| v.to_string())
                    .ok_or_else(|| Error::new(format!("{method}() needs a selector or text")))?;
                let timeout_ms = args
                    .get(1)
                    .filter(|v| !v.is_null())
                    .and_then(|v| v.as_i64().ok())
                    .unwrap_or(5_000)
                    .max(1) as u64;
                let cond = if method.eq_ignore_ascii_case("waitForSelector") {
                    WaitCond::Selector(target)
                } else {
                    WaitCond::Text(target)
                };
                service()
                    .call(
                        |reply| Cmd::WaitFor {
                            page: self.id,
                            cond,
                            timeout_ms,
                            poll_ms: 100,
                            reply,
                        },
                        Duration::from_millis(timeout_ms + 5_000),
                    )
                    .map_err(|e| Error::custom("browser.timeout", e))?;
                Ok(ctx.this())
            }
            "settle" => {
                self.live()?;
                let ms = args
                    .first()
                    .filter(|v| !v.is_null())
                    .and_then(|v| v.as_i64().ok())
                    .unwrap_or(1_000)
                    .max(0) as u64;
                service()
                    .call(
                        |reply| Cmd::Settle {
                            page: self.id,
                            max_ms: ms,
                            reply,
                        },
                        Duration::from_millis(ms + 10_000),
                    )
                    .map_err(Error::new)?;
                Ok(ctx.this())
            }
            "setviewport" => {
                self.live()?;
                let w = args.first().and_then(|v| v.as_f64().ok()).unwrap_or(1280.0) as f32;
                let h = args.get(1).and_then(|v| v.as_f64().ok()).unwrap_or(800.0) as f32;
                service()
                    .call(
                        |reply| Cmd::SetViewport {
                            page: self.id,
                            width: w,
                            height: h,
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                Ok(ctx.this())
            }

            // ---- interaction ----
            //
            // These are JS, because Obscura implements no native click or type:
            // Page has settle() and nothing else. The two globals below come
            // from obscura's bootstrap and are not optional garnish —
            // __obscura_setFieldValue goes through the native value setter so a
            // React controlled input actually updates, and __obscura_markTrusted
            // makes the event isTrusted so framework handlers do not ignore it.
            // Assigning el.value and firing a plain Event looks like it works
            // and silently does nothing on any modern framework.
            "click" => {
                let sel = args.first().map(|v| v.to_string()).unwrap_or_default();
                self.act(
                    format!(
                        r#"(function(){{ var el=document.querySelector({s});
                           if(!el) return "error:notfound"; el.click(); return "ok"; }})()"#,
                        s = js_str(&sel)
                    ),
                    "click",
                    &sel,
                )?;
                Ok(ctx.this())
            }
            "fill" | "type" => {
                let sel = args.first().map(|v| v.to_string()).unwrap_or_default();
                let val = args.get(1).map(|v| v.to_string()).unwrap_or_default();
                let append = method.eq_ignore_ascii_case("type");
                self.act(
                    format!(
                        r#"(function(){{ var el=document.querySelector({s});
                           if(!el) return "error:notfound";
                           var v = {append} ? ((el.value||"") + {v}) : {v};
                           globalThis.__obscura_setFieldValue(el,"value",v);
                           el.dispatchEvent(globalThis.__obscura_markTrusted(new Event("input",{{bubbles:true}})));
                           el.dispatchEvent(globalThis.__obscura_markTrusted(new Event("change",{{bubbles:true}})));
                           return "ok"; }})()"#,
                        s = js_str(&sel),
                        v = js_str(&val),
                        append = append
                    ),
                    if append { "type" } else { "fill" },
                    &sel,
                )?;
                Ok(ctx.this())
            }
            "selectoption" => {
                let sel = args.first().map(|v| v.to_string()).unwrap_or_default();
                let val = args.get(1).map(|v| v.to_string()).unwrap_or_default();
                self.act(
                    format!(
                        r#"(function(){{ var el=document.querySelector({s});
                           if(!el) return "error:notfound";
                           globalThis.__obscura_setFieldValue(el,"value",{v});
                           el.dispatchEvent(globalThis.__obscura_markTrusted(new Event("change",{{bubbles:true}})));
                           return "ok"; }})()"#,
                        s = js_str(&sel),
                        v = js_str(&val)
                    ),
                    "selectOption",
                    &sel,
                )?;
                Ok(ctx.this())
            }
            "press" => {
                let key = args.first().map(|v| v.to_string()).unwrap_or_default();
                let sel = args.get(1).filter(|v| !v.is_null()).map(|v| v.to_string());
                let target = match &sel {
                    Some(s) => format!("document.querySelector({})", js_str(s)),
                    None => "(document.activeElement || document.body)".to_string(),
                };
                self.act(
                    format!(
                        r#"(function(){{ var t={t};
                           if(!t) return "error:notfound";
                           t.dispatchEvent(new KeyboardEvent("keydown",{{key:{k},bubbles:true}}));
                           t.dispatchEvent(new KeyboardEvent("keyup",{{key:{k},bubbles:true}}));
                           if({k}==="Enter" && t.form && typeof t.form.requestSubmit==="function") t.form.requestSubmit();
                           return "ok"; }})()"#,
                        t = target,
                        k = js_str(&key)
                    ),
                    "press",
                    sel.as_deref().unwrap_or("<active element>"),
                )?;
                Ok(ctx.this())
            }
            "scroll" => {
                let x = args.first().and_then(|v| v.as_f64().ok()).unwrap_or(0.0);
                let y = args.get(1).and_then(|v| v.as_f64().ok()).unwrap_or(0.0);
                self.act(
                    format!(
                        r#"(function(){{ window.scrollTo({x},{y});
                           try{{window.dispatchEvent(new Event("scroll",{{bubbles:true}}));}}catch(e){{}}
                           return "ok"; }})()"#
                    ),
                    "scroll",
                    "window",
                )?;
                Ok(ctx.this())
            }

            "back" | "forward" | "reload" => {
                self.live()?;
                let delta = match method.to_ascii_lowercase().as_str() {
                    "back" => -1,
                    "forward" => 1,
                    _ => 0,
                };
                service()
                    .call(
                        |reply| Cmd::History {
                            page: self.id,
                            delta,
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                Ok(ctx.this())
            }
            "consolemessages" => {
                self.live()?;
                let v = service()
                    .call(
                        |reply| Cmd::Evaluate {
                            page: self.id,
                            script: "(globalThis.__cfmlConsole || [])".to_string(),
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                crate::convert::json_to_cfml(ctx, &v)
            }
            "close" => {
                if !self.closed.swap(true, Ordering::SeqCst) {
                    service().post(Cmd::ClosePage { page: self.id });
                }
                Ok(ctx.this())
            }

            // ---- terminals: return data ----
            "url" | "title" | "content" => {
                self.live()?;
                let snap = service()
                    .call(
                        |reply| Cmd::Snapshot {
                            page: self.id,
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                Ok(ctx.string(match method.to_ascii_lowercase().as_str() {
                    "url" => snap.url,
                    "title" => snap.title,
                    _ => snap.html,
                }))
            }
            "text" => {
                let sel = args.first().filter(|v| !v.is_null()).map(|v| v.to_string());
                match sel {
                    None => {
                        self.live()?;
                        let snap = service()
                            .call(
                                |reply| Cmd::Snapshot {
                                    page: self.id,
                                    reply,
                                },
                                self.budget(),
                            )
                            .map_err(Error::new)?;
                        Ok(ctx.string(snap.text))
                    }
                    Some(sel) => match self.query(Query::Text(sel))? {
                        QueryOut::Text(Some(t)) => Ok(ctx.string(t)),
                        _ => Ok(ctx.null()),
                    },
                }
            }
            "attr" => {
                let sel = args.first().map(|v| v.to_string()).unwrap_or_default();
                let name = args.get(1).map(|v| v.to_string()).unwrap_or_default();
                match self.query(Query::Attr(sel, name))? {
                    QueryOut::Text(Some(t)) => Ok(ctx.string(t)),
                    _ => Ok(ctx.null()),
                }
            }
            "count" => match self.query(Query::Count(
                args.first().map(|v| v.to_string()).unwrap_or_default(),
            ))? {
                QueryOut::Count(n) => Ok(ctx.int(n as i64)),
                _ => Ok(ctx.int(0)),
            },
            "exists" => match self.query(Query::Count(
                args.first().map(|v| v.to_string()).unwrap_or_default(),
            ))? {
                QueryOut::Count(n) => Ok(ctx.bool(n > 0)),
                _ => Ok(ctx.bool(false)),
            },
            "links" => match self.query(Query::Links)? {
                QueryOut::List(items) => {
                    let arr = ctx.array_with_capacity(items.len());
                    for (i, href) in items.into_iter().enumerate() {
                        arr.set(i, ctx.string(href))?;
                    }
                    Ok(arr)
                }
                _ => Ok(ctx.array()),
            },
            "extract" => match self.query(Query::Extract(
                args.first().map(|v| v.to_string()).unwrap_or_default(),
            ))? {
                QueryOut::Rows(rows) => {
                    let arr = ctx.array_with_capacity(rows.len());
                    for (i, (text, attrs)) in rows.into_iter().enumerate() {
                        let row = ctx.strukt();
                        row.put("text", ctx.string(text))?;
                        for (k, v) in attrs {
                            row.put(&k, ctx.string(v))?;
                        }
                        arr.set(i, row)?;
                    }
                    Ok(arr)
                }
                _ => Ok(ctx.array()),
            },
            "markdown" => {
                self.live()?;
                // htmd in every build, never Obscura's V8 converter: that one
                // is cruder, and routing through it would make markdown output
                // depend on which build you installed.
                let snap = service()
                    .call(
                        |reply| Cmd::Snapshot {
                            page: self.id,
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                let html = match args.first().filter(|v| !v.is_null()) {
                    // The document, not <body>, would carry <head> through: the
                    // first cut emitted the page title and the whole stylesheet
                    // above the content. Same trap as text() — reach for the
                    // rendered subtree, never the document.
                    None => {
                        let js =
                            "(function(){return document.body?document.body.outerHTML:null;})()";
                        let v = service()
                            .call(
                                |reply| Cmd::Evaluate {
                                    page: self.id,
                                    script: js.to_string(),
                                    reply,
                                },
                                self.budget(),
                            )
                            .map_err(Error::new)?;
                        v.as_str().map(|s| s.to_string()).unwrap_or(snap.html)
                    }
                    Some(sel) => {
                        let sel = sel.to_string();
                        let js = format!(
                            r#"(function(){{var e=document.querySelector({s});
                               return e?e.outerHTML:null;}})()"#,
                            s = js_str(&sel)
                        );
                        let v = service()
                            .call(
                                |reply| Cmd::Evaluate {
                                    page: self.id,
                                    script: js,
                                    reply,
                                },
                                self.budget(),
                            )
                            .map_err(Error::new)?;
                        match v.as_str() {
                            Some(h) => h.to_string(),
                            None => {
                                return Err(Error::new(format!(
                                    "markdown() found no element matching [{sel}]"
                                )))
                            }
                        }
                    }
                };
                let md = crate::md::to_markdown(&html)
                    .map_err(|e| Error::new(format!("markdown conversion failed: {e}")))?;
                Ok(ctx.string(md))
            }
            "boundingbox" => {
                self.live()?;
                let sel = args.first().map(|v| v.to_string()).unwrap_or_default();
                // Real numbers only because the render layer is compiled in:
                // without it getBoundingClientRect returns zeroes.
                let js = format!(
                    r#"(function(){{var e=document.querySelector({s});if(!e)return null;
                       var r=e.getBoundingClientRect();
                       return {{x:r.x,y:r.y,width:r.width,height:r.height,
                                top:r.top,left:r.left,bottom:r.bottom,right:r.right}};}})()"#,
                    s = js_str(&sel)
                );
                let v = service()
                    .call(
                        |reply| Cmd::Evaluate {
                            page: self.id,
                            script: js,
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                if v.is_null() {
                    return Ok(ctx.null());
                }
                crate::convert::json_to_cfml(ctx, &v)
            }
            "block" => {
                self.live()?;
                let mut patterns = Vec::new();
                if let Some(v) = args.first().filter(|v| !v.is_null()) {
                    match v.len() {
                        Ok(n) => {
                            for i in 0..n {
                                patterns.push(v.get(i).to_string());
                            }
                        }
                        // A bare string is a reasonable thing to pass for one
                        // pattern; treating it as a one-element list beats an
                        // error nobody learns anything from.
                        Err(_) => patterns.push(v.to_string()),
                    }
                }
                service()
                    .call(
                        |reply| Cmd::Block {
                            page: self.id,
                            patterns,
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                Ok(ctx.this())
            }
            "fonts" => {
                self.live()?;
                let v = service()
                    .call(
                        |reply| Cmd::Evaluate {
                            page: self.id,
                            script: crate::fonts::report_js(),
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                crate::convert::json_to_cfml(ctx, &v)
            }
            "mock" => {
                self.live()?;
                let pattern = args
                    .first()
                    .filter(|v| !v.is_null())
                    .map(|v| v.to_string())
                    .ok_or_else(|| Error::new("mock() needs a url pattern"))?;
                let o = args.get(1).filter(|v| !v.is_null());
                let status = o
                    .map(|o| o.key("status"))
                    .filter(|v| !v.is_null())
                    .and_then(|v| v.as_i64().ok())
                    .unwrap_or(200) as u16;
                let body = o
                    .map(|o| o.key("body"))
                    .filter(|v| !v.is_null())
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let mut headers: Vec<(String, String)> = Vec::new();
                if let Some(h) = o.map(|o| o.key("headers")).filter(|v| !v.is_null()) {
                    for k in h.keys().unwrap_or_default() {
                        headers.push((k.to_string(), h.key(k).to_string()));
                    }
                }
                if !headers
                    .iter()
                    .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                {
                    // Guess from the body so the common case — mocking a JSON
                    // endpoint — does not need a headers struct every time.
                    let looks_json =
                        body.trim_start().starts_with('{') || body.trim_start().starts_with('[');
                    headers.push((
                        "content-type".into(),
                        if looks_json {
                            "application/json".into()
                        } else {
                            "text/plain".into()
                        },
                    ));
                }
                service()
                    .call(
                        |reply| Cmd::Mock {
                            page: self.id,
                            rule: crate::service::MockRule {
                                pattern,
                                status,
                                headers,
                                body,
                            },
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                Ok(ctx.this())
            }
            "requests" => {
                self.live()?;
                let rows = service()
                    .call(
                        |reply| Cmd::Network {
                            page: self.id,
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                let arr = ctx.array_with_capacity(rows.len());
                for (i, r) in rows.into_iter().enumerate() {
                    let row = ctx.strukt();
                    row.put("url", ctx.string(r.url))?;
                    row.put("method", ctx.string(r.method))?;
                    row.put("type", ctx.string(r.kind))?;
                    row.put("status", ctx.int(r.status as i64))?;
                    row.put("size", ctx.int(r.size as i64))?;
                    row.put("timestamp", ctx.double(r.timestamp))?;
                    arr.set(i, row)?;
                }
                Ok(arr)
            }
            // CFML developers live in queries: `<cfoutput query="rows">` is the
            // idiom, and an array of structs is not. Same extraction, shaped for
            // the code people already write.
            "extractquery" => {
                let sel = args.first().map(|v| v.to_string()).unwrap_or_default();
                let QueryOut::Rows(rows) = self.query(Query::Extract(sel))? else {
                    return Err(Error::new("extractQuery() got an unexpected result"));
                };
                // A query needs one column set up front, but different matches
                // carry different attributes, so collect the union first and
                // keep it stable: "text" leads, the rest in first-seen order.
                let mut cols: Vec<String> = vec!["text".to_string()];
                for (_, attrs) in &rows {
                    for (k, _) in attrs {
                        let k = k.to_ascii_lowercase();
                        if !cols.iter().any(|c| c == &k) {
                            cols.push(k);
                        }
                    }
                }
                let col_refs: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
                let q = ctx.query(&col_refs)?;
                for (text, attrs) in rows {
                    // query_add_row takes cells in COLUMN ORDER, not a struct.
                    let row = ctx.array_with_capacity(cols.len());
                    for (i, col) in cols.iter().enumerate() {
                        let cell = if col == "text" {
                            text.clone()
                        } else {
                            // An element without this attribute still needs a
                            // cell, or the query comes out ragged.
                            attrs
                                .iter()
                                .find(|(k, _)| k.eq_ignore_ascii_case(col))
                                .map(|(_, v)| v.clone())
                                .unwrap_or_default()
                        };
                        row.set(i, ctx.string(cell))?;
                    }
                    q.query_add_row(row)?;
                }
                Ok(q)
            }
            "evaluate" => {
                self.live()?;
                let script = args
                    .first()
                    .filter(|v| !v.is_null())
                    .map(|v| v.to_string())
                    .ok_or_else(|| Error::new("evaluate() needs a script"))?;
                let json = service()
                    .call(
                        |reply| Cmd::Evaluate {
                            page: self.id,
                            script,
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(|e| Error::custom("browser.javascript", e))?;
                crate::convert::json_to_cfml(ctx, &json)
            }
            "screenshot" => {
                self.live()?;
                let o = args.first().filter(|v| !v.is_null());
                let w = o.and_then(|o| opt_f32(o, "width")).unwrap_or(1280.0);
                let h = o.and_then(|o| opt_f32(o, "height")).unwrap_or(800.0);
                let full = o.and_then(|o| opt_bool(o, "fullPage")).unwrap_or(false);
                let detail = o.and_then(|o| opt_bool(o, "detail")).unwrap_or(false);
                let (png, captured_h, truncated) = service()
                    .call(
                        |reply| Cmd::Screenshot {
                            page: self.id,
                            width: w,
                            height: h,
                            full_page: full,
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                if !detail {
                    return Ok(ctx.binary(&png));
                }
                let _ = truncated;
                // The honest post-condition for settle(): a capture that painted
                // nothing is what a failed hydration, a swallowed JS error, or a
                // waitForSelector that matched a skeleton all look like. Cheap,
                // because we already have the pixels.
                let (coverage, colours) = crate::ink::coverage(&png);
                let out = ctx.strukt();
                out.put("image", ctx.binary(&png))?;
                out.put("inkCoverage", ctx.double(coverage))?;
                out.put("distinctColours", ctx.int(colours as i64))?;
                out.put("looksUnrendered", ctx.bool(coverage < 0.001))?;
                out.put("height", ctx.double(captured_h as f64))?;
                // Never let a clipped full-page capture pass as complete.
                out.put("truncated", ctx.bool(truncated))?;
                Ok(out)
            }
            "pdf" => {
                self.live()?;
                let o = args.first().filter(|v| !v.is_null());
                let mut opts = RasterPdfOptions::default();
                if let Some(o) = o {
                    if let Some(p) = opt_str(o, "paper") {
                        let (w, h) = paper_size(&p)?;
                        opts.paper_width_in = w;
                        opts.paper_height_in = h;
                    }
                    if let Some(v) = opt_f32(o, "paperWidth") {
                        opts.paper_width_in = v;
                    }
                    if let Some(v) = opt_f32(o, "paperHeight") {
                        opts.paper_height_in = v;
                    }
                    if let Some(v) = opt_bool(o, "landscape") {
                        opts.landscape = v;
                    }
                    if let Some(v) = opt_bool(o, "printBackground") {
                        opts.print_background = v;
                    }
                    if let Some(v) = opt_f32(o, "scale") {
                        opts.scale = v;
                    }
                    if let Some(v) = opt_f32(o, "margin") {
                        opts.margin_top_in = v;
                        opts.margin_bottom_in = v;
                        opts.margin_left_in = v;
                        opts.margin_right_in = v;
                    }
                    let pages = o.key("pageRanges");
                    if !pages.is_null() {
                        opts.page_ranges = parse_ranges(&pages.to_string())?;
                    }
                }
                let pdf = service()
                    .call(
                        |reply| Cmd::Pdf {
                            page: self.id,
                            options: Box::new(opts.clone()),
                            reply,
                        },
                        self.budget(),
                    )
                    .map_err(Error::new)?;
                Ok(ctx.binary(&pdf))
            }

            other => Err(Error::new(format!("Page has no method [{other}]"))),
        }
    }
}

/// Named paper sizes in inches. Unknown names are an error naming what is
/// available, rather than a silent fall back to Letter.
fn paper_size(name: &str) -> Result<(f32, f32)> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "a3" => (11.7, 16.5),
        "a4" => (8.27, 11.7),
        "a5" => (5.83, 8.27),
        "letter" => (8.5, 11.0),
        "legal" => (8.5, 14.0),
        "tabloid" => (11.0, 17.0),
        other => {
            return Err(Error::new(format!(
                "unknown paper [{other}] — expected A3, A4, A5, Letter, Legal or Tabloid, \
                 or give paperWidth/paperHeight in inches"
            )))
        }
    })
}

/// "1-3,7" in the CDP spelling.
fn parse_ranges(spec: &str) -> Result<Vec<RasterPdfPageRange>> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let range = match part.split_once('-') {
            None => {
                let n = part.parse::<usize>().map_err(|_| {
                    Error::new(format!(
                        "bad page range [{part}] — expected a number or 'from-to'"
                    ))
                })?;
                RasterPdfPageRange {
                    start: Some(n),
                    end: Some(n),
                }
            }
            Some((a, b)) => RasterPdfPageRange {
                start: if a.trim().is_empty() {
                    None
                } else {
                    Some(
                        a.trim()
                            .parse()
                            .map_err(|_| Error::new(format!("bad page range [{part}]")))?,
                    )
                },
                end: if b.trim().is_empty() {
                    None
                } else {
                    Some(
                        b.trim()
                            .parse()
                            .map_err(|_| Error::new(format!("bad page range [{part}]")))?,
                    )
                },
            },
        };
        out.push(range);
    }
    Ok(out)
}
