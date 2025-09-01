use anyhow::Result;
use serde_json::json;

use super::Backend;
use crate::model::{Config, EnvVars};

pub struct JsonBackend;

impl Backend for JsonBackend {
    fn name(&self) -> &'static str {
        "json"
    }
    fn file_extension(&self) -> &'static str {
        "json"
    }
    fn generate(&self, cfg: &Config, env: &EnvVars, _strict: bool) -> Result<String> {
        let mut vars_json = serde_json::Map::new();
        for (k, v) in &env.vars {
            vars_json.insert(k.clone(), hcl_to_json(v)?);
        }

        let output = json!({
            "backend": self.name(),
            "config": cfg,
            "vars": vars_json,
        });
        serde_json::to_string_pretty(&output).map_err(Into::into)
    }
}

fn hcl_to_json(value: &hcl::Value) -> Result<serde_json::Value> {
    match value {
        hcl::Value::Null => Ok(serde_json::Value::Null),
        hcl::Value::Bool(b) => Ok(serde_json::Value::Bool(*b)),
        hcl::Value::Number(n) => Ok(serde_json::Value::Number(
            serde_json::Number::from_f64(n.as_f64().unwrap()).unwrap(),
        )),
        hcl::Value::String(s) => Ok(serde_json::Value::String(s.clone())),
        hcl::Value::Array(arr) => {
            let mut values = Vec::new();
            for v in arr {
                values.push(hcl_to_json(v)?);
            }
            Ok(serde_json::Value::Array(values))
        }
        hcl::Value::Object(map) => {
            let mut json_map = serde_json::Map::new();
            for (k, v) in map {
                json_map.insert(k.clone(), hcl_to_json(v)?);
            }
            Ok(serde_json::Value::Object(json_map))
        }
    }
}
