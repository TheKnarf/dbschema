use std::fmt;

pub fn ident(s: &str) -> String {
    let escaped = s.replace('"', "\"");
    format!("\"{}\"", escaped)
}

pub fn literal(s: &str) -> String {
    let escaped = s.replace("'", "''");
    format!("'{}'", escaped)
}

#[derive(Debug, Clone)]
pub struct Role {
    pub name: String,
    pub login: bool,
}

impl From<&crate::ir::RoleSpec> for Role {
    fn from(r: &crate::ir::RoleSpec) -> Self {
        Self {
            name: r.alt_name.clone().unwrap_or_else(|| r.name.clone()),
            login: r.login,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let login = if self.login { " LOGIN" } else { "" };
        write!(
            f,
            "DO $$\nBEGIN\n  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = {name_lit}) THEN\n    CREATE ROLE {name_ident}{login};\n  END IF;\nEND$$;",
            name_lit = literal(&self.name),
            name_ident = ident(&self.name),
            login = login,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Extension {
    pub name: String,
    pub if_not_exists: bool,
    pub schema: Option<String>,
    pub version: Option<String>,
}

impl From<&crate::ir::ExtensionSpec> for Extension {
    fn from(s: &crate::ir::ExtensionSpec) -> Self {
        Self {
            name: s.alt_name.clone().unwrap_or_else(|| s.name.clone()),
            if_not_exists: s.if_not_exists,
            schema: s.schema.clone(),
            version: s.version.clone(),
        }
    }
}

impl fmt::Display for Extension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CREATE EXTENSION ")?;
        if self.if_not_exists {
            write!(f, "IF NOT EXISTS ")?;
        }
        write!(f, "{}", ident(&self.name))?;
        let mut with_parts = Vec::new();
        if let Some(schema) = &self.schema {
            with_parts.push(format!("SCHEMA {}", ident(schema)));
        }
        if let Some(version) = &self.version {
            with_parts.push(format!("VERSION {}", literal(version)));
        }
        if !with_parts.is_empty() {
            write!(f, " WITH {}", with_parts.join(" "))?;
        }
        write!(f, ";")
    }
}

#[derive(Debug, Clone)]
pub struct Schema {
    pub name: String,
    pub if_not_exists: bool,
    pub authorization: Option<String>,
}

impl From<&crate::ir::SchemaSpec> for Schema {
    fn from(s: &crate::ir::SchemaSpec) -> Self {
        Self {
            name: s.alt_name.clone().unwrap_or_else(|| s.name.clone()),
            if_not_exists: s.if_not_exists,
            authorization: s.authorization.clone(),
        }
    }
}

impl fmt::Display for Schema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.if_not_exists {
            write!(f, "CREATE SCHEMA IF NOT EXISTS {}", ident(&self.name))?;
        } else {
            write!(f, "CREATE SCHEMA {}", ident(&self.name))?;
        }
        if let Some(auth) = &self.authorization {
            write!(f, " AUTHORIZATION {}", ident(auth))?;
        }
        write!(f, ";")
    }
}

#[derive(Debug, Clone)]
pub struct Sequence {
    pub schema: String,
    pub name: String,
    pub if_not_exists: bool,
    pub r#as: Option<String>,
    pub increment: Option<i64>,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub start: Option<i64>,
    pub cache: Option<i64>,
    pub cycle: bool,
    pub owned_by: Option<String>,
}

impl From<&crate::ir::SequenceSpec> for Sequence {
    fn from(s: &crate::ir::SequenceSpec) -> Self {
        Self {
            schema: s.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: s.alt_name.clone().unwrap_or_else(|| s.name.clone()),
            if_not_exists: s.if_not_exists,
            r#as: s.r#as.clone(),
            increment: s.increment,
            min_value: s.min_value,
            max_value: s.max_value,
            start: s.start,
            cache: s.cache,
            cycle: s.cycle,
            owned_by: s.owned_by.clone(),
        }
    }
}

impl fmt::Display for Sequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CREATE SEQUENCE")?;
        if self.if_not_exists {
            write!(f, " IF NOT EXISTS")?;
        }
        write!(f, " {}.{}", ident(&self.schema), ident(&self.name))?;
        if let Some(t) = &self.r#as {
            write!(f, " AS {}", t)?;
        }
        if let Some(i) = self.increment {
            write!(f, " INCREMENT BY {}", i)?;
        }
        if let Some(min) = self.min_value {
            write!(f, " MINVALUE {}", min)?;
        }
        if let Some(max) = self.max_value {
            write!(f, " MAXVALUE {}", max)?;
        }
        if let Some(start) = self.start {
            write!(f, " START WITH {}", start)?;
        }
        if let Some(cache) = self.cache {
            write!(f, " CACHE {}", cache)?;
        }
        if self.cycle {
            write!(f, " CYCLE")?;
        }
        if let Some(ob) = &self.owned_by {
            if ob.eq_ignore_ascii_case("NONE") {
                write!(f, " OWNED BY NONE")?;
            } else {
                let parts: Vec<&str> = ob.split('.').collect();
                match parts.as_slice() {
                    [table, column] => write!(f, " OWNED BY {}.{}", ident(table), ident(column))?,
                    [schema, table, column] => write!(
                        f,
                        " OWNED BY {}.{}.{}",
                        ident(schema),
                        ident(table),
                        ident(column)
                    )?,
                    _ => write!(f, " OWNED BY {}", ob)?,
                }
            }
        }
        write!(f, ";")
    }
}

#[derive(Debug, Clone)]
pub struct Domain {
    pub schema: String,
    pub name: String,
    pub r#type: String,
    pub not_null: bool,
    pub default: Option<String>,
    pub constraint: Option<String>,
    pub check: Option<String>,
}

impl From<&crate::ir::DomainSpec> for Domain {
    fn from(d: &crate::ir::DomainSpec) -> Self {
        Self {
            schema: d.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: d.alt_name.clone().unwrap_or_else(|| d.name.clone()),
            r#type: d.r#type.clone(),
            not_null: d.not_null,
            default: d.default.clone(),
            constraint: d.constraint.clone(),
            check: d.check.clone(),
        }
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_type t\n    JOIN pg_namespace n ON n.oid = t.typnamespace\n  WHERE t.typname = {name_lit}\n      AND n.nspname = {schema_lit}\n  ) THEN\n    CREATE DOMAIN {schema_ident}.{name_ident} AS {ty}",
            name_lit = literal(&self.name),
            schema_lit = literal(&self.schema),
            schema_ident = ident(&self.schema),
            name_ident = ident(&self.name),
            ty = self.r#type,
        )?;
        if let Some(def) = &self.default {
            write!(f, " DEFAULT {}", def)?;
        }
        if self.not_null {
            write!(f, " NOT NULL")?;
        }
        if let Some(check) = &self.check {
            if let Some(cons) = &self.constraint {
                write!(f, " CONSTRAINT {} CHECK ({})", ident(cons), check)?;
            } else {
                write!(f, " CHECK ({})", check)?;
            }
        }
        write!(f, ";\n  END IF;\nEND$$;")
    }
}

#[derive(Debug, Clone)]
pub struct CompositeField {
    pub name: String,
    pub r#type: String,
}

impl From<&crate::ir::CompositeTypeFieldSpec> for CompositeField {
    fn from(f: &crate::ir::CompositeTypeFieldSpec) -> Self {
        Self {
            name: f.name.clone(),
            r#type: f.r#type.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompositeType {
    pub schema: String,
    pub name: String,
    pub fields: Vec<CompositeField>,
}

impl From<&crate::ir::CompositeTypeSpec> for CompositeType {
    fn from(t: &crate::ir::CompositeTypeSpec) -> Self {
        Self {
            schema: t.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: t.alt_name.clone().unwrap_or_else(|| t.name.clone()),
            fields: t.fields.iter().map(Into::into).collect(),
        }
    }
}

impl fmt::Display for CompositeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fields = self
            .fields
            .iter()
            .map(|c| format!("{} {}", ident(&c.name), c.r#type))
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_type t\n    JOIN pg_namespace n ON n.oid = t.typnamespace\n  WHERE t.typname = {name_lit}\n      AND n.nspname = {schema_lit}\n  ) THEN\n    CREATE TYPE {schema_ident}.{name_ident} AS ({fields});\n  END IF;\nEND$$;",
            name_lit = literal(&self.name),
            schema_lit = literal(&self.schema),
            schema_ident = ident(&self.schema),
            name_ident = ident(&self.name),
            fields = fields,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub schema: String,
    pub name: String,
    pub values: Vec<String>,
}

impl From<&crate::ir::EnumSpec> for Enum {
    fn from(e: &crate::ir::EnumSpec) -> Self {
        Self {
            schema: e.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: e.alt_name.clone().unwrap_or_else(|| e.name.clone()),
            values: e.values.clone(),
        }
    }
}

impl fmt::Display for Enum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let values = self
            .values
            .iter()
            .map(|v| literal(v))
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_type t\n    JOIN pg_namespace n ON n.oid = t.typnamespace\n    WHERE t.typname = {name_lit}\n      AND n.nspname = {schema_lit}\n  ) THEN\n    CREATE TYPE {schema_ident}.{name_ident} AS ENUM ({values});\n  END IF;\nEND$$;",
            name_lit = literal(&self.name),
            schema_lit = literal(&self.schema),
            schema_ident = ident(&self.schema),
            name_ident = ident(&self.name),
            values = values,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub schema: String,
    pub name: String,
    pub language: String,
    pub returns: String,
    pub replace: bool,
    pub security_definer: bool,
    pub body: String,
}

impl From<&crate::ir::FunctionSpec> for Function {
    fn from(f: &crate::ir::FunctionSpec) -> Self {
        Self {
            schema: f.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: f.alt_name.clone().unwrap_or_else(|| f.name.clone()),
            language: f.language.clone(),
            returns: f.returns.clone(),
            replace: f.replace,
            security_definer: f.security_definer,
            body: f.body.clone(),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let definer = if self.security_definer {
            " SECURITY DEFINER"
        } else {
            ""
        };
        let or_replace = if self.replace { "OR REPLACE " } else { "" };
        write!(
            f,
            "CREATE {or_replace}FUNCTION {schema}.{name}() RETURNS {returns} LANGUAGE {lang}{definer} AS $$\n{body}\n$$;",
            or_replace = or_replace,
            schema = ident(&self.schema),
            name = ident(&self.name),
            returns = self.returns,
            lang = self.language.to_lowercase(),
            definer = definer,
            body = self.body,
        )
    }
}

#[derive(Debug, Clone)]
pub struct View {
    pub schema: String,
    pub name: String,
    pub sql: String,
    pub replace: bool,
}

impl From<&crate::ir::ViewSpec> for View {
    fn from(v: &crate::ir::ViewSpec) -> Self {
        Self {
            schema: v.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: v.alt_name.clone().unwrap_or_else(|| v.name.clone()),
            sql: v.sql.clone(),
            replace: v.replace,
        }
    }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let or_replace = if self.replace { "OR REPLACE " } else { "" };
        write!(
            f,
            "CREATE {or_replace}VIEW {schema}.{name} AS\n{body};",
            or_replace = or_replace,
            schema = ident(&self.schema),
            name = ident(&self.name),
            body = self.sql,
        )
    }
}

#[derive(Debug, Clone)]
pub struct MaterializedView {
    pub schema: String,
    pub name: String,
    pub sql: String,
    pub with_data: bool,
}

impl From<&crate::ir::MaterializedViewSpec> for MaterializedView {
    fn from(m: &crate::ir::MaterializedViewSpec) -> Self {
        Self {
            schema: m.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: m.alt_name.clone().unwrap_or_else(|| m.name.clone()),
            sql: m.sql.clone(),
            with_data: m.with_data,
        }
    }
}

impl fmt::Display for MaterializedView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let with = if self.with_data {
            "WITH DATA"
        } else {
            "WITH NO DATA"
        };
        write!(
            f,
            "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_matviews WHERE schemaname = {schema_lit} AND matviewname = {name_lit}\n  ) THEN\n    CREATE MATERIALIZED VIEW {schema_ident}.{name_ident} AS\n{body}\n    {with};\n  END IF;\nEND$$;",
            schema_lit = literal(&self.schema),
            name_lit = literal(&self.name),
            schema_ident = ident(&self.schema),
            name_ident = ident(&self.name),
            body = self.sql,
            with = with,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub r#type: String,
    pub nullable: bool,
    pub default: Option<String>,
}

impl From<&crate::ir::ColumnSpec> for Column {
    fn from(c: &crate::ir::ColumnSpec) -> Self {
        Self {
            name: c.name.clone(),
            r#type: c.r#type.clone(),
            nullable: c.nullable,
            default: c.default.clone(),
        }
    }
}

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", ident(&self.name), self.r#type)?;
        if !self.nullable {
            write!(f, " NOT NULL")?;
        }
        if let Some(d) = &self.default {
            write!(f, " DEFAULT {}", d)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PrimaryKey {
    pub name: Option<String>,
    pub columns: Vec<String>,
}

impl From<&crate::ir::PrimaryKeySpec> for PrimaryKey {
    fn from(pk: &crate::ir::PrimaryKeySpec) -> Self {
        Self {
            name: pk.name.clone(),
            columns: pk.columns.clone(),
        }
    }
}

impl fmt::Display for PrimaryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cols = self
            .columns
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        match &self.name {
            Some(n) => write!(f, "CONSTRAINT {} PRIMARY KEY ({})", ident(n), cols),
            None => write!(f, "PRIMARY KEY ({})", cols),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ForeignKey {
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub ref_schema: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_delete: Option<String>,
    pub on_update: Option<String>,
}

impl From<&crate::ir::ForeignKeySpec> for ForeignKey {
    fn from(fk: &crate::ir::ForeignKeySpec) -> Self {
        Self {
            name: fk.name.clone(),
            columns: fk.columns.clone(),
            ref_schema: fk
                .ref_schema
                .clone()
                .unwrap_or_else(|| "public".to_string()),
            ref_table: fk.ref_table.clone(),
            ref_columns: fk.ref_columns.clone(),
            on_delete: fk.on_delete.clone(),
            on_update: fk.on_update.clone(),
        }
    }
}

impl fmt::Display for ForeignKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cols = self
            .columns
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let ref_cols = self
            .ref_columns
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        if let Some(n) = &self.name {
            write!(f, "CONSTRAINT {} ", ident(n))?;
        }
        write!(
            f,
            "FOREIGN KEY ({cols}) REFERENCES {rschema}.{rtable} ({rcols})",
            cols = cols,
            rschema = ident(&self.ref_schema),
            rtable = ident(&self.ref_table),
            rcols = ref_cols,
        )?;
        if let Some(od) = &self.on_delete {
            write!(f, " ON DELETE {}", od)?;
        }
        if let Some(ou) = &self.on_update {
            write!(f, " ON UPDATE {}", ou)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Table {
    pub schema: String,
    pub name: String,
    pub if_not_exists: bool,
    pub columns: Vec<Column>,
    pub primary_key: Option<PrimaryKey>,
    pub foreign_keys: Vec<ForeignKey>,
}

impl From<&crate::ir::TableSpec> for Table {
    fn from(t: &crate::ir::TableSpec) -> Self {
        Self {
            schema: t.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: t.table_name.clone().unwrap_or_else(|| t.name.clone()),
            if_not_exists: t.if_not_exists,
            columns: t.columns.iter().map(Column::from).collect(),
            primary_key: t.primary_key.as_ref().map(PrimaryKey::from),
            foreign_keys: t.foreign_keys.iter().map(ForeignKey::from).collect(),
        }
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut lines: Vec<String> = self.columns.iter().map(|c| format!("{}", c)).collect();
        if let Some(pk) = &self.primary_key {
            lines.push(format!("{}", pk));
        }
        for fk in &self.foreign_keys {
            lines.push(format!("{}", fk));
        }
        let body = lines
            .into_iter()
            .map(|l| format!("  {}", l))
            .collect::<Vec<_>>()
            .join(",\n");
        let ine = if self.if_not_exists {
            " IF NOT EXISTS"
        } else {
            ""
        };
        write!(
            f,
            "CREATE TABLE{ine} {schema}.{name} (\n{body}\n);",
            ine = ine,
            schema = ident(&self.schema),
            name = ident(&self.name),
            body = body,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Index {
    pub table_schema: String,
    pub table_name: String,
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub unique: bool,
}

impl Index {
    pub fn from_specs(table: &crate::ir::TableSpec, idx: &crate::ir::IndexSpec) -> Self {
        Self {
            table_schema: table.schema.clone().unwrap_or_else(|| "public".to_string()),
            table_name: table
                .table_name
                .clone()
                .unwrap_or_else(|| table.name.clone()),
            name: idx.name.clone(),
            columns: idx.columns.clone(),
            unique: idx.unique,
        }
    }
}

impl fmt::Display for Index {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cols = self
            .columns
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let unique = if self.unique { "UNIQUE " } else { "" };
        let name = match &self.name {
            Some(n) => ident(n),
            None => {
                let mut n = format!(
                    "{}_{}_{}",
                    self.table_name,
                    self.columns.join("_"),
                    if self.unique { "uniq" } else { "idx" }
                );
                n = n.replace('.', "_");
                ident(&n)
            }
        };
        write!(
            f,
            "CREATE {unique}INDEX IF NOT EXISTS {name} ON {schema}.{table} ({cols});",
            unique = unique,
            name = name,
            schema = ident(&self.table_schema),
            table = ident(&self.table_name),
            cols = cols,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Trigger {
    pub schema: String,
    pub table: String,
    pub name: String,
    pub timing: String,
    pub events: Vec<String>,
    pub level: String,
    pub function: String,
    pub function_schema: String,
    pub when: Option<String>,
}

impl From<&crate::ir::TriggerSpec> for Trigger {
    fn from(t: &crate::ir::TriggerSpec) -> Self {
        Self {
            schema: t.schema.clone().unwrap_or_else(|| "public".to_string()),
            table: t.table.clone(),
            name: t.alt_name.clone().unwrap_or_else(|| t.name.clone()),
            timing: t.timing.clone(),
            events: t.events.clone(),
            level: t.level.clone(),
            function: t.function.clone(),
            function_schema: t
                .function_schema
                .clone()
                .unwrap_or_else(|| t.schema.clone().unwrap_or_else(|| "public".to_string())),
            when: t.when.clone(),
        }
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let events = self
            .events
            .iter()
            .map(|e| e.to_uppercase())
            .collect::<Vec<_>>()
            .join(" OR ");
        let when = self
            .when
            .as_ref()
            .map(|w| format!("\n    WHEN ({})", w))
            .unwrap_or_default();
        write!(
            f,
            "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_trigger tg\n    JOIN pg_class c ON c.oid = tg.tgrelid\n    JOIN pg_namespace n ON n.oid = c.relnamespace\n    WHERE tg.tgname = {tgname}\n      AND n.nspname = {schema_lit}\n      AND c.relname = {table_lit}\n  ) THEN\n    CREATE TRIGGER {tg}\n    {timing} {events} ON {schema_ident}.{table_ident}\n    FOR EACH {for_each}{when}\n    EXECUTE FUNCTION {fn_schema_ident}.{fn_name}();\n  END IF;\nEND$$;",
            tgname = literal(&self.name),
            schema_lit = literal(&self.schema),
            table_lit = literal(&self.table),
            tg = ident(&self.name),
            timing = self.timing.to_uppercase(),
            events = events,
            for_each = self.level.to_uppercase(),
            when = when,
            schema_ident = ident(&self.schema),
            table_ident = ident(&self.table),
            fn_schema_ident = ident(&self.function_schema),
            fn_name = ident(&self.function),
        )
    }
}

#[derive(Debug, Clone)]
pub struct Policy {
    pub schema: String,
    pub table: String,
    pub name: String,
    pub command: String,
    pub r#as: Option<String>,
    pub roles: Vec<String>,
    pub using: Option<String>,
    pub check: Option<String>,
}

impl From<&crate::ir::PolicySpec> for Policy {
    fn from(p: &crate::ir::PolicySpec) -> Self {
        Self {
            schema: p.schema.clone().unwrap_or_else(|| "public".to_string()),
            table: p.table.clone(),
            name: p.alt_name.clone().unwrap_or_else(|| p.name.clone()),
            command: p.command.clone(),
            r#as: p.r#as.clone(),
            roles: p.roles.clone(),
            using: p.using.clone(),
            check: p.check.clone(),
        }
    }
}

impl fmt::Display for Policy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cmd = self.command.to_uppercase();
        let as_clause = match self.r#as.as_ref().map(|s| s.to_uppercase()) {
            Some(ref k) if k == "PERMISSIVE" || k == "RESTRICTIVE" => format!(" AS {}", k),
            _ => String::new(),
        };
        let for_clause = if cmd == "ALL" {
            String::new()
        } else {
            format!(" FOR {}", cmd)
        };
        let to_clause = if self.roles.is_empty() {
            String::new()
        } else {
            let roles = self
                .roles
                .iter()
                .map(|r| ident(r))
                .collect::<Vec<_>>()
                .join(", ");
            format!(" TO {}", roles)
        };
        let using_clause = match &self.using {
            Some(u) => format!("\n    USING ({})", u),
            None => String::new(),
        };
        let check_clause = match &self.check {
            Some(c) => format!("\n    WITH CHECK ({})", c),
            None => String::new(),
        };
        write!(
            f,
            "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_policies\n    WHERE policyname = {pname}\n      AND schemaname = {schema_lit}\n      AND tablename = {table_lit}\n  ) THEN\n    CREATE POLICY {pname_ident} ON {schema_ident}.{table_ident}{as_clause}{for_clause}{to_clause}{using}{check};\n  END IF;\nEND$$;",
            pname = literal(&self.name),
            schema_lit = literal(&self.schema),
            table_lit = literal(&self.table),
            pname_ident = ident(&self.name),
            schema_ident = ident(&self.schema),
            table_ident = ident(&self.table),
            as_clause = as_clause,
            for_clause = for_clause,
            to_clause = to_clause,
            using = using_clause,
            check = check_clause,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Grant {
    pub role: String,
    pub privileges: Vec<String>,
    pub schema: Option<String>,
    pub table: Option<String>,
    pub function: Option<String>,
}

impl From<&crate::ir::GrantSpec> for Grant {
    fn from(g: &crate::ir::GrantSpec) -> Self {
        Self {
            role: g.role.clone(),
            privileges: g.privileges.clone(),
            schema: g.schema.clone(),
            table: g.table.clone(),
            function: g.function.clone(),
        }
    }
}

impl fmt::Display for Grant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let privs = self
            .privileges
            .iter()
            .map(|p| p.to_uppercase())
            .collect::<Vec<_>>()
            .join(", ");
        let role = ident(&self.role);
        if let Some(table) = &self.table {
            let schema = self.schema.clone().unwrap_or_else(|| "public".to_string());
            write!(
                f,
                "GRANT {privs} ON TABLE {schema_ident}.{table_ident} TO {role};",
                privs = privs,
                schema_ident = ident(&schema),
                table_ident = ident(table),
                role = role,
            )
        } else if let Some(function) = &self.function {
            let schema = self.schema.clone().unwrap_or_else(|| "public".to_string());
            write!(
                f,
                "GRANT {privs} ON FUNCTION {schema_ident}.{fn_ident}() TO {role};",
                privs = privs,
                schema_ident = ident(&schema),
                fn_ident = ident(function),
                role = role,
            )
        } else if let Some(schema) = &self.schema {
            write!(
                f,
                "GRANT {privs} ON SCHEMA {schema_ident} TO {role};",
                privs = privs,
                schema_ident = ident(schema),
                role = role,
            )
        } else {
            Ok(())
        }
    }
}
