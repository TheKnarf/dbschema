use anyhow::Result;

use crate::{parser::Config, sql};
use super::Backend;

pub struct PostgresBackend;

impl Backend for PostgresBackend {
    fn name(&self) -> &'static str { "postgres" }
    fn file_extension(&self) -> &'static str { "sql" }
    fn generate(&self, cfg: &Config) -> Result<String> {
        sql::to_sql(cfg)
    }
}

