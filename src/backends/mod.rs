use anyhow::Result;

use crate::parser::Config;

pub mod postgres;
pub mod json;

pub trait Backend {
    fn name(&self) -> &'static str;
    fn file_extension(&self) -> &'static str;
    fn generate(&self, cfg: &Config) -> Result<String>;
}

pub fn get_backend(name: &str) -> Option<Box<dyn Backend>> {
    match name.to_lowercase().as_str() {
        "postgres" | "pg" => Some(Box::new(postgres::PostgresBackend)),
        "json" => Some(Box::new(json::JsonBackend)),
        _ => None,
    }
}

