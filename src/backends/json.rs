use anyhow::Result;

use crate::parser::{Config, ExtensionSpec, FunctionSpec, TriggerSpec, TableSpec, ColumnSpec, IndexSpec, ForeignKeySpec, ViewSpec, MaterializedViewSpec, EnumSpec};
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

        fn render_columns(items: &[ColumnSpec]) -> String {
            let mut s = String::from("[");
            for (i, c) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push('{');
                s.push_str(&format!(
                    "\"name\":{},\"type\":{},\"nullable\":{},\"default\":{}",
                    q(&c.name), q(&c.r#type), c.nullable,
                    match &c.default { Some(v) => q(v), None => "null".into() },
                ));
                s.push('}');
            }
            s.push(']');
            s
        }

        fn render_indexes(items: &[IndexSpec]) -> String {
            let mut s = String::from("[");
            for (i, ix) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push('{');
                s.push_str(&format!(
                    "\"name\":{},\"columns\":{},\"unique\":{}",
                    match &ix.name { Some(v) => q(v), None => "null".into() },
                    {
                        let mut t = String::from("[");
                        for (j, c) in ix.columns.iter().enumerate() { if j>0 { t.push(','); } t.push_str(&q(c)); }
                        t.push(']');
                        t
                    },
                    ix.unique,
                ));
                s.push('}');
            }
            s.push(']');
            s
        }

        fn render_fk(items: &[ForeignKeySpec]) -> String {
            let mut s = String::from("[");
            for (i, fk) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push('{');
                s.push_str(&format!(
                    "\"name\":{},\"columns\":{},\"ref_schema\":{},\"ref_table\":{},\"ref_columns\":{},\"on_delete\":{},\"on_update\":{}",
                    match &fk.name { Some(v) => q(v), None => "null".into() },
                    {
                        let mut t = String::from("[");
                        for (j, c) in fk.columns.iter().enumerate() { if j>0 { t.push(','); } t.push_str(&q(c)); }
                        t.push(']');
                        t
                    },
                    match &fk.ref_schema { Some(v) => q(v), None => "null".into() },
                    q(&fk.ref_table),
                    {
                        let mut t = String::from("[");
                        for (j, c) in fk.ref_columns.iter().enumerate() { if j>0 { t.push(','); } t.push_str(&q(c)); }
                        t.push(']');
                        t
                    },
                    match &fk.on_delete { Some(v) => q(v), None => "null".into() },
                    match &fk.on_update { Some(v) => q(v), None => "null".into() },
                ));
                s.push('}');
            }
            s.push(']');
            s
        }

        fn render_tables(items: &[TableSpec]) -> String {
            let mut s = String::from("[");
            for (i, t) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push('{');
                // primary key
                let pk = match &t.primary_key {
                    Some(pk) => {
                        let mut pkjson = String::from("{");
                        pkjson.push_str(&format!(
                            "\"name\":{},\"columns\":[{}]",
                            match &pk.name { Some(v) => q(v), None => "null".into() },
                            pk.columns.iter().map(|c| q(c)).collect::<Vec<_>>().join(",")
                        ));
                        pkjson.push('}');
                        pkjson
                    }
                    None => "null".into(),
                };
                s.push_str(&format!(
                    "\"name\":{},\"schema\":{},\"if_not_exists\":{},\"columns\":{},\"primary_key\":{},\"indexes\":{},\"foreign_keys\":{}",
                    q(&t.name),
                    match &t.schema { Some(v) => q(v), None => "null".into() },
                    t.if_not_exists,
                    render_columns(&t.columns),
                    pk,
                    render_indexes(&t.indexes),
                    render_fk(&t.foreign_keys),
                ));
                s.push('}');
            }
            s.push(']');
            s
        }

        fn render_views(items: &[ViewSpec]) -> String {
            let mut s = String::from("[");
            for (i, v) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push('{');
                s.push_str(&format!(
                    "\"name\":{},\"schema\":{},\"replace\":{},\"sql\":{}",
                    q(&v.name),
                    match &v.schema { Some(v) => q(v), None => "null".into() },
                    v.replace,
                    q(&v.sql),
                ));
                s.push('}');
            }
            s.push(']');
            s
        }

        fn render_materialized(items: &[MaterializedViewSpec]) -> String {
            let mut s = String::from("[");
            for (i, v) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push('{');
                s.push_str(&format!(
                    "\"name\":{},\"schema\":{},\"with_data\":{},\"sql\":{}",
                    q(&v.name),
                    match &v.schema { Some(v) => q(v), None => "null".into() },
                    v.with_data,
                    q(&v.sql),
                ));
                s.push('}');
            }
            s.push(']');
            s
        }

        fn render_enums(items: &[EnumSpec]) -> String {
            let mut s = String::from("[");
            for (i, e) in items.iter().enumerate() {
                if i > 0 { s.push(','); }
                s.push('{');
                s.push_str(&format!(
                    "\"name\":{},\"schema\":{},\"values\":[{}]",
                    q(&e.name),
                    match &e.schema { Some(v) => q(v), None => "null".into() },
                    e.values.iter().map(|v| q(v)).collect::<Vec<_>>().join(",")
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
        out.push_str(",\"tables\":");
        out.push_str(&render_tables(&cfg.tables));
        out.push_str(",\"enums\":");
        out.push_str(&render_enums(&cfg.enums));
        out.push_str(",\"views\":");
        out.push_str(&render_views(&cfg.views));
        out.push_str(",\"materialized\":");
        out.push_str(&render_materialized(&cfg.materialized));
        out.push_str(",\"extensions\":");
        out.push_str(&render_extensions(&cfg.extensions));
        out.push('}');
        Ok(out)
    }
}
