use anyhow::Result;
use serde::Serialize;

use crate::model::Config;
use super::Backend;

#[derive(Serialize)]
struct JsonOutput<'a> {
    backend: &'static str,
    #[serde(flatten)]
    config: &'a Config,
}

pub struct JsonBackend;

impl Backend for JsonBackend {
    fn name(&self) -> &'static str { "json" }
    fn file_extension(&self) -> &'static str { "json" }
    fn generate(&self, cfg: &Config) -> Result<String> {
        let output = JsonOutput {
            backend: self.name(),
            config: cfg,
        };
        serde_json::to_string_pretty(&output).map_err(Into::into)
    }
}
