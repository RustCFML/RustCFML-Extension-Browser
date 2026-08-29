//! serde_json -> CFML values.

use rustcfml_module::{Ctx, Result, Value};

pub fn json_to_cfml<'a>(ctx: &'a Ctx, v: &serde_json::Value) -> Result<Value<'a>> {
    Ok(match v {
        serde_json::Value::Null => ctx.null(),
        serde_json::Value::Bool(b) => ctx.bool(*b),
        serde_json::Value::Number(n) => match n.as_i64() {
            // Keep integers integral: a JS count arriving in CFML as 3.0
            // formats as "3.0" and compares badly against 3.
            Some(i) => ctx.int(i),
            None => ctx.double(n.as_f64().unwrap_or(0.0)),
        },
        serde_json::Value::String(s) => ctx.string(s),
        serde_json::Value::Array(items) => {
            let arr = ctx.array_with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                arr.set(i, json_to_cfml(ctx, item)?)?;
            }
            arr
        }
        serde_json::Value::Object(map) => {
            let s = ctx.strukt();
            for (k, val) in map {
                s.put(k, json_to_cfml(ctx, val)?)?;
            }
            s
        }
    })
}
