use anyhow::Result;

use crate::parser::{Config, ExtensionSpec, FunctionSpec, TriggerSpec};
use super::Backend;

pub struct JsonBackend;

impl Backend for JsonBackend {
    fn name(&self) -> &'static str { "json" }
    fn file_extension(&self) -> &'static str { "json" }
    fn generate(&self, cfg: &Config) -> Result<String> {
        fn esc(s: &str) -> String {
            let mut out = String::with_capacity(s.len() + 4);
            for c in s.chars() {
                match c {
                    '\\' => out.push_str("\\\\"),
                    '"' => out.push_str("\\\""),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if c < ' ' => {
                        use std::fmt::Write as _;
                        let _ = write!(out, "\\u{:04x}", c as u32);
                    }
                    _ => out.push(c),
                }
            }
            out
        }
        fn q(s: &str) -> String { format!("\"{}\"", esc(s)) }

        fn render_extensions(items: &[ExtensionSpec]) -> String {
            let mut s = String::from("[");
            for (i, e) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push('{');
                s.push_str(&format!(
                    "\"name\":{},\"if_not_exists\":{},\"schema\":{},\"version\":{}",
                    q(&e.name),
                    e.if_not_exists,
                    match &e.schema { Some(v) => q(v), None => "null".into() },
                    match &e.version { Some(v) => q(v), None => "null".into() },
                ));
                s.push('}');
            }
            s.push(']');
            s
        }

        fn render_functions(items: &[FunctionSpec]) -> String {
            let mut s = String::from("[");
            for (i, f) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push('{');
                s.push_str(&format!(
                    "\"name\":{},\"schema\":{},\"language\":{},\"returns\":{},\"replace\":{},\"security_definer\":{},\"body\":{}",
                    q(&f.name),
                    match &f.schema { Some(v) => q(v), None => "null".into() },
                    q(&f.language),
                    q(&f.returns),
                    f.replace,
                    f.security_definer,
                    q(&f.body),
                ));
                s.push('}');
            }
            s.push(']');
            s
        }

        fn render_string_array(items: &[String]) -> String {
            let mut s = String::from("[");
            for (i, v) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push_str(&q(v));
            }
            s.push(']');
            s
        }

        fn render_triggers(items: &[TriggerSpec]) -> String {
            let mut s = String::from("[");
            for (i, t) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push('{');
                s.push_str(&format!(
                    "\"name\":{},\"schema\":{},\"table\":{},\"timing\":{},\"events\":{},\"level\":{},\"function\":{},\"function_schema\":{},\"when\":{}",
                    q(&t.name),
                    match &t.schema { Some(v) => q(v), None => "null".into() },
                    q(&t.table),
                    q(&t.timing),
                    render_string_array(&t.events),
                    q(&t.level),
                    q(&t.function),
                    match &t.function_schema { Some(v) => q(v), None => "null".into() },
                    match &t.when { Some(v) => q(v), None => "null".into() },
                ));
                s.push('}');
            }
            s.push(']');
            s
        }

        let mut out = String::new();
        out.push('{');
        out.push_str(&format!("\"backend\":\"{}\"", self.name()));
        out.push_str(",\"functions\":");
        out.push_str(&render_functions(&cfg.functions));
        out.push_str(",\"triggers\":");
        out.push_str(&render_triggers(&cfg.triggers));
        out.push_str(",\"extensions\":");
        out.push_str(&render_extensions(&cfg.extensions));
        out.push('}');
        Ok(out)
    }
}
