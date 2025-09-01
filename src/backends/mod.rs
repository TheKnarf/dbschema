use anyhow::Result;

use crate::model::Config;

pub mod json;
pub mod postgres;
pub mod prisma;

pub trait Backend {
    fn name(&self) -> &'static str;
    fn file_extension(&self) -> &'static str;
    fn generate(&self, cfg: &Config, env: &crate::model::EnvVars, strict: bool) -> Result<String>;
}

pub fn get_backend(name: &str) -> Option<Box<dyn Backend>> {
    match name.to_lowercase().as_str() {
        "postgres" | "pg" => Some(Box::new(postgres::PostgresBackend)),
        "json" => Some(Box::new(json::JsonBackend)),
        "prisma" => Some(Box::new(prisma::PrismaBackend)),
        _ => None,
    }
}
