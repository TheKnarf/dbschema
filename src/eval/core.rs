use anyhow::{bail, Context, Result};
use hcl::template::{Element as TplElement, Template};
use hcl::{expr::TemplateExpr, Traversal, TraversalOperator, Value};
use hcl::eval::{Evaluate, Context as HclContext};
use path_absolutize::Absolutize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::model::{Config, EnvVars};
use crate::Loader;
use crate::eval::for_each::execute_for_each;
use crate::eval::builtins;

pub fn expr_to_string(expr: &hcl::Expression, env: &EnvVars) -> Result<String> {
    match expr {
        hcl::Expression::String(s) => Ok(s.clone()),
        hcl::Expression::FuncCall(_) => {
            let v = evaluate_expr(expr, env)?;
            value_to_string(&v)
        }
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

pub fn expr_to_string_vec(expr: &hcl::Expression, env: &EnvVars) -> Result<Vec<String>> {
    match expr {
        hcl::Expression::Array(a) => a.iter().map(|e| expr_to_string(e, env)).collect(),
        _ => bail!("expected array expression"),
    }
}

pub fn expr_to_value(expr: &hcl::Expression, env: &EnvVars) -> Result<Value> {
    match expr {
        hcl::Expression::String(s) => Ok(Value::String(s.clone())),
        hcl::Expression::Number(n) => Ok(Value::Number(n.clone())),
        hcl::Expression::Bool(b) => Ok(Value::Bool(*b)),
        hcl::Expression::FuncCall(_) => evaluate_expr(expr, env),
        hcl::Expression::Traversal(t) => resolve_traversal_value(t, env),
        hcl::Expression::TemplateExpr(t) => {
            let s = expr_to_string(&hcl::Expression::TemplateExpr(t.clone()), env)?;
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

fn resolve_traversal_value(tr: &Traversal, env: &EnvVars) -> Result<Value> {
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

pub fn find_attr<'a>(body: &'a hcl::Body, name: &str) -> Option<&'a hcl::Attribute> {
    body.attributes().find(|a| a.key() == name)
}

pub fn get_attr_string(body: &hcl::Body, name: &str, env: &EnvVars) -> Result<Option<String>> {
    Ok(match find_attr(body, name) {
        Some(attr) => Some(expr_to_string(attr.expr(), env)?),
        None => None,
    })
}

pub fn get_attr_bool(body: &hcl::Body, name: &str, env: &EnvVars) -> Result<Option<bool>> {
    Ok(match find_attr(body, name) {
        Some(attr) => match attr.expr() {
            hcl::Expression::Bool(b) => Some(*b),
            _ => {
                let v = expr_to_value(attr.expr(), env)?;
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

/// Create an HCL evaluation context with built-in functions and custom variable resolvers
pub fn create_eval_context(env: &EnvVars) -> HclContext {
    let mut ctx = builtins::create_context();

    // Add custom variable resolvers for our special variables (var, local, each)
    for (key, value) in &env.vars {
        ctx.declare_var(key.clone(), value.clone());
    }

    for (key, value) in &env.locals {
        // For locals, we'll prefix them to avoid conflicts
        ctx.declare_var(format!("local_{}", key), value.clone());
    }

    if let Some((key, value)) = &env.each {
        ctx.declare_var("each_key", key.clone());
        ctx.declare_var("each_value", value.clone());
    }

    ctx
}

/// Evaluate an expression using the HCL evaluation context with built-in functions
pub fn evaluate_expr(expr: &hcl::Expression, env: &EnvVars) -> Result<Value> {
    // Try to use HCL's built-in evaluation for expressions that support functions
    match expr {
        hcl::Expression::FuncCall(_) => {
            // Function calls should be evaluated by HCL's context
            let ctx = create_eval_context(env);
            // Convert the expression to a string and parse it back as an evaluable expression
            let expr_str = format!("{}", expr);
            let body: hcl::Body = hcl::from_str(&format!("temp = {}", expr_str))
                .map_err(|e| anyhow::anyhow!("Failed to parse expression: {}", e))?;
            let temp_attr = body.attributes().find(|a| a.key() == "temp").unwrap();
            let temp_expr = temp_attr.expr();
            temp_expr.evaluate(&ctx).map_err(|e| anyhow::anyhow!("Function evaluation error: {}", e))
        }
        hcl::Expression::Traversal(tr) => {
            // Handle our custom traversals (var, local, each)
            resolve_traversal_value(tr, env)
        }
        _ => {
            // Fall back to our custom evaluation for other expression types
            expr_to_value(expr, env)
        }
    }
}

pub fn resolve_module_path(base: &Path, source: &str) -> Result<PathBuf> {
    let p = Path::new(source);
    let path = if p.is_absolute() { p.to_path_buf() } else { base.join(p) };
    Ok(path)
}

pub fn load_root_with_loader(path: &Path, loader: &dyn Loader, root_env: EnvVars) -> Result<Config> {
    let path = if path.is_dir() { path.join("main.hcl") } else { path.to_path_buf() };
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut visited = Vec::new();
    let mut cfg = load_file(loader, &path, &base, &root_env, &mut visited)?;
    populate_back_references(&mut cfg)?;
    Ok(cfg)
}

fn populate_back_references(cfg: &mut Config) -> Result<()> {
    let tables = cfg.tables.clone();
    for table in &mut cfg.tables {
        for other_table in &tables {
            for fk in &other_table.foreign_keys {
                if fk.ref_table == table.name {
                    let name = fk.back_reference_name.clone().unwrap_or_else(|| {
                        other_table.name.clone().to_lowercase() + "s"
                    });
                    table.back_references.push(crate::model::BackReferenceSpec {
                        name,
                        table: other_table.name.clone(),
                    });
                }
            }
        }
    }
    Ok(())
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
            let v = expr_to_value(attr.expr(), parent_env)
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
            let v = expr_to_value(attr.expr(), &env)
                .with_context(|| format!("evaluating local '{}')", key))?;
            env.locals.insert(key.to_string(), v);
        }
    }

    // 3) Parse resources using the ForEachSupport trait
    let mut cfg = Config::default();

    // Process each resource type using the trait system
    for blk in body.blocks().filter(|b| b.identifier() == "schema") {
        let name = blk.labels().get(0).ok_or_else(|| anyhow::anyhow!("schema block missing name label"))?.as_str().to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        execute_for_each::<crate::model::SchemaSpec>(&name, blk.body(), &env, &mut cfg, for_each_expr)?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "table") {
        let name = blk.labels().get(0).ok_or_else(|| anyhow::anyhow!("table block missing name label"))?.as_str().to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        execute_for_each::<crate::model::TableSpec>(&name, blk.body(), &env, &mut cfg, for_each_expr)?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "view") {
        let name = blk.labels().get(0).ok_or_else(|| anyhow::anyhow!("view block missing name label"))?.as_str().to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        execute_for_each::<crate::model::ViewSpec>(&name, blk.body(), &env, &mut cfg, for_each_expr)?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "materialized") {
        let name = blk.labels().get(0).ok_or_else(|| anyhow::anyhow!("materialized block missing name label"))?.as_str().to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        execute_for_each::<crate::model::MaterializedViewSpec>(&name, blk.body(), &env, &mut cfg, for_each_expr)?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "policy") {
        let name = blk.labels().get(0).ok_or_else(|| anyhow::anyhow!("policy block missing name label"))?.as_str().to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        execute_for_each::<crate::model::PolicySpec>(&name, blk.body(), &env, &mut cfg, for_each_expr)?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "function") {
        let name = blk.labels().get(0).ok_or_else(|| anyhow::anyhow!("function block missing name label"))?.as_str().to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        execute_for_each::<crate::model::FunctionSpec>(&name, blk.body(), &env, &mut cfg, for_each_expr)?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "trigger") {
        let name = blk.labels().get(0).ok_or_else(|| anyhow::anyhow!("trigger block missing name label"))?.as_str().to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        execute_for_each::<crate::model::TriggerSpec>(&name, blk.body(), &env, &mut cfg, for_each_expr)?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "extension") {
        let name = blk.labels().get(0).ok_or_else(|| anyhow::anyhow!("extension block missing name label"))?.as_str().to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        execute_for_each::<crate::model::ExtensionSpec>(&name, blk.body(), &env, &mut cfg, for_each_expr)?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "enum") {
        let name = blk.labels().get(0).ok_or_else(|| anyhow::anyhow!("enum block missing name label"))?.as_str().to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        execute_for_each::<crate::model::EnumSpec>(&name, blk.body(), &env, &mut cfg, for_each_expr)?;
    }

    // Handle test blocks (these don't use for_each typically)
    for blk in body.blocks().filter(|b| b.identifier() == "test") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("test block missing name label"))?
            .as_str()
            .to_string();
        let b = blk.body();
        let setup = match find_attr(b, "setup") {
            Some(attr) => expr_to_string_vec(attr.expr(), &env)?,
            None => Vec::new(),
        };
        let assert_sql = get_attr_string(b, "assert", &env)?.context("test 'assert' is required")?;
        let teardown = match find_attr(b, "teardown") {
            Some(attr) => expr_to_string_vec(attr.expr(), &env)?,
            None => Vec::new(),
        };
        cfg.tests.push(crate::model::TestSpec { name, setup, assert_sql, teardown });
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
            let coll = expr_to_value(fe.expr(), &env)?;
            crate::eval::for_each::for_each_iter(&coll, &mut |k, v| {
                let mut iter_env = env.clone();
                iter_env.each = Some((k.clone(), v.clone()));
                let mut mod_vars: HashMap<String, hcl::Value> = HashMap::new();
                for attr in b.attributes() {
                    let k = attr.key();
                    if k == "source" || k == "for_each" { continue; }
                    let v = expr_to_value(attr.expr(), &iter_env)
                        .with_context(|| format!("evaluating module var '{}.{}'", label.as_str(), k))?;
                    mod_vars.insert(k.to_string(), v);
                }
                let mod_env = EnvVars { vars: mod_vars, locals: HashMap::new(), each: None };
                let sub = load_file(loader, &module_path.join("main.hcl"), &module_path, &mod_env, visited)
                    .with_context(|| format!("loading module '{}' from {}", label.as_str(), module_path.display()))?;
                cfg.schemas.extend(sub.schemas);
                cfg.enums.extend(sub.enums);
                cfg.functions.extend(sub.functions);
                cfg.triggers.extend(sub.triggers);
                cfg.extensions.extend(sub.extensions);
                cfg.tables.extend(sub.tables);
                cfg.views.extend(sub.views);
                cfg.materialized.extend(sub.materialized);
                cfg.policies.extend(sub.policies);
                Ok(())
            })?;
        } else {
            // Prepare vars for module: start empty, collect its own defaults while loading; pass overrides from attrs (excluding 'source'/'for_each')
            let mut mod_vars: HashMap<String, hcl::Value> = HashMap::new();
            for attr in b.attributes() {
                let k = attr.key();
                if k == "source" || k == "for_each" { continue; }
                let v = expr_to_value(attr.expr(), &env)
                    .with_context(|| format!("evaluating module var '{}.{}'", label.as_str(), k))?;
                mod_vars.insert(k.to_string(), v);
            }
            let mod_env = EnvVars { vars: mod_vars, locals: HashMap::new(), each: None };
            let sub = load_file(loader, &module_path.join("main.hcl"), &module_path, &mod_env, visited)
                .with_context(|| format!("loading module '{}' from {}", label.as_str(), module_path.display()))?;
            cfg.schemas.extend(sub.schemas);
            cfg.enums.extend(sub.enums);
            cfg.functions.extend(sub.functions);
            cfg.triggers.extend(sub.triggers);
            cfg.extensions.extend(sub.extensions);
            cfg.tables.extend(sub.tables);
            cfg.views.extend(sub.views);
            cfg.materialized.extend(sub.materialized);
            cfg.policies.extend(sub.policies);
        }
    }

    visited.pop();
    Ok(cfg)
}