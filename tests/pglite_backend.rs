#[cfg(feature = "pglite")]
use anyhow::Result;
#[cfg(feature = "pglite")]
use dbschema::frontend::env::EnvVars;
#[cfg(feature = "pglite")]
use dbschema::test_runner::{pglite::PGliteTestBackend, TestBackend};
#[cfg(feature = "pglite")]
use dbschema::{load_config, Loader};
#[cfg(feature = "pglite")]
use std::collections::HashMap;
#[cfg(feature = "pglite")]
use std::path::Path;

#[cfg(feature = "pglite")]
struct FsLoader;
#[cfg(feature = "pglite")]
impl Loader for FsLoader {
    fn load(&self, path: &Path) -> Result<String> {
        Ok(std::fs::read_to_string(path)?)
    }
}

#[cfg(feature = "pglite")]
#[test]
#[ignore]
fn pglite_backend_runs_test() -> Result<()> {
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    let dir = tempdir()?;
    let hcl_path = dir.path().join("main.hcl");
    let mut file = File::create(&hcl_path)?;
    writeln!(
        file,
        r#"test "simple" {{
  setup = ["CREATE TABLE foo(id int); INSERT INTO foo VALUES (1)"]
  assert = "SELECT count(*) = 1 FROM foo"
}}"#
    )?;

    let loader = FsLoader;
    let cfg = load_config(
        &hcl_path,
        &loader,
        EnvVars {
            vars: HashMap::new(),
            locals: HashMap::new(),
            modules: HashMap::new(),
            each: None,
            count: None,
        },
    )?;

    let backend = PGliteTestBackend;
    let summary = backend.run(&cfg, "pglite", None)?;
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 0);
    Ok(())
}
