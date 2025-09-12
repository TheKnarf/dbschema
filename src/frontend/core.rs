use anyhow::{bail, Context, Result};
use hcl::eval::{Context as HclContext, Evaluate};
use hcl::template::{Element as TplElement, Template};
use hcl::{
    expr::{BinaryOperator, TemplateExpr, UnaryOperator},
    Attribute, Block, Body, Number, Structure, Traversal, TraversalOperator, Value,
};
use path_absolutize::Absolutize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::frontend::ast;
use crate::frontend::ast::VarValidation;
use crate::frontend::builtins;
use crate::frontend::env::{EnvVars, VarSpec, VarType};
use crate::frontend::for_each::execute_for_each;
use crate::frontend::lower;
use crate::ir;
use crate::Loader;

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
                    TplElement::Directive(_) => {
                        bail!("template directives not supported in this context")
                    }
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
        hcl::Expression::Variable(v) => {
            if let Some(val) = env.vars.get(v.as_str()) {
                Ok(val.clone())
            } else if let Some(val) = env.locals.get(v.as_str()) {
                Ok(val.clone())
            } else {
                bail!(
                    "undefined variable '{}': use var.<name> or define in for expression",
                    v.as_str()
                );
            }
        }
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
        hcl::Expression::Conditional(c) => {
            let cond = expr_to_value(&c.cond_expr, env)?;
            match cond {
                Value::Bool(true) => expr_to_value(&c.true_expr, env),
                Value::Bool(false) => expr_to_value(&c.false_expr, env),
                other => bail!(
                    "conditional expression must evaluate to bool, got {}",
                    value_kind(&other)
                ),
            }
        }
        hcl::Expression::ForExpr(fe) => {
            let coll = expr_to_value(&fe.collection_expr, env)?;
            let mut arr_out = Vec::new();
            let mut map_out: hcl::value::Map<String, Value> = hcl::value::Map::new();

            let iter: Vec<(Value, Value)> = match coll {
                Value::Array(a) => a
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| (Value::Number(Number::from(i as i64)), v))
                    .collect(),
                Value::Object(o) => o.into_iter().map(|(k, v)| (Value::from(k), v)).collect(),
                other => bail!(
                    "for expression collection must be array or object, got {}",
                    value_kind(&other)
                ),
            };

            for (key, val) in iter {
                let mut iter_env = env.clone();
                iter_env
                    .vars
                    .insert(fe.value_var.as_str().to_string(), val.clone());
                if let Some(kv) = &fe.key_var {
                    iter_env.vars.insert(kv.as_str().to_string(), key.clone());
                }

                if let Some(cond_expr) = &fe.cond_expr {
                    let cond = expr_to_value(cond_expr, &iter_env)?;
                    match cond {
                        Value::Bool(true) => {}
                        Value::Bool(false) => continue,
                        other => bail!(
                            "for expression condition must evaluate to bool, got {}",
                            value_kind(&other)
                        ),
                    }
                }

                if let Some(key_expr) = &fe.key_expr {
                    let key_val = expr_to_value(key_expr, &iter_env)?;
                    let key_str = match key_val {
                        Value::String(s) => s,
                        Value::Number(n) => n.to_string(),
                        other => bail!(
                            "for expression key must be string or number, got {}",
                            value_kind(&other)
                        ),
                    };
                    let val_expr = expr_to_value(&fe.value_expr, &iter_env)?;
                    if fe.grouping {
                        map_out
                            .entry(key_str)
                            .and_modify(|v| {
                                if let Value::Array(arr) = v {
                                    arr.push(val_expr.clone());
                                } else {
                                    *v = Value::from(vec![v.clone(), val_expr.clone()]);
                                }
                            })
                            .or_insert_with(|| Value::from(vec![val_expr]));
                    } else {
                        map_out.insert(key_str, val_expr);
                    }
                } else {
                    let val_expr = expr_to_value(&fe.value_expr, &iter_env)?;
                    arr_out.push(val_expr);
                }
            }

            if fe.key_expr.is_some() {
                Ok(Value::Object(map_out))
            } else {
                Ok(Value::from(arr_out))
            }
        }
        hcl::Expression::Operation(op) => match &**op {
            hcl::expr::Operation::Unary(u) => {
                let v = expr_to_value(&u.expr, env)?;
                match u.operator {
                    UnaryOperator::Not => match v {
                        Value::Bool(b) => Ok(Value::Bool(!b)),
                        _ => bail!("unsupported operand type for !: {}", value_kind(&v)),
                    },
                    UnaryOperator::Neg => match v {
                        Value::Number(n) => Ok(Value::Number(
                            Number::from_f64(-n.as_f64().unwrap_or(0.0)).unwrap(),
                        )),
                        _ => bail!("unsupported operand type for -: {}", value_kind(&v)),
                    },
                }
            }
            hcl::expr::Operation::Binary(b) => {
                let lhs = expr_to_value(&b.lhs_expr, env)?;
                let rhs = expr_to_value(&b.rhs_expr, env)?;
                match b.operator {
                    BinaryOperator::Eq => Ok(Value::Bool(lhs == rhs)),
                    BinaryOperator::NotEq => Ok(Value::Bool(lhs != rhs)),
                    BinaryOperator::And => match (lhs, rhs) {
                        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
                        (a, b) => bail!(
                            "unsupported operands for &&: {} && {}",
                            value_kind(&a),
                            value_kind(&b)
                        ),
                    },
                    BinaryOperator::Or => match (lhs, rhs) {
                        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
                        (a, b) => bail!(
                            "unsupported operands for ||: {} || {}",
                            value_kind(&a),
                            value_kind(&b)
                        ),
                    },
                    BinaryOperator::Less
                    | BinaryOperator::LessEq
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterEq => {
                        let l = match lhs {
                            Value::Number(n) => n.as_f64().unwrap_or(0.0),
                            _ => bail!(
                                "unsupported operand type for comparison: {}",
                                value_kind(&lhs)
                            ),
                        };
                        let r = match rhs {
                            Value::Number(n) => n.as_f64().unwrap_or(0.0),
                            _ => bail!(
                                "unsupported operand type for comparison: {}",
                                value_kind(&rhs)
                            ),
                        };
                        let res = match b.operator {
                            BinaryOperator::Less => l < r,
                            BinaryOperator::LessEq => l <= r,
                            BinaryOperator::Greater => l > r,
                            BinaryOperator::GreaterEq => l >= r,
                            _ => unreachable!(),
                        };
                        Ok(Value::Bool(res))
                    }
                    _ => bail!("unsupported binary operator: {:?}", b.operator),
                }
            }
        },
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
        "module" => {
            let Some(TraversalOperator::GetAttr(mod_name)) = it.next() else {
                bail!("expected module.<name>.<output>");
            };
            let module_outputs = env
                .modules
                .get(mod_name.as_str())
                .with_context(|| format!("undefined module '{}'", mod_name))?;
            let Some(TraversalOperator::GetAttr(out_name)) = it.next() else {
                bail!("expected module.<name>.<output>");
            };
            let mut current = module_outputs
                .get(out_name.as_str())
                .cloned()
                .with_context(|| format!("undefined module output '{}.{}'", mod_name, out_name))?;
            for op in it {
                match op {
                    TraversalOperator::GetAttr(attr) => {
                        if let Value::Object(map) = current {
                            current = map.get(attr.as_str()).cloned().ok_or_else(|| {
                                anyhow::anyhow!("unknown attribute '{}' on module output", attr)
                            })?;
                        } else {
                            bail!("cannot access attribute on non-object value");
                        }
                    }
                    _ => bail!("unsupported traversal operator in module.* expression"),
                }
            }
            Ok(current)
        }
        "each" => {
            let Some(TraversalOperator::GetAttr(name)) = it.next() else {
                bail!("expected each.key or each.value");
            };
            let (key, value) = env.each.as_ref().ok_or_else(|| {
                anyhow::anyhow!("'each' is only available inside for_each blocks")
            })?;
            let mut current = match name.as_str() {
                "key" => key.clone().into(),
                "value" => value.clone(),
                other => bail!(
                    "unsupported each attribute '{}': expected key or value",
                    other
                ),
            };
            for op in it {
                match op {
                    TraversalOperator::GetAttr(attr) => {
                        if let Value::Object(map) = current {
                            current = map.get(attr.as_str()).cloned().ok_or_else(|| {
                                anyhow::anyhow!("unknown attribute '{}' on each.* value", attr)
                            })?;
                        } else {
                            bail!("cannot access attribute on non-object value");
                        }
                    }
                    _ => bail!("unsupported traversal operator in each.* expression"),
                }
            }
            Ok(current)
        }
        "count" => {
            let Some(TraversalOperator::GetAttr(name)) = it.next() else {
                bail!("expected count.index");
            };
            let idx = env
                .count
                .ok_or_else(|| anyhow::anyhow!("'count' is only available inside count iterations"))?;
            match name.as_str() {
                "index" => {
                    if it.next().is_some() {
                        bail!("count.index does not support further traversal");
                    }
                    Ok(Value::Number(Number::from(idx as u64)))
                }
                other => bail!("unsupported count attribute '{}': expected index", other),
            }
        }
        _ => {
            // Check if the root is a variable in the environment
            if let Some(mut current) = env.vars.get(root).cloned() {
                for op in it {
                    match op {
                        TraversalOperator::GetAttr(attr) => {
                            if let Value::Object(map) = current {
                                current = map.get(attr.as_str()).cloned().ok_or_else(|| {
                                    anyhow::anyhow!("unknown attribute '{}' on variable '{}'", attr, root)
                                })?;
                            } else {
                                bail!("cannot access attribute '{}' on non-object value for variable '{}'", attr, root);
                            }
                        }
                        _ => bail!("unsupported traversal operator on variable '{}'", root),
                    }
                }
                Ok(current)
            } else {
                bail!(
                    "unsupported traversal root '{}': expected var.*, local.*, module.*, each.*, count.*, or a variable name",
                    root
                );
            }
        }
    }
}

// Expand Terraform-style dynamic blocks into concrete blocks.
// This allows constructs like:
//
// dynamic "column" {
//   for_each = var.cols
//   labels   = [each.key]
//   content {
//     type    = each.value.type
//     nullable = each.value.nullable
//   }
// }
//
// to be turned into multiple `column` blocks.
fn expand_dynamic_blocks(body: &Body, env: &EnvVars) -> Result<Body> {
    let mut builder = Body::builder();
    for structure in body.iter() {
        match structure {
            Structure::Attribute(attr) => {
                if env.each.is_some() {
                    let val = expr_to_value(attr.expr(), env)?;
                    builder = builder.add_attribute(Attribute::new(attr.key().to_string(), val));
                } else {
                    builder = builder.add_attribute(attr.clone());
                }
            }
            Structure::Block(block) => {
                if block.identifier() == "dynamic" {
                    // The first label is the resulting block identifier
                    let ident = block
                        .labels()
                        .get(0)
                        .ok_or_else(|| anyhow::anyhow!("dynamic block missing type label"))?
                        .as_str()
                        .to_string();

                    // Retrieve required for_each expression
                    let for_each_attr = find_attr(block.body(), "for_each")
                        .context("dynamic block missing for_each")?;
                    let coll = expr_to_value(for_each_attr.expr(), env)?;

                    // Optional labels expression
                    let labels_attr = find_attr(block.body(), "labels");

                    // Content block contains the actual body
                    let content_block = block
                        .body()
                        .blocks()
                        .find(|b| b.identifier() == "content")
                        .context("dynamic block missing content")?;

                    let mut new_blocks = Vec::new();
                    crate::frontend::for_each::for_each_iter(&coll, &mut |k, v| {
                        let mut iter_env = env.clone();
                        iter_env.each = Some((k.clone(), v.clone()));

                        let labels: Vec<String> = match labels_attr {
                            Some(attr) => expr_to_string_vec(attr.expr(), &iter_env)?,
                            None => Vec::new(),
                        };

                        let expanded = expand_dynamic_blocks(content_block.body(), &iter_env)?;

                        let mut bb = Block::builder(ident.clone());
                        if !labels.is_empty() {
                            bb = bb.add_labels(labels);
                        }
                        bb = bb.add_structures(expanded.into_iter());
                        new_blocks.push(bb.build());
                        Ok(())
                    })?;
                    builder = builder.add_blocks(new_blocks);
                } else {
                    let expanded = expand_dynamic_blocks(block.body(), env)?;
                    let mut bb = Block::builder(block.identifier().to_string());
                    if !block.labels().is_empty() {
                        bb = bb.add_labels(block.labels().iter().cloned());
                    }
                    bb = bb.add_structures(expanded.into_iter());
                    builder = builder.add_block(bb.build());
                }
            }
        }
    }
    Ok(builder.build())
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
pub fn create_eval_context(env: &EnvVars) -> HclContext<'_> {
    let mut ctx = builtins::create_context();

    // Add custom variable resolvers for our special variables (var, local, each)
    for (key, value) in &env.vars {
        ctx.declare_var(key.clone(), value.clone());
    }

    for (key, value) in &env.locals {
        // Declare locals directly without prefix to match how they're referenced in expressions
        ctx.declare_var(key.clone(), value.clone());
    }

    if let Some((key, value)) = &env.each {
        ctx.declare_var("each_key", key.clone());
        ctx.declare_var("each_value", value.clone());
    }

    if let Some(index) = env.count {
        ctx.declare_var("count_index", Value::Number(Number::from(index as u64)));
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
            // Try to evaluate the expression directly using HCL's evaluation
            // This avoids the string conversion and re-parsing that was causing issues
            match expr.evaluate(&ctx) {
                Ok(value) => Ok(value),
                Err(_) => {
                    // Fallback to the old string conversion approach for compatibility
                    let expr_str = format!("{}", expr);
                    let body: hcl::Body = hcl::from_str(&format!("temp = {}", expr_str))
                        .map_err(|e| anyhow::anyhow!("Failed to parse expression: {}", e))?;
                    let temp_attr = body.attributes().find(|a| a.key() == "temp").unwrap();
                    let temp_expr = temp_attr.expr();
                    temp_expr
                        .evaluate(&ctx)
                        .map_err(|e| anyhow::anyhow!("Function evaluation error: {}", e))
                }
            }
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
    let path = if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    };
    Ok(path)
}

pub fn load_root_with_loader(
    path: &Path,
    loader: &dyn Loader,
    root_env: EnvVars,
) -> Result<ir::Config> {
    let path = if path.is_dir() {
        path.join("main.hcl")
    } else {
        path.to_path_buf()
    };
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut visited = Vec::new();
    let ast_cfg = load_file(loader, &path, &base, &root_env, &mut visited)?;
    let mut cfg = lower::lower_config(ast_cfg);
    populate_back_references(&mut cfg)?;
    Ok(cfg)
}

fn populate_back_references(cfg: &mut ir::Config) -> Result<()> {
    let tables = cfg.tables.clone();
    for table in &mut cfg.tables {
        for other_table in &tables {
            for fk in &other_table.foreign_keys {
                // Match either the resource name or the explicit table_name (alt_name)
                let matches_name = fk.ref_table == table.name;
                let matches_alt = table
                    .alt_name
                    .as_ref()
                    .map(|an| fk.ref_table == *an)
                    .unwrap_or(false);
                if matches_name || matches_alt {
                    let name = fk
                        .back_reference_name
                        .clone()
                        .unwrap_or_else(|| other_table.name.clone().to_lowercase() + "s");
                    // Prefer the concrete table name when present so downstream backends
                    // (like Prisma) can use it directly for model naming.
                    let target_table = other_table
                        .alt_name
                        .clone()
                        .unwrap_or_else(|| other_table.name.clone());
                    table.back_references.push(crate::ir::BackReferenceSpec {
                        name,
                        table: target_table,
                    });
                }
            }
        }
    }
    Ok(())
}

fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "bool",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        _ => "unknown",
    }
}

fn check_var_type(name: &str, v: &Value, expected: &VarType) -> Result<()> {
    match expected {
        VarType::String => {
            if matches!(v, Value::String(_)) {
                Ok(())
            } else {
                bail!(
                    "variable '{name}' expected type string, got {}",
                    value_kind(v)
                )
            }
        }
        VarType::Number => {
            if matches!(v, Value::Number(_)) {
                Ok(())
            } else {
                bail!(
                    "variable '{name}' expected type number, got {}",
                    value_kind(v)
                )
            }
        }
        VarType::Bool => {
            if matches!(v, Value::Bool(_)) {
                Ok(())
            } else {
                bail!(
                    "variable '{name}' expected type bool, got {}",
                    value_kind(v)
                )
            }
        }
        VarType::List(inner) => match v {
            Value::Array(arr) => {
                for (i, item) in arr.iter().enumerate() {
                    check_var_type(&format!("{name}[{i}]"), item, inner)?;
                }
                Ok(())
            }
            _ => bail!(
                "variable '{name}' expected type {expected}, got {}",
                value_kind(v)
            ),
        },
        VarType::Map(inner) => match v {
            Value::Object(map) => {
                for (k, item) in map.iter() {
                    check_var_type(&format!("{name}.{k}"), item, inner)?;
                }
                Ok(())
            }
            _ => bail!(
                "variable '{name}' expected type {expected}, got {}",
                value_kind(v)
            ),
        },
    }
}

fn load_file(
    loader: &dyn Loader,
    path: &Path,
    base: &Path,
    parent_env: &EnvVars,
    visited: &mut Vec<PathBuf>,
) -> Result<ast::Config> {
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
    let mut body: hcl::Body =
        hcl::from_str(&content).with_context(|| format!("parsing HCL in {}", path.display()))?;

    // 1) Collect variable specs (default/type/validation)
    let mut var_specs: HashMap<String, VarSpec> = HashMap::new();
    for blk in body.blocks().filter(|b| b.identifier() == "variable") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("variable block missing name label"))?
            .as_str()
            .to_string();
        let mut spec = VarSpec::default();
        if let Some(attr) = find_attr(blk.body(), "default") {
            let v = expr_to_value(attr.expr(), parent_env)
                .with_context(|| format!("evaluating default for variable '{}')", name))?;
            spec.default = Some(v);
        }
        if let Some(attr) = find_attr(blk.body(), "type") {
            let t = expr_to_string(attr.expr(), parent_env)
                .with_context(|| format!("evaluating type for variable '{}')", name))?;
            spec.r#type = Some(
                t.parse()
                    .with_context(|| format!("parsing type for variable '{name}'"))?,
            );
        }
        if let Some(vblk) = blk.body().blocks().find(|b| b.identifier() == "validation") {
            let cond_attr = find_attr(vblk.body(), "condition")
                .ok_or_else(|| anyhow::anyhow!("validation block missing 'condition'"))?;
            let err_attr = find_attr(vblk.body(), "error_message")
                .ok_or_else(|| anyhow::anyhow!("validation block missing 'error_message'"))?;
            spec.validation = Some(VarValidation {
                condition: cond_attr.expr().clone(),
                error_message: err_attr.expr().clone(),
            });
        }
        var_specs.insert(name, spec);
    }

    // Merge env: defaults overridden by parent vars (root) for root file; for modules we override via module call
    let mut env = EnvVars::default();
    for (name, spec) in &var_specs {
        if let Some(v) = &spec.default {
            env.vars.insert(name.clone(), v.clone());
        }
    }
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

    // Enforce variable types and run validations
    for (name, spec) in &var_specs {
        if let Some(value) = env.vars.get(name) {
            if let Some(t) = &spec.r#type {
                check_var_type(name, value, t)?;
            }
            if let Some(vspec) = &spec.validation {
                let v = expr_to_value(&vspec.condition, &env)
                    .with_context(|| format!("evaluating validation for variable '{}')", name))?;
                match v {
                    Value::Bool(true) => {}
                    Value::Bool(false) => {
                        let msg =
                            expr_to_string(&vspec.error_message, &env).with_context(|| {
                                format!(
                                    "evaluating validation error_message for variable '{}')",
                                    name
                                )
                            })?;
                        bail!(msg);
                    }
                    other => bail!(
                        "validation for variable '{}' must return a bool, got {}",
                        name,
                        value_kind(&other)
                    ),
                }
            }
        }
    }

    // Expand any dynamic blocks now that variables and locals are known
    body = expand_dynamic_blocks(&body, &env)?;

    // 3) Load modules first so their outputs are available
    let mut cfg = ast::Config::default();
    for blk in body.blocks().filter(|b| b.identifier() == "module") {
        let label = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("module block missing name label"))?;
        let b = blk.body();
        let source = get_attr_string(b, "source", &env)?
            .with_context(|| format!("module '{}' missing 'source'", label.as_str()))?;
        let module_path = resolve_module_path(base, &source)?;
        let for_each_attr = find_attr(b, "for_each");
        let count_attr = find_attr(b, "count");
        if for_each_attr.is_some() && count_attr.is_some() {
            bail!(
                "module '{}' cannot have both for_each and count",
                label.as_str()
            );
        }
        if let Some(fe) = for_each_attr {
            let coll = expr_to_value(fe.expr(), &env)?;
            crate::frontend::for_each::for_each_iter(&coll, &mut |k, v| {
                let mut iter_env = env.clone();
                iter_env.each = Some((k.clone(), v.clone()));
                let mut mod_vars: HashMap<String, hcl::Value> = HashMap::new();
                for attr in b.attributes() {
                    let k = attr.key();
                    if k == "source" || k == "for_each" || k == "count" {
                        continue;
                    }
                    let v = expr_to_value(attr.expr(), &iter_env).with_context(|| {
                        format!("evaluating module var '{}.{}'", label.as_str(), k)
                    })?;
                    mod_vars.insert(k.to_string(), v);
                }
                let mod_env = EnvVars {
                    vars: mod_vars,
                    locals: HashMap::new(),
                    modules: HashMap::new(),
                    each: None,
                    count: None,
                };
                let sub = load_file(
                    loader,
                    &module_path.join("main.hcl"),
                    &module_path,
                    &mod_env,
                    visited,
                )
                .with_context(|| {
                    format!(
                        "loading module '{}' from {}",
                        label.as_str(),
                        module_path.display()
                    )
                })?;
                cfg.schemas.extend(sub.schemas);
                cfg.enums.extend(sub.enums);
                cfg.functions.extend(sub.functions);
                cfg.triggers.extend(sub.triggers);
                cfg.extensions.extend(sub.extensions);
                cfg.sequences.extend(sub.sequences);
                cfg.tables.extend(sub.tables);
                cfg.views.extend(sub.views);
                cfg.materialized.extend(sub.materialized);
                cfg.policies.extend(sub.policies);
                // Outputs from for_each modules aren't accessible via module.*
                Ok(())
            })?;
        } else if let Some(ce) = count_attr {
            let val = expr_to_value(ce.expr(), &env)?;
            let times = match val {
                hcl::Value::Number(n) => n
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("count must be a non-negative integer"))?
                    as usize,
                other => bail!("count expects number, got {other:?}"),
            };
            for i in 0..times {
                let mut iter_env = env.clone();
                iter_env.count = Some(i);
                let mut mod_vars: HashMap<String, hcl::Value> = HashMap::new();
                for attr in b.attributes() {
                    let k = attr.key();
                    if k == "source" || k == "for_each" || k == "count" {
                        continue;
                    }
                    let v = expr_to_value(attr.expr(), &iter_env).with_context(|| {
                        format!("evaluating module var '{}.{}'", label.as_str(), k)
                    })?;
                    mod_vars.insert(k.to_string(), v);
                }
                let mod_env = EnvVars {
                    vars: mod_vars,
                    locals: HashMap::new(),
                    modules: HashMap::new(),
                    each: None,
                    count: None,
                };
                let sub = load_file(
                    loader,
                    &module_path.join("main.hcl"),
                    &module_path,
                    &mod_env,
                    visited,
                )
                .with_context(|| {
                    format!(
                        "loading module '{}' from {}",
                        label.as_str(),
                        module_path.display()
                    )
                })?;
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
        } else {
            // Prepare vars for module: start empty, collect its own defaults while loading; pass overrides from attrs (excluding 'source'/'for_each'/'count')
            let mut mod_vars: HashMap<String, hcl::Value> = HashMap::new();
            for attr in b.attributes() {
                let k = attr.key();
                if k == "source" || k == "for_each" || k == "count" {
                    continue;
                }
                let v = expr_to_value(attr.expr(), &env)
                    .with_context(|| format!("evaluating module var '{}.{}'", label.as_str(), k))?;
                mod_vars.insert(k.to_string(), v);
            }
            let mod_env = EnvVars {
                vars: mod_vars,
                locals: HashMap::new(),
                modules: HashMap::new(),
                each: None,
                count: None,
            };
            let sub = load_file(
                loader,
                &module_path.join("main.hcl"),
                &module_path,
                &mod_env,
                visited,
            )
            .with_context(|| {
                format!(
                    "loading module '{}' from {}",
                    label.as_str(),
                    module_path.display()
                )
            })?;
            // Store module outputs for traversal
            let mut map = HashMap::new();
            for o in &sub.outputs {
                map.insert(o.name.clone(), o.value.clone());
            }
            env.modules.insert(label.as_str().to_string(), map);
            cfg.schemas.extend(sub.schemas);
            cfg.enums.extend(sub.enums);
            cfg.domains.extend(sub.domains);
            cfg.types.extend(sub.types);
            cfg.functions.extend(sub.functions);
            cfg.triggers.extend(sub.triggers);
            cfg.extensions.extend(sub.extensions);
            cfg.sequences.extend(sub.sequences);
            cfg.tables.extend(sub.tables);
            cfg.indexes.extend(sub.indexes);
            cfg.views.extend(sub.views);
            cfg.materialized.extend(sub.materialized);
            cfg.policies.extend(sub.policies);
            cfg.roles.extend(sub.roles);
            cfg.grants.extend(sub.grants);
        }
    }

    // 4) Parse resources using the ForEachSupport trait
    // Process each resource type using the trait system
    for blk in body.blocks().filter(|b| b.identifier() == "schema") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("schema block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstSchema>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "sequence") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("sequence block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstSequence>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "table") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("table block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstTable>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "index") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("index block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstStandaloneIndex>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "view") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("view block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstView>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "materialized") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("materialized block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstMaterializedView>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "policy") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("policy block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstPolicy>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "function") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("function block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstFunction>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "procedure") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("procedure block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstProcedure>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "aggregate") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("aggregate block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstAggregate>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "trigger") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("trigger block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstTrigger>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "event_trigger") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("event_trigger block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstEventTrigger>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "extension") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("extension block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstExtension>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "collation") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("collation block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstCollation>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "enum") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("enum block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstEnum>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "domain") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("domain block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstDomain>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "type") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("type block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstCompositeType>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "foreign_data_wrapper") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("foreign_data_wrapper block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstForeignDataWrapper>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "foreign_server") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("foreign_server block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstForeignServer>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "foreign_table") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("foreign_table block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstForeignTable>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "role") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("role block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstRole>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "tablespace") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("tablespace block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstTablespace>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
    }

    for blk in body.blocks().filter(|b| b.identifier() == "grant") {
        let name = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("grant block missing name label"))?
            .as_str()
            .to_string();
        let for_each_expr = find_attr(blk.body(), "for_each");
        let count_expr = find_attr(blk.body(), "count");
        execute_for_each::<ast::AstGrant>(
            &name,
            blk.body(),
            &env,
            &mut cfg,
            for_each_expr,
            count_expr,
        )?;
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
            Some(attr) => expr_to_string_vec(attr.expr(), &env)?,
            None => Vec::new(),
        };
        // 'assert' can be a string or an array of strings
        let asserts: Vec<String> = match find_attr(b, "assert") {
            Some(attr) => match expr_to_string_vec(attr.expr(), &env) {
                Ok(v) => v,
                Err(_) => {
                    // Fallback: try single string
                    vec![get_attr_string(b, "assert", &env)?.context("test 'assert' is required")?]
                }
            },
            None => Vec::new(),
        };
        // Negative asserts expected to fail
        let assert_fail = match find_attr(b, "assert_fail") {
            Some(attr) => expr_to_string_vec(attr.expr(), &env)?,
            None => Vec::new(),
        };
        if asserts.is_empty() && assert_fail.is_empty() {
            return Err(anyhow::anyhow!(
                "test '{}' must define 'assert' or 'assert_fail'",
                name
            ));
        }
        let teardown = match find_attr(b, "teardown") {
            Some(attr) => expr_to_string_vec(attr.expr(), &env)?,
            None => Vec::new(),
        };
        cfg.tests.push(ast::AstTest {
            name,
            setup,
            asserts,
            assert_fail,
            teardown,
        });
    }

    // Handle output blocks
    for blk in body.blocks().filter(|b| b.identifier() == "output") {
        let label = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("output block missing name label"))?
            .as_str()
            .to_string();
        let b = blk.body();
        let value_attr =
            find_attr(b, "value").context("output block requires 'value' attribute")?;
        let value = expr_to_value(value_attr.expr(), &env)
            .with_context(|| format!("evaluating output '{}'", label))?;
        cfg.outputs.push(ast::AstOutput { name: label, value });
    }

    visited.pop();
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::env::EnvVars;

    #[test]
    fn evaluates_conditional_expression() {
        let expr: hcl::Expression = "true ? 1 : 0".parse().unwrap();
        let env = EnvVars::default();
        let v = expr_to_value(&expr, &env).unwrap();
        assert_eq!(v, Value::from(1));
    }

    #[test]
    fn evaluates_for_expression() {
        let expr: hcl::Expression = "[for x in [1,2,3] : x]".parse().unwrap();
        let env = EnvVars::default();
        let v = expr_to_value(&expr, &env).unwrap();
        let expected = Value::from(vec![Value::from(1), Value::from(2), Value::from(3)]);
        assert_eq!(v, expected);
    }
}
