use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use dbschema::{load_config, validate, EnvVars, Loader};
use dbschema::test_runner::TestBackend;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "dbschema")] 
#[command(about = "HCL-driven tables, functions & triggers for Postgres", long_about = None)]
struct Cli {
    /// Root HCL file (default: main.hcl)
    #[arg(long, default_value = "main.hcl")]
    input: PathBuf,

    /// Set a variable: --var key=value (repeatable)
    #[arg(long, value_parser = parse_key_val::<String, String>)]
    var: Vec<(String, String)>,

    /// Load variables from a file (HCL or .tfvars-like). Can repeat.
    #[arg(long)]
    var_file: Vec<PathBuf>,

    /// Backend to use: postgres|json
    #[arg(long, default_value = "postgres")]
    backend: String,

    /// Include only these resources (repeatable). If none, includes all.
    #[arg(long = "include", value_enum)]
    include_resources: Vec<ResourceKind>,

    /// Exclude these resources (repeatable)
    #[arg(long = "exclude", value_enum)]
    exclude_resources: Vec<ResourceKind>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate HCL and print a summary
    Validate {},
    /// Create a SQL migration file from the HCL
    CreateMigration {
        /// Output directory for migration files; if omitted, prints to stdout
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Optional migration name (used in filename when writing to a dir)
        #[arg(long)]
        name: Option<String>,
    },
    /// Run tests defined in HCL against a database
    Test {
        /// Database connection string (falls back to env DATABASE_URL)
        #[arg(long)]
        dsn: Option<String>,
        /// Names of tests to run (repeatable). If omitted, runs all.
        #[arg(long = "name")]
        names: Vec<String>,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, ValueEnum)]
enum ResourceKind { Enums, Tables, Views, Materialized, Functions, Triggers, Extensions, Tests }

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut vars: HashMap<String, hcl::Value> = HashMap::new();
    for vf in &cli.var_file {
        let loaded = load_var_file(vf).with_context(|| format!("loading var file {}", vf.display()))?;
        vars.extend(loaded);
    }
    for (k, v) in cli.var.iter() {
        vars.insert(k.clone(), hcl::Value::String(v.clone()));
    }

    let fs_loader = FsLoader;
    let config = load_config(&cli.input, &fs_loader, EnvVars { vars, locals: HashMap::new(), each: None })
        .with_context(|| format!("loading root HCL {}", cli.input.display()))?;

    let filtered = apply_filters(&config, &cli.backend, &cli.include_resources, &cli.exclude_resources);

    match cli.command {
        Commands::Validate {} => {
            validate(&filtered)?;
            println!(
                "Valid: {} enum(s), {} table(s), {} view(s), {} materialized view(s), {} function(s), {} trigger(s)",
                filtered.enums.len(),
                filtered.tables.len(),
                filtered.views.len(),
                filtered.materialized.len(),
                filtered.functions.len(),
                filtered.triggers.len()
            );
        }
        Commands::CreateMigration { out_dir, name } => {
            validate(&filtered)?;
            let artifact = dbschema::generate_with_backend(&cli.backend, &filtered)?;
            if let Some(dir) = out_dir {
                let name = name.unwrap_or_else(|| "triggers".to_string());
                let ext = dbschema::backends::get_backend(&cli.backend)
                    .as_ref()
                    .map(|b| b.file_extension())
                    .unwrap_or("txt");
                let path = write_artifact(&dir, &name, ext, &artifact)?;
                println!("Wrote migration: {}", path.display());
            } else {
                print!("{}", artifact);
            }
        }
        Commands::Test { dsn, names } => {
            let dsn = dsn
                .or_else(|| std::env::var("DATABASE_URL").ok())
                .ok_or_else(|| anyhow::anyhow!("missing DSN: pass --dsn or set DATABASE_URL"))?;
            // Only Postgres tests supported currently
            let runner = dbschema::test_runner::postgres::PostgresTestBackend;
            let only: Option<std::collections::HashSet<String>> = if names.is_empty() {
                None
            } else {
                Some(names.into_iter().collect())
            };
            // Tests run against full config regardless of filters
            let summary = runner.run(&config, &dsn, only.as_ref())?;
            for r in summary.results {
                if r.passed { println!("ok - {}", r.name); } else { println!("FAIL - {}: {}", r.name, r.message); }
            }
            println!("Summary: {} passed, {} failed ({} total)", summary.passed, summary.failed, summary.total);
            if summary.failed > 0 { std::process::exit(1); }
        }
    }

    Ok(())
}

fn write_artifact(out_dir: &Path, name: &str, ext: &str, contents: &str) -> Result<PathBuf> {
    fs::create_dir_all(out_dir)?;
    let ts = chrono::Local::now().format("%Y%m%d%H%M%S");
    let file = format!("{}_{}.{}", ts, sanitize_filename(name), ext);
    let path = out_dir.join(file);
    fs::write(&path, contents)?;
    Ok(path)
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect()
}

fn load_var_file(path: &Path) -> Result<HashMap<String, hcl::Value>> {
    let content = fs::read_to_string(path)?;
    // Try HCL body, collect top-level attributes as strings
    let body: hcl::Body = hcl::from_str(&content).or_else(|_| {
        // Fallback: simple key = "value" lines parsing
        let mut structs: Vec<hcl::Structure> = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                let val = v.trim().trim_matches('"').to_string();
                structs.push(hcl::Structure::Attribute(hcl::Attribute::new(key, val)));
            }
        }
        Ok::<_, hcl::Error>(hcl::Body::from(structs))
    })
        .map_err(|e| anyhow::anyhow!("failed to parse var file as HCL: {e}"))?;

    let mut out = HashMap::new();
    for attr in body.attributes() {
        let name = attr.key();
        let v = dbschema::parser::eval::expr_to_value(attr.expr(), &EnvVars::default())
            .with_context(|| format!("evaluating var '{}': unsupported expression", name))?;
        out.insert(name.to_string(), v);
    }
    Ok(out)
}

fn parse_key_val<K, V>(s: &str) -> Result<(K, V)>
where
    K: std::str::FromStr,
    V: std::str::FromStr,
    <K as std::str::FromStr>::Err: std::fmt::Display,
    <V as std::str::FromStr>::Err: std::fmt::Display,
{
    let pos = s.find('=').ok_or_else(|| anyhow::anyhow!("expected key=value"))?;
    let key = s[..pos]
        .parse()
        .map_err(|e| anyhow::anyhow!("failed to parse key: {e}"))?;
    let value = s[pos + 1..]
        .parse()
        .map_err(|e| anyhow::anyhow!("failed to parse value: {e}"))?;
    Ok((key, value))
}

struct FsLoader;
impl Loader for FsLoader {
    fn load(&self, path: &Path) -> Result<String> {
        Ok(fs::read_to_string(path)?)
    }
}

fn apply_filters(cfg: &dbschema::Config, backend: &str, include: &[ResourceKind], exclude: &[ResourceKind]) -> dbschema::Config {
    use ResourceKind as R;
    let mut inc: std::collections::HashSet<R> = if include.is_empty() {
        [R::Enums, R::Tables, R::Views, R::Materialized, R::Functions, R::Triggers, R::Extensions, R::Tests].into_iter().collect()
    } else {
        include.iter().copied().collect()
    };
    for r in exclude { inc.remove(r); }

    // Prisma backend supports tables and enums; enforce that regardless of flags unless explicitly excluded
    if backend.eq_ignore_ascii_case("prisma") {
        inc = [R::Enums, R::Tables].into_iter().collect();
        // If user excluded tables, keep enums if allowed
        if exclude.iter().any(|e| *e == R::Tables) { inc.retain(|r| *r != R::Tables); }
        // If user excluded enums, keep tables if allowed
        if exclude.iter().any(|e| *e == R::Enums) { inc.retain(|r| *r != R::Enums); }
        // Prisma will only emit tables/enums even if includes specify more
    }

    dbschema::Config {
        enums: if inc.contains(&R::Enums) { cfg.enums.clone() } else { Vec::new() },
        tables: if inc.contains(&R::Tables) { cfg.tables.clone() } else { Vec::new() },
        views: if inc.contains(&R::Views) { cfg.views.clone() } else { Vec::new() },
        materialized: if inc.contains(&R::Materialized) { cfg.materialized.clone() } else { Vec::new() },
        functions: if inc.contains(&R::Functions) { cfg.functions.clone() } else { Vec::new() },
        triggers: if inc.contains(&R::Triggers) { cfg.triggers.clone() } else { Vec::new() },
        extensions: if inc.contains(&R::Extensions) { cfg.extensions.clone() } else { Vec::new() },
        tests: if inc.contains(&R::Tests) { cfg.tests.clone() } else { Vec::new() },
    }
}
