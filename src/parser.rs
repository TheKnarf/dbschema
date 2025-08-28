use anyhow::{bail, Context, Result};
use path_absolutize::Absolutize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::Loader;

pub mod eval {
    use super::*;
    use hcl::template::{Element as TplElement, Template};
    use hcl::{expr::TemplateExpr, Traversal, TraversalOperator, Value};

    pub fn expr_to_string(expr: &hcl::Expression, env: &super::EnvVars) -> Result<String> {
        match expr {
            hcl::Expression::String(s) => Ok(s.clone()),
            hcl::Expression::TemplateExpr(t) => {
                let tpl = Template::from_expr(t as &TemplateExpr)?;
                let mut out = String::new();
                for el in tpl.elements() {
                    match el {
                        TplElement::Literal(s) => out.push_str(s),
                        TplElement::Interpolation(ip) => {
                            let v = expr_to_value(&ip.expr, env)?;
                            out.push_str(&value_to_string(&v)?);
                        }
                        TplElement::Directive(_) => bail!("template directives not supported in this context"),
                    }
                }
                Ok(out)
            }
            hcl::Expression::Traversal(tr) => {
                let v = resolve_traversal_value(tr, env)?;
                value_to_string(&v)
            }
            hcl::Expression::Number(n) => Ok(n.to_string()),
            hcl::Expression::Bool(b) => Ok(b.to_string()),
            _ => bail!("unsupported expression kind for string value: {expr:?}"),
        }
    }

    pub fn expr_to_string_vec(expr: &hcl::Expression, env: &super::EnvVars) -> Result<Vec<String>> {
        match expr {
            hcl::Expression::Array(a) => a.iter().map(|e| expr_to_string(e, env)).collect(),
            _ => bail!("expected array expression"),
        }
    }

    pub fn expr_to_value(expr: &hcl::Expression, env: &super::EnvVars) -> Result<Value> {
        match expr {
            hcl::Expression::String(s) => Ok(Value::String(s.clone())),
            hcl::Expression::Number(n) => Ok(Value::Number(n.clone())),
            hcl::Expression::Bool(b) => Ok(Value::Bool(*b)),
            hcl::Expression::Traversal(t) => resolve_traversal_value(t, env),
            hcl::Expression::TemplateExpr(t) => {
                let s = super::eval::expr_to_string(&hcl::Expression::TemplateExpr(t.clone()), env)?;
                Ok(Value::String(s))
            }
            hcl::Expression::Array(a) => {
                let mut out = Vec::with_capacity(a.len());
                for e in a {
                    out.push(expr_to_value(e, env)?);
                }
                Ok(Value::from(out))
            }
            hcl::Expression::Object(obj) => {
                let mut map: hcl::value::Map<String, Value> = hcl::value::Map::new();
                for (k, v) in obj {
                    let key: String = k.clone().into();
                    map.insert(key, expr_to_value(v, env)?);
                }
                Ok(Value::Object(map))
            }
            _ => bail!("unsupported expression: {expr:?}"),
        }
    }

    fn value_to_string(v: &Value) -> Result<String> {
        match v {
            Value::String(s) => Ok(s.clone()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            _ => bail!("cannot convert value to string: {v:?}"),
        }
    }

    fn resolve_traversal_value(tr: &Traversal, env: &super::EnvVars) -> Result<Value> {
        let mut it = tr.operators.iter();
        let root = match &tr.expr {
            hcl::Expression::Variable(v) => v.as_str(),
            _ => bail!("unsupported traversal root expression: {:?}", tr.expr),
        };
        match root {
            "var" => {
                let Some(TraversalOperator::GetAttr(name)) = it.next() else {
                    bail!("expected var.<name>");
                };
                env.vars
                    .get(name.as_str())
                    .cloned()
                    .with_context(|| format!("undefined variable '{}': pass --var or default", name))
            }
            "local" | "locals" => {
                let Some(TraversalOperator::GetAttr(name)) = it.next() else {
                    bail!("expected local.<name>");
                };
                env.locals
                    .get(name.as_str())
                    .cloned()
                    .with_context(|| format!("undefined local '{}': define in locals block", name))
            }
            "each" => {
                let Some(TraversalOperator::GetAttr(name)) = it.next() else {
                    bail!("expected each.key or each.value");
                };
                let (key, value) = env
                    .each
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("'each' is only available inside for_each blocks"))?;
                match name.as_str() {
                    "key" => Ok(key.clone().into()),
                    "value" => Ok(value.clone().into()),
                    other => bail!("unsupported each attribute '{}': expected key or value", other),
                }
            }
            _ => bail!("unsupported traversal root '{}': expected var.*, local.*, or each.*", root),
        }
    }
}

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Config {
    pub functions: Vec<FunctionSpec>,
    pub triggers: Vec<TriggerSpec>,
    pub extensions: Vec<ExtensionSpec>,
    pub tests: Vec<TestSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionSpec {
    pub name: String,
    pub schema: Option<String>,
    pub language: String,
    pub returns: String,
    pub replace: bool,
    pub security_definer: bool,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TriggerSpec {
    pub name: String,
    pub schema: Option<String>,
    pub table: String,
    pub timing: String,         // BEFORE | AFTER
    pub events: Vec<String>,    // INSERT | UPDATE | DELETE
    pub level: String,          // ROW | STATEMENT
    pub function: String,       // function name (unqualified)
    pub function_schema: Option<String>,
    pub when: Option<String>,   // optional condition, raw SQL
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtensionSpec {
    pub name: String,
    pub if_not_exists: bool,
    pub schema: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestSpec {
    pub name: String,
    pub setup: Vec<String>,
    pub assert_sql: String,
    pub teardown: Vec<String>,
}

#[derive(Default, Clone, Debug)]
pub struct EnvVars {
    pub vars: HashMap<String, hcl::Value>,
    pub locals: HashMap<String, hcl::Value>,
    pub each: Option<(hcl::Value, hcl::Value)>, // (key, value)
}

pub fn load_root_with_loader(path: &Path, loader: &dyn Loader, root_env: EnvVars) -> Result<Config> {
    let path = if path.is_dir() { path.join("main.hcl") } else { path.to_path_buf() };
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut visited = Vec::new();
    load_file(loader, &path, &base, &root_env, &mut visited)
}

fn load_file(loader: &dyn Loader, path: &Path, base: &Path, parent_env: &EnvVars, visited: &mut Vec<PathBuf>) -> Result<Config> {
    let abspath = path
        .absolutize()
        .map_err(|e| anyhow::anyhow!("absolutize error: {e}"))?
        .to_path_buf();
    if visited.contains(&abspath) {
        bail!("module cycle detected at {}", abspath.display());
    }
    visited.push(abspath.clone());

    let content = loader
        .load(path)
        .with_context(|| format!("reading HCL file {}", path.display()))?;
    let body: hcl::Body = hcl::from_str(&content)
        .with_context(|| format!("parsing HCL in {}", path.display()))?;

    // 1) Collect variable defaults
    let mut var_defaults: HashMap<String, hcl::Value> = HashMap::new();
    for blk in body.blocks().filter(|b| b.identifier() == "variable") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("variable block missing name label"))?
            .as_str()
            .to_string();
        if let Some(attr) = find_attr(blk.body(), "default") {
            let v = eval::expr_to_value(attr.expr(), parent_env)
                .with_context(|| format!("evaluating default for variable '{}')", name))?;
            var_defaults.insert(name, v);
        }
    }

    // Merge env: defaults overridden by parent vars (root) for root file; for modules we override via module call
    let mut env = EnvVars::default();
    env.vars.extend(var_defaults);
    env.vars.extend(parent_env.vars.clone());

    // 2) Compute locals (can reference vars)
    for blk in body.blocks().filter(|b| b.identifier() == "locals") {
        for attr in blk.body().attributes() {
            let key = attr.key();
            let v = eval::expr_to_value(attr.expr(), &env)
                .with_context(|| format!("evaluating local '{}')", key))?;
            env.locals.insert(key.to_string(), v);
        }
    }

    // 3) Parse functions and triggers with env
    let mut cfg = Config::default();

    for blk in body.blocks().filter(|b| b.identifier() == "function") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("function block missing name label"))?
            .as_str()
            .to_string();
        let b = blk.body();
        if let Some(fe) = find_attr(b, "for_each") {
            let coll = eval::expr_to_value(fe.expr(), &env)?;
            for_each_iter(&coll, &mut |k, v| {
                let mut iter_env = env.clone();
                iter_env.each = Some((k.clone(), v.clone()));
                let language = get_attr_string(b, "language", &iter_env)?.unwrap_or_else(|| "plpgsql".to_string());
                let body_sql = get_attr_string(b, "body", &iter_env)?.context("function 'body' is required")?;
                let returns = get_attr_string(b, "returns", &iter_env)?.unwrap_or_else(|| "trigger".to_string());
                let schema = get_attr_string(b, "schema", &iter_env)?;
                let replace = get_attr_bool(b, "replace", &iter_env)?.unwrap_or(true);
                let security_definer = get_attr_bool(b, "security_definer", &iter_env)?.unwrap_or(false);
                cfg.functions.push(FunctionSpec {
                    name: name.clone(),
                    schema,
                    language,
                    returns,
                    replace,
                    security_definer,
                    body: body_sql,
                });
                Ok(())
            })?;
        } else {
            let language = get_attr_string(b, "language", &env)?.unwrap_or_else(|| "plpgsql".to_string());
            let body_sql = get_attr_string(b, "body", &env)?.context("function 'body' is required")?;
            let returns = get_attr_string(b, "returns", &env)?.unwrap_or_else(|| "trigger".to_string());
            let schema = get_attr_string(b, "schema", &env)?;
            let replace = get_attr_bool(b, "replace", &env)?.unwrap_or(true);
            let security_definer = get_attr_bool(b, "security_definer", &env)?.unwrap_or(false);
            cfg.functions.push(FunctionSpec {
                name,
                schema,
                language,
                returns,
                replace,
                security_definer,
                body: body_sql,
            });
        }
    }

    for blk in body.blocks().filter(|b| b.identifier() == "trigger") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("trigger block missing name label"))?
            .as_str()
            .to_string();
        let b = blk.body();
        if let Some(fe) = find_attr(b, "for_each") {
            let coll = eval::expr_to_value(fe.expr(), &env)?;
            match coll {
                hcl::Value::Array(_) | hcl::Value::Object(_) => {
                    for_each_iter(&coll, &mut |k, v| {
                        let mut iter_env = env.clone();
                        iter_env.each = Some((k.clone(), v.clone()));
                        let schema = get_attr_string(b, "schema", &iter_env)?;
                        let table = get_attr_string(b, "table", &iter_env)?.context("trigger 'table' is required")?;
                        let timing = get_attr_string(b, "timing", &iter_env)?.unwrap_or_else(|| "BEFORE".to_string());
                        let events = match find_attr(b, "events") {
                            Some(attr) => eval::expr_to_string_vec(attr.expr(), &iter_env)?,
                            None => vec!["UPDATE".to_string()],
                        };
                        let level = get_attr_string(b, "level", &iter_env)?.unwrap_or_else(|| "ROW".to_string());
                        let function = get_attr_string(b, "function", &iter_env)?.context("trigger 'function' is required")?;
                        let function_schema = get_attr_string(b, "function_schema", &iter_env)?;
                        let when = get_attr_string(b, "when", &iter_env)?;
                        cfg.triggers.push(TriggerSpec {
                            name: name.clone(),
                            schema,
                            table,
                            timing,
                            events,
                            level,
                            function,
                            function_schema,
                            when,
                        });
                        Ok(())
                    })?;
                }
                // Backwards-compat: if for_each is a scalar, treat it as level
                _ => {
                    let schema = get_attr_string(b, "schema", &env)?;
                    let table = get_attr_string(b, "table", &env)?.context("trigger 'table' is required")?;
                    let timing = get_attr_string(b, "timing", &env)?.unwrap_or_else(|| "BEFORE".to_string());
                    let events = match find_attr(b, "events") {
                        Some(attr) => eval::expr_to_string_vec(attr.expr(), &env)?,
                        None => vec!["UPDATE".to_string()],
                    };
                    let level = eval::expr_to_string(fe.expr(), &env)?;
                    let function = get_attr_string(b, "function", &env)?.context("trigger 'function' is required")?;
                    let function_schema = get_attr_string(b, "function_schema", &env)?;
                    let when = get_attr_string(b, "when", &env)?;
                    cfg.triggers.push(TriggerSpec {
                        name,
                        schema,
                        table,
                        timing,
                        events,
                        level,
                        function,
                        function_schema,
                        when,
                    });
                }
            }
        } else {
            let schema = get_attr_string(b, "schema", &env)?;
            let table = get_attr_string(b, "table", &env)?.context("trigger 'table' is required")?;
            let timing = get_attr_string(b, "timing", &env)?.unwrap_or_else(|| "BEFORE".to_string());
            let events = match find_attr(b, "events") {
                Some(attr) => eval::expr_to_string_vec(attr.expr(), &env)?,
                None => vec!["UPDATE".to_string()],
            };
            let level = get_attr_string(b, "level", &env)?.unwrap_or_else(|| "ROW".to_string());
            let function = get_attr_string(b, "function", &env)?.context("trigger 'function' is required")?;
            let function_schema = get_attr_string(b, "function_schema", &env)?;
            let when = get_attr_string(b, "when", &env)?;
            cfg.triggers.push(TriggerSpec {
                name,
                schema,
                table,
                timing,
                events,
                level,
                function,
                function_schema,
                when,
            });
        }
    }

    for blk in body.blocks().filter(|b| b.identifier() == "extension") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("extension block missing name label"))?
            .as_str()
            .to_string();
        let b = blk.body();
        if let Some(fe) = find_attr(b, "for_each") {
            let coll = eval::expr_to_value(fe.expr(), &env)?;
            for_each_iter(&coll, &mut |k, v| {
                let mut iter_env = env.clone();
                iter_env.each = Some((k.clone(), v.clone()));
                let if_not_exists = get_attr_bool(b, "if_not_exists", &iter_env)?.unwrap_or(true);
                let schema = get_attr_string(b, "schema", &iter_env)?;
                let version = get_attr_string(b, "version", &iter_env)?;
                cfg.extensions.push(ExtensionSpec { name: name.clone(), if_not_exists, schema, version });
                Ok(())
            })?;
        } else {
            let if_not_exists = get_attr_bool(b, "if_not_exists", &env)?.unwrap_or(true);
            let schema = get_attr_string(b, "schema", &env)?;
            let version = get_attr_string(b, "version", &env)?;
            cfg.extensions.push(ExtensionSpec { name, if_not_exists, schema, version });
        }
    }

    for blk in body.blocks().filter(|b| b.identifier() == "test") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("test block missing name label"))?
            .as_str()
            .to_string();
        let b = blk.body();
        let setup = match find_attr(b, "setup") {
            Some(attr) => eval::expr_to_string_vec(attr.expr(), &env)?,
            None => Vec::new(),
        };
        let assert_sql = get_attr_string(b, "assert", &env)?.context("test 'assert' is required")?;
        let teardown = match find_attr(b, "teardown") {
            Some(attr) => eval::expr_to_string_vec(attr.expr(), &env)?,
            None => Vec::new(),
        };
        cfg.tests.push(TestSpec { name, setup, assert_sql, teardown });
    }

    // 4) Load modules (merge their resources)
    for blk in body.blocks().filter(|b| b.identifier() == "module") {
        let label = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("module block missing name label"))?;
        let b = blk.body();
        let source = get_attr_string(b, "source", &env)?
            .with_context(|| format!("module '{}' missing 'source'", label.as_str()))?;
        let module_path = resolve_module_path(base, &source)?;
        if let Some(fe) = find_attr(b, "for_each") {
            let coll = eval::expr_to_value(fe.expr(), &env)?;
            for_each_iter(&coll, &mut |k, v| {
                let mut iter_env = env.clone();
                iter_env.each = Some((k.clone(), v.clone()));
                let mut mod_vars: HashMap<String, hcl::Value> = HashMap::new();
                for attr in b.attributes() {
                    let k = attr.key();
                    if k == "source" || k == "for_each" { continue; }
                    let v = eval::expr_to_value(attr.expr(), &iter_env)
                        .with_context(|| format!("evaluating module var '{}.{}'", label.as_str(), k))?;
                    mod_vars.insert(k.to_string(), v);
                }
                let mod_env = EnvVars { vars: mod_vars, locals: HashMap::new(), each: None };
                let sub = load_file(loader, &module_path.join("main.hcl"), &module_path, &mod_env, visited)
                    .with_context(|| format!("loading module '{}' from {}", label.as_str(), module_path.display()))?;
                cfg.functions.extend(sub.functions);
                cfg.triggers.extend(sub.triggers);
                cfg.extensions.extend(sub.extensions);
                Ok(())
            })?;
        } else {
            // Prepare vars for module: start empty, collect its own defaults while loading; pass overrides from attrs (excluding 'source'/'for_each')
            let mut mod_vars: HashMap<String, hcl::Value> = HashMap::new();
            for attr in b.attributes() {
                let k = attr.key();
                if k == "source" || k == "for_each" { continue; }
                let v = eval::expr_to_value(attr.expr(), &env)
                    .with_context(|| format!("evaluating module var '{}.{}'", label.as_str(), k))?;
                mod_vars.insert(k.to_string(), v);
            }
            let mod_env = EnvVars { vars: mod_vars, locals: HashMap::new(), each: None };
            let sub = load_file(loader, &module_path.join("main.hcl"), &module_path, &mod_env, visited)
                .with_context(|| format!("loading module '{}' from {}", label.as_str(), module_path.display()))?;
            cfg.functions.extend(sub.functions);
            cfg.triggers.extend(sub.triggers);
            cfg.extensions.extend(sub.extensions);
        }
    }

    visited.pop();
    Ok(cfg)
}

fn find_attr<'a>(body: &'a hcl::Body, name: &str) -> Option<&'a hcl::Attribute> {
    body.attributes().find(|a| a.key() == name)
}

fn get_attr_string(body: &hcl::Body, name: &str, env: &EnvVars) -> Result<Option<String>> {
    Ok(match find_attr(body, name) {
        Some(attr) => Some(eval::expr_to_string(attr.expr(), env)?),
        None => None,
    })
}

fn get_attr_bool(body: &hcl::Body, name: &str, env: &EnvVars) -> Result<Option<bool>> {
    Ok(match find_attr(body, name) {
        Some(attr) => match attr.expr() {
            hcl::Expression::Bool(b) => Some(*b),
            _ => {
                let v = eval::expr_to_value(attr.expr(), env)?;
                match v {
                    hcl::Value::Bool(b) => Some(b),
                    hcl::Value::String(ref s) if s == "true" => Some(true),
                    hcl::Value::String(ref s) if s == "false" => Some(false),
                    _ => None,
                }
            }
        },
        None => None,
    })
}

fn for_each_iter<F>(collection: &hcl::Value, f: &mut F) -> Result<()>
where
    F: FnMut(hcl::Value, hcl::Value) -> Result<()>,
{
    match collection {
        hcl::Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                f(hcl::Value::Number(hcl::Number::from(i as u64)), v.clone())?;
            }
        }
        hcl::Value::Object(obj) => {
            for (k, v) in obj.iter() {
                f(hcl::Value::String(k.clone()), v.clone())?;
            }
        }
        other => bail!("for_each expects array or object, got {other:?}"),
    }
    Ok(())
}

fn resolve_module_path(base: &Path, source: &str) -> Result<PathBuf> {
    let p = Path::new(source);
    let path = if p.is_absolute() { p.to_path_buf() } else { base.join(p) };
    Ok(path)
}
