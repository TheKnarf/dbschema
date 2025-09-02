use super::Backend;
use crate::{ir::*, postgres as pg};
use anyhow::Result;

pub struct PostgresBackend;

impl Backend for PostgresBackend {
    fn name(&self) -> &'static str {
        "postgres"
    }
    fn file_extension(&self) -> &'static str {
        "sql"
    }
    fn generate(&self, cfg: &Config, _strict: bool) -> Result<String> {
        to_sql(cfg)
    }
}

fn to_sql(cfg: &Config) -> Result<String> {
    let mut out = String::new();

    for r in &cfg.roles {
        out.push_str(&format!("{}\n\n", pg::Role::from(r)));
    }

    for s in &cfg.schemas {
        out.push_str(&format!("{}\n\n", pg::Schema::from(s)));
    }

    for e in &cfg.extensions {
        out.push_str(&format!("{}\n\n", pg::Extension::from(e)));
    }

    for s in &cfg.sequences {
        out.push_str(&format!("{}\n\n", pg::Sequence::from(s)));
    }

    for e in &cfg.enums {
        out.push_str(&format!("{}\n\n", pg::Enum::from(e)));
    }

    for t in &cfg.tables {
        out.push_str(&format!("{}\n\n", pg::Table::from(t)));
        for idx in &t.indexes {
            out.push_str(&format!("{}\n\n", pg::Index::from_specs(t, idx)));
        }
    }

    for p in &cfg.policies {
        out.push_str(&format!("{}\n\n", pg::Policy::from(p)));
    }

    for f in &cfg.functions {
        out.push_str(&format!("{}\n\n", pg::Function::from(f)));
    }

    for v in &cfg.views {
        out.push_str(&format!("{}\n\n", pg::View::from(v)));
    }

    for mv in &cfg.materialized {
        out.push_str(&format!("{}\n\n", pg::MaterializedView::from(mv)));
    }

    for t in &cfg.triggers {
        out.push_str(&format!("{}\n\n", pg::Trigger::from(t)));
    }

    for g in &cfg.grants {
        out.push_str(&format!("{}\n\n", pg::Grant::from(g)));
    }

    Ok(out)
}
