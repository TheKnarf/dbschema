use anyhow::Result;
use serde_json::json;

use super::Backend;
use crate::ir::Config;

pub struct JsonBackend;

impl Backend for JsonBackend {
    fn name(&self) -> &'static str {
        "json"
    }
    fn file_extension(&self) -> &'static str {
        "json"
    }
    fn generate(&self, cfg: &Config, _strict: bool) -> Result<String> {
        let output = json!({
            "backend": self.name(),
            "config": cfg,
        });
        serde_json::to_string_pretty(&output).map_err(Into::into)
    }
}
