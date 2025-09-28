use super::{generate_header_comment, Backend, CommentStyle};
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
    let header = generate_header_comment("PostgreSQL", CommentStyle::Sql);
    let mut out = header;

    for r in &cfg.roles {
        out.push_str(&format!("{}\n\n", pg::Role::from(r)));
        if let Some(comment) = &r.comment {
            let name = r.alt_name.clone().unwrap_or_else(|| r.name.clone());
            out.push_str(&format!(
                "COMMENT ON ROLE {} IS {};\n\n",
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for t in &cfg.tablespaces {
        out.push_str(&format!("{}\n\n", pg::Tablespace::from(t)));
        if let Some(comment) = &t.comment {
            let name = t.alt_name.clone().unwrap_or_else(|| t.name.clone());
            out.push_str(&format!(
                "COMMENT ON TABLESPACE {} IS {};\n\n",
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for s in &cfg.schemas {
        out.push_str(&format!("{}\n\n", pg::Schema::from(s)));
        if let Some(comment) = &s.comment {
            let name = s.alt_name.clone().unwrap_or_else(|| s.name.clone());
            out.push_str(&format!(
                "COMMENT ON SCHEMA {} IS {};\n\n",
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for e in &cfg.extensions {
        out.push_str(&format!("{}\n\n", pg::Extension::from(e)));
        if let Some(comment) = &e.comment {
            let name = e.alt_name.clone().unwrap_or_else(|| e.name.clone());
            out.push_str(&format!(
                "COMMENT ON EXTENSION {} IS {};\n\n",
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for c in &cfg.collations {
        out.push_str(&format!("{}\n\n", pg::Collation::from(c)));
        if let Some(comment) = &c.comment {
            let schema = c.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = c.alt_name.clone().unwrap_or_else(|| c.name.clone());
            out.push_str(&format!(
                "COMMENT ON COLLATION {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for d in &cfg.text_search_dictionaries {
        out.push_str(&format!("{}\n\n", pg::TextSearchDictionary::from(d)));
        if let Some(comment) = &d.comment {
            let schema = d.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = d.alt_name.clone().unwrap_or_else(|| d.name.clone());
            out.push_str(&format!(
                "COMMENT ON TEXT SEARCH DICTIONARY {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for t in &cfg.text_search_templates {
        out.push_str(&format!("{}\n\n", pg::TextSearchTemplate::from(t)));
        if let Some(comment) = &t.comment {
            let schema = t.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = t.alt_name.clone().unwrap_or_else(|| t.name.clone());
            out.push_str(&format!(
                "COMMENT ON TEXT SEARCH TEMPLATE {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for p in &cfg.text_search_parsers {
        out.push_str(&format!("{}\n\n", pg::TextSearchParser::from(p)));
        if let Some(comment) = &p.comment {
            let schema = p.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = p.alt_name.clone().unwrap_or_else(|| p.name.clone());
            out.push_str(&format!(
                "COMMENT ON TEXT SEARCH PARSER {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for c in &cfg.text_search_configurations {
        out.push_str(&format!("{}\n\n", pg::TextSearchConfiguration::from(c)));
        if let Some(comment) = &c.comment {
            let schema = c.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = c.alt_name.clone().unwrap_or_else(|| c.name.clone());
            out.push_str(&format!(
                "COMMENT ON TEXT SEARCH CONFIGURATION {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for s in &cfg.sequences {
        out.push_str(&format!("{}\n\n", pg::Sequence::from(s)));
        if let Some(comment) = &s.comment {
            let schema = s.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = s.alt_name.clone().unwrap_or_else(|| s.name.clone());
            out.push_str(&format!(
                "COMMENT ON SEQUENCE {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for e in &cfg.enums {
        out.push_str(&format!("{}\n\n", pg::Enum::from(e)));
        if let Some(comment) = &e.comment {
            let schema = e.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = e.alt_name.clone().unwrap_or_else(|| e.name.clone());
            out.push_str(&format!(
                "COMMENT ON TYPE {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for d in &cfg.domains {
        out.push_str(&format!("{}\n\n", pg::Domain::from(d)));
        if let Some(comment) = &d.comment {
            let schema = d.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = d.alt_name.clone().unwrap_or_else(|| d.name.clone());
            out.push_str(&format!(
                "COMMENT ON DOMAIN {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for t in &cfg.types {
        out.push_str(&format!("{}\n\n", pg::CompositeType::from(t)));
        if let Some(comment) = &t.comment {
            let schema = t.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = t.alt_name.clone().unwrap_or_else(|| t.name.clone());
            out.push_str(&format!(
                "COMMENT ON TYPE {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for t in &cfg.tables {
        out.push_str(&format!("{}\n\n", pg::Table::from(t)));
        let schema = t.schema.clone().unwrap_or_else(|| "public".to_string());
        let table_name = t.alt_name.clone().unwrap_or_else(|| t.name.clone());
        for idx in &t.indexes {
            out.push_str(&format!("{}\n\n", pg::Index::from_specs(t, idx)));
        }
        for chk in &t.checks {
            let constraint = chk
                .name
                .as_ref()
                .map(|n| format!("CONSTRAINT {} ", pg::ident(n)))
                .unwrap_or_default();
            out.push_str(&format!(
                "ALTER TABLE {}.{} ADD {constraint}CHECK ({});\n\n",
                pg::ident(&schema),
                pg::ident(&table_name),
                chk.expression,
                constraint = constraint,
            ));
        }
        if let Some(comment) = &t.comment {
            out.push_str(&format!(
                "COMMENT ON TABLE {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&table_name),
                pg::literal(comment)
            ));
        }
        for c in &t.columns {
            if let Some(comment) = &c.comment {
                out.push_str(&format!(
                    "COMMENT ON COLUMN {}.{}.{} IS {};\n\n",
                    pg::ident(&schema),
                    pg::ident(&table_name),
                    pg::ident(&c.name),
                    pg::literal(comment)
                ));
            }
        }
    }

    // Apply sequence ownership after tables exist to avoid ordering issues
    for s in &cfg.sequences {
        if let Some(ob) = &s.owned_by {
            let schema = s.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = s.alt_name.clone().unwrap_or_else(|| s.name.clone());
            let target = if ob.eq_ignore_ascii_case("NONE") {
                "NONE".to_string()
            } else {
                let parts: Vec<&str> = ob.split('.').collect();
                match parts.as_slice() {
                    [table, column] => format!("{}.{}", pg::ident(table), pg::ident(column)),
                    [schema, table, column] => format!(
                        "{}.{}.{}",
                        pg::ident(schema),
                        pg::ident(table),
                        pg::ident(column)
                    ),
                    _ => ob.to_string(),
                }
            };
            out.push_str(&format!(
                "ALTER SEQUENCE {}.{} OWNED BY {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                target
            ));
        }
    }

    for idx in &cfg.indexes {
        out.push_str(&format!("{}\n\n", pg::Index::from_standalone(idx)));
    }

    for s in &cfg.statistics {
        out.push_str(&format!("{}\n\n", pg::Statistics::from(s)));
        if let Some(comment) = &s.comment {
            let schema = s.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = s.alt_name.clone().unwrap_or_else(|| s.name.clone());
            out.push_str(&format!(
                "COMMENT ON STATISTICS {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for p in &cfg.policies {
        out.push_str(&format!("{}\n\n", pg::Policy::from(p)));
        if let Some(comment) = &p.comment {
            let schema = p.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = p.alt_name.clone().unwrap_or_else(|| p.name.clone());
            out.push_str(&format!(
                "COMMENT ON POLICY {} ON {}.{} IS {};\n\n",
                pg::ident(&name),
                pg::ident(&schema),
                pg::ident(&p.table),
                pg::literal(comment)
            ));
        }
    }

    for f in &cfg.functions {
        out.push_str(&format!("{}\n\n", pg::Function::from(f)));
        if let Some(comment) = &f.comment {
            let schema = f.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = f.alt_name.clone().unwrap_or_else(|| f.name.clone());
            out.push_str(&format!(
                "COMMENT ON FUNCTION {}.{}() IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for p in &cfg.procedures {
        out.push_str(&format!("{}\n\n", pg::Procedure::from(p)));
        if let Some(comment) = &p.comment {
            let schema = p.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = p.alt_name.clone().unwrap_or_else(|| p.name.clone());
            out.push_str(&format!(
                "COMMENT ON PROCEDURE {}.{}() IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for a in &cfg.aggregates {
        out.push_str(&format!("{}\n\n", pg::Aggregate::from(a)));
        if let Some(comment) = &a.comment {
            let schema = a.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = a.alt_name.clone().unwrap_or_else(|| a.name.clone());
            let inputs = a.inputs.join(", ");
            out.push_str(&format!(
                "COMMENT ON AGGREGATE {}.{}({}) IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                inputs,
                pg::literal(comment)
            ));
        }
    }

    for o in &cfg.operators {
        out.push_str(&format!("{}\n\n", pg::Operator::from(o)));
        if let Some(comment) = &o.comment {
            let schema = o.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = o.alt_name.clone().unwrap_or_else(|| o.name.clone());
            let left = o.left.clone().unwrap_or_else(|| "NONE".to_string());
            let right = o.right.clone().unwrap_or_else(|| "NONE".to_string());
            out.push_str(&format!(
                "COMMENT ON OPERATOR OPERATOR({}.{}) ({}, {}) IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                left,
                right,
                pg::literal(comment)
            ));
        }
    }

    for v in &cfg.views {
        out.push_str(&format!("{}\n\n", pg::View::from(v)));
        if let Some(comment) = &v.comment {
            let schema = v.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = v.alt_name.clone().unwrap_or_else(|| v.name.clone());
            out.push_str(&format!(
                "COMMENT ON VIEW {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for mv in &cfg.materialized {
        out.push_str(&format!("{}\n\n", pg::MaterializedView::from(mv)));
        if let Some(comment) = &mv.comment {
            let schema = mv.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = mv.alt_name.clone().unwrap_or_else(|| mv.name.clone());
            out.push_str(&format!(
                "COMMENT ON MATERIALIZED VIEW {}.{} IS {};\n\n",
                pg::ident(&schema),
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for e in &cfg.event_triggers {
        out.push_str(&format!("{}\n\n", pg::EventTrigger::from(e)));
        if let Some(comment) = &e.comment {
            let name = e.alt_name.clone().unwrap_or_else(|| e.name.clone());
            out.push_str(&format!(
                "COMMENT ON EVENT TRIGGER {} IS {};\n\n",
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for t in &cfg.triggers {
        out.push_str(&format!("{}\n\n", pg::Trigger::from(t)));
        if let Some(comment) = &t.comment {
            let schema = t.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = t.alt_name.clone().unwrap_or_else(|| t.name.clone());
            out.push_str(&format!(
                "COMMENT ON TRIGGER {} ON {}.{} IS {};\n\n",
                pg::ident(&name),
                pg::ident(&schema),
                pg::ident(&t.table),
                pg::literal(comment)
            ));
        }
    }

    for r in &cfg.rules {
        out.push_str(&format!("{}\n\n", pg::Rule::from(r)));
        if let Some(comment) = &r.comment {
            let schema = r.schema.clone().unwrap_or_else(|| "public".to_string());
            let name = r.alt_name.clone().unwrap_or_else(|| r.name.clone());
            out.push_str(&format!(
                "COMMENT ON RULE {} ON {}.{} IS {};\n\n",
                pg::ident(&name),
                pg::ident(&schema),
                pg::ident(&r.table),
                pg::literal(comment)
            ));
        }
    }

    for g in &cfg.grants {
        out.push_str(&format!("{}\n\n", pg::Grant::from(g)));
    }

    for p in &cfg.publications {
        out.push_str(&format!("{}\n\n", pg::Publication::from(p)));
        if let Some(comment) = &p.comment {
            let name = p.alt_name.clone().unwrap_or_else(|| p.name.clone());
            out.push_str(&format!(
                "COMMENT ON PUBLICATION {} IS {};\n\n",
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    for s in &cfg.subscriptions {
        out.push_str(&format!("{}\n\n", pg::Subscription::from(s)));
        if let Some(comment) = &s.comment {
            let name = s.alt_name.clone().unwrap_or_else(|| s.name.clone());
            out.push_str(&format!(
                "COMMENT ON SUBSCRIPTION {} IS {};\n\n",
                pg::ident(&name),
                pg::literal(comment)
            ));
        }
    }

    Ok(out)
}
