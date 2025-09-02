use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use dbschema::frontend::env::EnvVars;
use dbschema::test_runner::TestBackend;
use dbschema::{
    apply_filters,
    config::{self, Config as DbschemaConfig, ResourceKind, TargetConfig},
    load_config, validate, Loader, OutputSpec,
};
#[cfg(feature = "pglite")]
use fallible_iterator::FallibleIterator;
use log::{error, info};
#[cfg(feature = "pglite")]
use postgres_protocol::message::backend;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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

    /// Backend to use: postgres|prisma|json (ignored if using config file)
    #[arg(long, default_value = "postgres")]
    backend: String,

    /// Include only these resources (repeatable). If none, includes all.
    #[arg(long = "include", value_enum)]
    include_resources: Vec<ResourceKind>,

    /// Exclude these resources (repeatable)
    #[arg(long = "exclude", value_enum)]
    exclude_resources: Vec<ResourceKind>,

    /// Use dbschema.toml configuration file
    #[arg(long)]
    config: bool,

    /// Target name to run (when using config file)
    #[arg(long)]
    target: Option<String>,

    /// Enable strict mode (errors on undefined enums)
    #[arg(long)]
    strict: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Copy, Clone, ValueEnum)]
enum TestBackendKind {
    Postgres,
    #[cfg(feature = "pglite")]
    Pglite,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate HCL and print a summary
    Validate {},
    /// Format HCL files in place
    Fmt {
        /// Files or directories to format (defaults to current directory)
        #[arg(value_name = "PATH", default_value = ".")]
        paths: Vec<PathBuf>,
    },
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
        /// Test backend: postgres or pglite
        #[arg(long, value_enum, default_value = "postgres")]
        backend: TestBackendKind,
        /// Names of tests to run (repeatable). If omitted, runs all.
        #[arg(long = "name")]
        names: Vec<String>,
    },
    /// Start an in-memory PGlite database REPL
    #[cfg(feature = "pglite")]
    Pglite {},
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    if cli.config && cli.command.is_none() {
        let dbschema_config = config::load_config()
            .with_context(|| "failed to load dbschema.toml")?
            .ok_or_else(|| anyhow!("dbschema.toml not found"))?;

        let targets_to_run = if let Some(name) = cli.target {
            vec![dbschema_config
                .targets
                .iter()
                .find(|t| t.name == name)
                .ok_or_else(|| anyhow!("target '{}' not found in dbschema.toml", name))?
                .clone()]
        } else {
            dbschema_config.targets.clone()
        };

        for target in targets_to_run {
            run_target(&dbschema_config, &target, cli.strict)?;
        }
    } else if let Some(command) = cli.command {
        match command {
            Commands::Validate {} => {
                let mut vars: HashMap<String, hcl::Value> = HashMap::new();
                for vf in &cli.var_file {
                    let loaded = load_var_file(vf)
                        .with_context(|| format!("loading var file {}", vf.display()))?;
                    vars.extend(loaded);
                }
                for (k, v) in cli.var.iter() {
                    vars.insert(k.clone(), hcl::Value::String(v.clone()));
                }

                let fs_loader = FsLoader;
                let env = EnvVars {
                    vars,
                    locals: HashMap::new(),
                    modules: HashMap::new(),
                    each: None,
                };
                let config = load_config(&cli.input, &fs_loader, env.clone())
                    .with_context(|| format!("loading root HCL {}", cli.input.display()))?;

                let (include_set, exclude_set) =
                    cli_filter_sets(&cli.backend, &cli.include_resources, &cli.exclude_resources);
                let filtered = apply_filters(&config, &include_set, &exclude_set);

                dbschema::validate(&filtered, cli.strict)?;
                info!(
                    "Valid: {} schema(s), {} enum(s), {} table(s), {} view(s), {} materialized view(s), {} function(s), {} trigger(s)",
                    filtered.schemas.len(),
                    filtered.enums.len(),
                    filtered.tables.len(),
                    filtered.views.len(),
                    filtered.materialized.len(),
                    filtered.functions.len(),
                    filtered.triggers.len()
                );
                print_outputs(&filtered.outputs);
            }
            Commands::Fmt { paths } => {
                for p in paths {
                    format_path(&p)?;
                }
            }
            Commands::CreateMigration { out_dir, name } => {
                let mut vars: HashMap<String, hcl::Value> = HashMap::new();
                for vf in &cli.var_file {
                    let loaded = load_var_file(vf)
                        .with_context(|| format!("loading var file {}", vf.display()))?;
                    vars.extend(loaded);
                }
                for (k, v) in cli.var.iter() {
                    vars.insert(k.clone(), hcl::Value::String(v.clone()));
                }

                let fs_loader = FsLoader;
                let env = EnvVars {
                    vars,
                    locals: HashMap::new(),
                    modules: HashMap::new(),
                    each: None,
                };
                let config = load_config(&cli.input, &fs_loader, env.clone())
                    .with_context(|| format!("loading root HCL {}", cli.input.display()))?;

                let (include_set, exclude_set) =
                    cli_filter_sets(&cli.backend, &cli.include_resources, &cli.exclude_resources);
                let filtered = apply_filters(&config, &include_set, &exclude_set);

                dbschema::validate(&filtered, cli.strict)?;
                let artifact =
                    dbschema::generate_with_backend(&cli.backend, &filtered, cli.strict)?;
                if let Some(dir) = out_dir {
                    let name = name.unwrap_or_else(|| "triggers".to_string());
                    let ext = dbschema::backends::get_backend(&cli.backend)
                        .as_ref()
                        .map(|b| b.file_extension())
                        .unwrap_or("txt");
                    let path = write_artifact(&dir, &name, ext, &artifact)?;
                    info!("Wrote migration: {}", path.display());
                } else {
                    print!("{}", artifact);
                }
                print_outputs(&filtered.outputs);
            }
            Commands::Test {
                dsn,
                names,
                backend,
            } => {
                let (dsn, backend, config) = if cli.config {
                    let dbschema_config = config::load_config()
                        .with_context(|| "failed to load dbschema.toml")?
                        .ok_or_else(|| anyhow!("dbschema.toml not found"))?;
                    for (key, value) in &dbschema_config.settings.env {
                        std::env::set_var(key, value);
                    }
                    let mut vars: HashMap<String, hcl::Value> = HashMap::new();
                    for vf in &dbschema_config.settings.var_files {
                        vars.extend(load_var_file(&PathBuf::from(vf))?);
                    }
                    let input_path = dbschema_config
                        .settings
                        .input
                        .as_deref()
                        .unwrap_or("main.hcl");
                    let fs_loader = FsLoader;
                    let env = EnvVars {
                        vars,
                        locals: HashMap::new(),
                        modules: HashMap::new(),
                        each: None,
                    };
                    let cfg = load_config(&PathBuf::from(input_path), &fs_loader, env.clone())
                        .with_context(|| format!("loading root HCL from {}", input_path))?;
                    let dsn = dsn
                        .or_else(|| dbschema_config.settings.test_dsn.clone())
                        .or_else(|| std::env::var("DATABASE_URL").ok());
                    let mut backend_choice = backend;
                    if matches!(backend_choice, TestBackendKind::Postgres) {
                        if let Some(be) = &dbschema_config.settings.test_backend {
                            backend_choice = match be.as_str() {
                                "postgres" => TestBackendKind::Postgres,
                                #[cfg(feature = "pglite")]
                                "pglite" => TestBackendKind::Pglite,
                                other => return Err(anyhow!("unknown test backend '{other}'")),
                            };
                        }
                    }
                    (dsn, backend_choice, cfg)
                } else {
                    let mut vars: HashMap<String, hcl::Value> = HashMap::new();
                    for vf in &cli.var_file {
                        let loaded = load_var_file(vf)
                            .with_context(|| format!("loading var file {}", vf.display()))?;
                        vars.extend(loaded);
                    }
                    for (k, v) in cli.var.iter() {
                        vars.insert(k.clone(), hcl::Value::String(v.clone()));
                    }
                    let fs_loader = FsLoader;
                    let env = EnvVars {
                        vars,
                        locals: HashMap::new(),
                        modules: HashMap::new(),
                        each: None,
                    };
                    let cfg = load_config(&cli.input, &fs_loader, env.clone())
                        .with_context(|| format!("loading root HCL {}", cli.input.display()))?;
                    (dsn, backend, cfg)
                };
                let dsn = match backend {
                    TestBackendKind::Postgres => dsn
                        .or_else(|| std::env::var("DATABASE_URL").ok())
                        .ok_or_else(|| anyhow!("missing DSN: pass --dsn or set DATABASE_URL"))?,
                    #[cfg(feature = "pglite")]
                    TestBackendKind::Pglite => dsn.unwrap_or_else(|| "pglite".to_string()),
                };
                let runner: Box<dyn TestBackend> = match backend {
                    TestBackendKind::Postgres => {
                        Box::new(dbschema::test_runner::postgres::PostgresTestBackend)
                    }
                    #[cfg(feature = "pglite")]
                    TestBackendKind::Pglite => {
                        Box::new(dbschema::test_runner::pglite::PGliteTestBackend)
                    }
                };
                let only: Option<std::collections::HashSet<String>> = if names.is_empty() {
                    None
                } else {
                    Some(names.into_iter().collect())
                };
                let summary = runner.run(&config, &dsn, only.as_ref())?;
                for r in summary.results {
                    if r.passed {
                        info!("ok - {}", r.name);
                    } else {
                        error!("FAIL - {}: {}", r.name, r.message);
                    }
                }
                if summary.failed > 0 {
                    error!(
                        "Summary: {} passed, {} failed ({} total)",
                        summary.passed, summary.failed, summary.total
                    );
                    std::process::exit(1);
                } else {
                    info!(
                        "Summary: {} passed, {} failed ({} total)",
                        summary.passed, summary.failed, summary.total
                    );
                }
                print_outputs(&config.outputs);
            }
            #[cfg(feature = "pglite")]
            Commands::Pglite {} => {
                use std::io::{self, Write};
                let mut rt = dbschema::test_runner::pglite::PGliteRuntime::new()?;
                rt.startup()?;
                let stdin = io::stdin();
                let mut line = String::new();
                loop {
                    print!("pglite=> ");
                    io::stdout().flush()?;
                    line.clear();
                    if stdin.read_line(&mut line)? == 0 {
                        break;
                    }
                    let sql = line.trim();
                    if sql.eq_ignore_ascii_case("\\q") {
                        break;
                    }
                    match rt.simple_query(sql) {
                        Ok(msgs) => {
                            for m in msgs {
                                if let backend::Message::DataRow(row) = m {
                                    let buf = row.buffer();
                                    let mut fields = row.ranges();
                                    let mut out = Vec::new();
                                    while let Some(res) = fields.next()? {
                                        match res {
                                            Some(range) => {
                                                let val = &buf[range];
                                                out.push(String::from_utf8_lossy(val).to_string());
                                            }
                                            None => out.push("NULL".into()),
                                        }
                                    }
                                    println!("{}", out.join(" | "));
                                } else if let backend::Message::CommandComplete(c) = m {
                                    if let Ok(tag) = c.tag() {
                                        println!("{}", tag);
                                    } else {
                                        println!("<command complete>");
                                    }
                                }
                            }
                        }
                        Err(e) => eprintln!("error: {e}"),
                    }
                }
                rt.shutdown()?;
            }
        }
    }

    Ok(())
}

fn format_path(path: &Path) -> Result<()> {
    if path.is_file() {
        format_file(path)?;
    } else if path.is_dir() {
        for entry in WalkDir::new(path) {
            let entry = entry?;
            if entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("hcl"))
                    .unwrap_or(false)
            {
                format_file(entry.path())?;
            }
        }
    }
    Ok(())
}

fn format_file(path: &Path) -> Result<()> {
    let content =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let body = hcl::parse(&content).with_context(|| format!("parsing {}", path.display()))?;
    let formatted = hcl::format::to_string(&body)?;
    fs::write(path, formatted).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn run_target(dbschema_config: &DbschemaConfig, target: &TargetConfig, strict: bool) -> Result<()> {
    info!("Running target: {}", target.name);

    for (key, value) in &dbschema_config.settings.env {
        std::env::set_var(key, value);
    }

    let input_path = target
        .input
        .as_deref()
        .or(dbschema_config.settings.input.as_deref())
        .unwrap_or("main.hcl");

    let mut vars = HashMap::new();
    for var_file in &dbschema_config.settings.var_files {
        vars.extend(load_var_file(&PathBuf::from(var_file))?);
    }
    for var_file in &target.var_files {
        vars.extend(load_var_file(&PathBuf::from(var_file))?);
    }
    for (key, value) in &target.vars {
        vars.insert(key.clone(), toml_to_hcl(value)?);
    }

    let fs_loader = FsLoader;
    let env = EnvVars {
        vars,
        ..EnvVars::default()
    };
    let config = load_config(&PathBuf::from(input_path), &fs_loader, env.clone())
        .with_context(|| format!("loading root HCL from {}", input_path))?;

    let include_set = target.get_include_set();
    let exclude_set = target.get_exclude_set();

    let filtered = apply_filters(&config, &include_set, &exclude_set);

    validate(&filtered, strict)?;
    let artifact = dbschema::generate_with_backend(&target.backend, &filtered, strict)?;

    if let Some(output_path) = &target.output {
        let path = Path::new(output_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, artifact)?;
        info!("Wrote output to: {}", output_path);
    } else {
        print!("{}", artifact);
    }

    print_outputs(&filtered.outputs);

    Ok(())
}

fn toml_to_hcl(value: &toml::Value) -> Result<hcl::Value> {
    match value {
        toml::Value::String(s) => Ok(hcl::Value::String(s.clone())),
        toml::Value::Integer(i) => Ok(hcl::Value::Number(hcl::Number::from(*i))),
        toml::Value::Float(f) => Ok(hcl::Value::Number(
            hcl::Number::from_f64(*f).ok_or_else(|| anyhow!("invalid float"))?,
        )),
        toml::Value::Boolean(b) => Ok(hcl::Value::Bool(*b)),
        toml::Value::Array(arr) => {
            let mut values = Vec::new();
            for v in arr {
                values.push(toml_to_hcl(v)?);
            }
            Ok(hcl::Value::Array(values))
        }
        toml::Value::Table(map) => {
            let mut hcl_map = hcl::Map::new();
            for (k, v) in map {
                hcl_map.insert(k.clone(), toml_to_hcl(v)?);
            }
            Ok(hcl::Value::Object(hcl_map))
        }
        _ => Err(anyhow!("Unsupported toml value type")),
    }
}

fn write_artifact(out_dir: &Path, name: &str, ext: &str, contents: &str) -> Result<PathBuf> {
    fs::create_dir_all(out_dir)?;
    let ts = chrono::Local::now().format("%Y%m%d%H%M%S");
    let file = format!("{}_{}.{}", ts, sanitize_filename(name), ext);
    let path = out_dir.join(file);
    fs::write(&path, contents)?;
    Ok(path)
}

fn print_outputs(outputs: &[OutputSpec]) {
    for o in outputs {
        let val = match &o.value {
            hcl::Value::String(s) => s.clone(),
            hcl::Value::Number(n) => n.to_string(),
            hcl::Value::Bool(b) => b.to_string(),
            _ => serde_json::to_string(&o.value).unwrap_or_default(),
        };
        println!("{} = {}", o.name, val);
    }
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn load_var_file(path: &Path) -> Result<HashMap<String, hcl::Value>> {
    let content = fs::read_to_string(path)?;
    // Try HCL body, collect top-level attributes as strings
    let body: hcl::Body = hcl::from_str(&content)
        .or_else(|_| {
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
        .map_err(|e| anyhow!("failed to parse var file as HCL: {}", e))?;

    let mut out = HashMap::new();
    for attr in body.attributes() {
        let name = attr.key();
        let v = dbschema::frontend::expr_to_value(attr.expr(), &EnvVars::default())
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
    let pos = s.find('=').ok_or_else(|| anyhow!("expected key=value"))?;
    let key = s[..pos]
        .parse()
        .map_err(|e| anyhow!("failed to parse key: {}", e))?;
    let value = s[pos + 1..]
        .parse()
        .map_err(|e| anyhow!("failed to parse value: {}", e))?;
    Ok((key, value))
}

struct FsLoader;
impl Loader for FsLoader {
    fn load(&self, path: &Path) -> Result<String> {
        Ok(fs::read_to_string(path)?)
    }
}

fn cli_filter_sets(
    backend: &str,
    include: &[ResourceKind],
    exclude: &[ResourceKind],
) -> (HashSet<ResourceKind>, HashSet<ResourceKind>) {
    use ResourceKind as R;

    let mut include_set: HashSet<R> = if include.is_empty() {
        [
            R::Schemas,
            R::Enums,
            R::Tables,
            R::Views,
            R::Materialized,
            R::Functions,
            R::Triggers,
            R::Extensions,
            R::Policies,
            R::Tests,
        ]
        .into_iter()
        .collect()
    } else {
        include.iter().copied().collect()
    };

    let exclude_set: HashSet<R> = exclude.iter().copied().collect();

    for r in &exclude_set {
        include_set.remove(r);
    }

    // Prisma backend supports tables and enums only; enforce that regardless of flags unless explicitly excluded
    if backend.eq_ignore_ascii_case("prisma") {
        include_set = [R::Enums, R::Tables].into_iter().collect();
        for r in &exclude_set {
            include_set.remove(r);
        }
    }

    (include_set, exclude_set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_run_target() -> Result<()> {
        let dir = tempdir()?;
        let dbschema_toml_path = dir.path().join("dbschema.toml");
        let main_hcl_path = dir.path().join("main.hcl");
        let another_hcl_path = dir.path().join("another.hcl");
        let var_file_path = dir.path().join("vars.hcl");

        let dbschema_toml = r#"
[settings]
input = "main.hcl"
var_files = ["vars.hcl"]

[[targets]]
name = "json_all"
backend = "json"
output = "all.json"

[[targets]]
name = "json_tables"
backend = "json"
output = "tables.json"
include = ["tables"]

[[targets]]
name = "another_input"
backend = "json"
input = "another.hcl"
output = "another.json"
include = ["functions"]

[[targets]]
name = "with_vars"
backend = "json"
output = "with_vars.json"
vars = { table_name = "my_users_table" }
include = ["tables"]

[[targets]]
name = "with_alt_name"
backend = "json"
output = "with_alt_name.json"
include = ["tables"]
"#;
        fs::write(&dbschema_toml_path, dbschema_toml)?;

        let main_hcl = r#"
variable "table_name" { default = "users" }
table "users" {
    table_name = var.table_name
}
function "my_func" {
    returns = "trigger"
    language = "plpgsql"
    body = "BEGIN RETURN NEW; END;"
}
"#;
        fs::write(&main_hcl_path, main_hcl)?;

        let another_hcl = r#"
function "another_func" {
    returns = "trigger"
    language = "plpgsql"
    body = "BEGIN RETURN NEW; END;"
}
"#;
        fs::write(&another_hcl_path, another_hcl)?;

        let var_file = r#"
table_name = "from_file"
"#;
        fs::write(&var_file_path, var_file)?;

        let dbschema_config = config::load_config_from_path(&dbschema_toml_path)
            .with_context(|| "failed to load dbschema.toml")?
            .ok_or_else(|| anyhow!("dbschema.toml not found"))?;

        std::env::set_current_dir(dir.path())?;

        // Test target "json_all"
        let target_all = dbschema_config
            .targets
            .iter()
            .find(|t| t.name == "json_all")
            .unwrap();
        run_target(&dbschema_config, target_all, false)?;
        let output_all = fs::read_to_string("all.json")?;
        assert!(output_all.contains("users"));
        assert!(output_all.contains("my_func"));

        // Test target "json_tables"
        let target_tables = dbschema_config
            .targets
            .iter()
            .find(|t| t.name == "json_tables")
            .unwrap();
        run_target(&dbschema_config, target_tables, false)?;
        let output_tables = fs::read_to_string("tables.json")?;
        assert!(output_tables.contains("users"));
        assert!(!output_tables.contains("my_func"));

        // Test target "another_input"
        let target_another = dbschema_config
            .targets
            .iter()
            .find(|t| t.name == "another_input")
            .unwrap();
        run_target(&dbschema_config, target_another, false)?;
        let output_another = fs::read_to_string("another.json")?;
        assert!(output_another.contains("another_func"));
        assert!(!output_another.contains("my_func"));

        // Test target "with_vars"
        let target_vars = dbschema_config
            .targets
            .iter()
            .find(|t| t.name == "with_vars")
            .unwrap();
        run_target(&dbschema_config, target_vars, false)?;
        let output_vars = fs::read_to_string("with_vars.json")?;
        // The variable from the target should be used
        assert!(output_vars.contains("my_users_table"));

        // Test target "with_alt_name"
        let target_alt_name = dbschema_config
            .targets
            .iter()
            .find(|t| t.name == "with_alt_name")
            .unwrap();
        run_target(&dbschema_config, target_alt_name, false)?;
        let output_alt_name = fs::read_to_string("with_alt_name.json")?;
        assert!(output_alt_name.contains("from_file"));

        dir.close()?;
        Ok(())
    }
}
