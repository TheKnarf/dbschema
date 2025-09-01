use crate::Loader;
use anyhow::Result;
use std::path::Path;

pub fn load_root_with_loader(
    path: &Path,
    loader: &dyn Loader,
    root_env: crate::model::EnvVars,
) -> Result<crate::model::Config> {
    crate::eval::load_root_with_loader(path, loader, root_env)
}

pub fn find_attr<'a>(body: &'a hcl::Body, name: &str) -> Option<&'a hcl::Attribute> {
    body.attributes().find(|a| a.key() == name)
}

pub fn get_attr_string(
    body: &hcl::Body,
    name: &str,
    env: &crate::model::EnvVars,
) -> Result<Option<String>> {
    Ok(match find_attr(body, name) {
        Some(attr) => Some(crate::eval::expr_to_string(attr.expr(), env)?),
        None => None,
    })
}

pub fn get_attr_bool(
    body: &hcl::Body,
    name: &str,
    env: &crate::model::EnvVars,
) -> Result<Option<bool>> {
    Ok(match find_attr(body, name) {
        Some(attr) => match attr.expr() {
            hcl::Expression::Bool(b) => Some(*b),
            _ => {
                let v = crate::eval::expr_to_value(attr.expr(), env)?;
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
