use anyhow::Result;
use clingo::{ShowType, SolveMode};
use postgres::{Client, Transaction};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ir::{Config, EqAssertSpec, InvariantSpec, ScenarioMapSpec, ScenarioSpec, SnapshotAssertSpec};
use crate::test_runner::{TestResult, is_verbose};
use log::info;

/// Statistics collected from a scenario run.
pub struct ScenarioStats {
    pub scenario_name: String,
    pub seed: u32,
    pub models_tested: usize,
    pub passed: usize,
    pub failed: usize,
}

/// Substitute `{1}`, `{2}`, ... in a SQL template with atom arguments.
fn substitute_args(template: &str, args: &[String]) -> String {
    let mut result = template.to_string();
    for (i, arg) in args.iter().enumerate() {
        result = result.replace(&format!("{{{}}}", i + 1), arg);
    }
    result
}

/// Generate a random seed from current time (no `rand` dependency needed).
fn generate_random_seed() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_nanos() & 0xFFFF_FFFF) as u32)
        .unwrap_or(42)
}

/// Run assertions (invariants) against a transaction.
/// Returns `Ok(())` on success, or `Err(message)` on the first failure.
fn run_invariants(tx: &mut Transaction, invariants: &[InvariantSpec]) -> std::result::Result<(), String> {
    for inv in invariants {
        for a in &inv.asserts {
            if is_verbose() {
                info!("-- invariant '{}': {}", inv.name, a);
            }
            match tx.query(a.as_str(), &[]) {
                Ok(rows) => {
                    if rows.is_empty() {
                        return Err(format!("invariant '{}' returned no rows", inv.name));
                    }
                    let val: bool = match rows[0].try_get::<usize, bool>(0) {
                        Ok(v) => v,
                        Err(_) => {
                            match rows[0].try_get::<usize, i64>(0) {
                                Ok(v) => v != 0,
                                Err(_) => {
                                    match rows[0].try_get::<usize, i32>(0) {
                                        Ok(v) => v != 0,
                                        Err(_) => return Err(format!("invariant '{}': unsupported result type", inv.name)),
                                    }
                                }
                            }
                        }
                    };
                    if !val {
                        return Err(format!("invariant '{}' returned false", inv.name));
                    }
                }
                Err(e) => return Err(format!("invariant '{}' query error: {}", inv.name, e)),
            }
        }
    }
    Ok(())
}

/// Extract a column value as a string from a postgres Row.
fn extract_column_string(row: &postgres::Row, col: usize) -> std::result::Result<String, String> {
    if let Ok(v) = row.try_get::<usize, String>(col) {
        return Ok(v);
    }
    if let Ok(v) = row.try_get::<usize, i32>(col) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, i64>(col) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, bool>(col) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, f64>(col) {
        return Ok(v.to_string());
    }
    Err(format!("unsupported column type at index {}", col))
}

/// Run assert_eq and assert_snapshot assertions inside a transaction.
fn run_scenario_assertions(
    tx: &mut Transaction,
    assert_eqs: &[EqAssertSpec],
    assert_snapshots: &[SnapshotAssertSpec],
) -> std::result::Result<(), String> {
    for eq in assert_eqs {
        if is_verbose() {
            info!("-- assert_eq: {}", eq.query);
        }
        let rows = tx.query(eq.query.as_str(), &[])
            .map_err(|e| format!("assert_eq query error: {}", e))?;
        if rows.is_empty() {
            return Err(format!("assert_eq returned no rows for query: {}", eq.query));
        }
        let actual = extract_column_string(&rows[0], 0)?;
        if actual != eq.expected {
            return Err(format!(
                "assert_eq failed: expected '{}', got '{}' (query: {})",
                eq.expected, actual, eq.query
            ));
        }
    }

    for snap in assert_snapshots {
        if is_verbose() {
            info!("-- assert_snapshot: {}", snap.query);
        }
        let rows = tx.query(snap.query.as_str(), &[])
            .map_err(|e| format!("assert_snapshot query error: {}", e))?;
        if rows.len() != snap.rows.len() {
            return Err(format!(
                "assert_snapshot row count mismatch: expected {}, got {} (query: {})",
                snap.rows.len(), rows.len(), snap.query
            ));
        }
        for (ri, (actual_row, expected_row)) in rows.iter().zip(snap.rows.iter()).enumerate() {
            let num_cols = expected_row.len();
            for ci in 0..num_cols {
                let actual_val = extract_column_string(actual_row, ci)?;
                if actual_val != expected_row[ci] {
                    return Err(format!(
                        "assert_snapshot mismatch at row {} col {}: expected '{}', got '{}' (query: {})",
                        ri, ci, expected_row[ci], actual_val, snap.query
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Build a label string from symbols for better test output.
fn build_atoms_label(symbols: &[clingo::Symbol]) -> String {
    let atom_strs: Vec<String> = symbols.iter().map(|s| s.to_string()).collect();
    if atom_strs.len() <= 5 {
        atom_strs.join(", ")
    } else {
        let first_five = atom_strs[..5].join(", ");
        format!("{}... ({} total)", first_five, atom_strs.len())
    }
}

/// Run all scenarios in the config, returning test results and statistics.
pub fn run_scenarios(
    cfg: &Config,
    client: &mut Client,
) -> Result<(Vec<TestResult>, Vec<ScenarioStats>)> {
    let mut all_results = Vec::new();
    let mut all_stats = Vec::new();

    for spec in &cfg.scenarios {
        let (results, stats) = run_one_scenario(spec, &cfg.invariants, client)?;
        all_results.extend(results);
        all_stats.push(stats);
    }

    Ok((all_results, all_stats))
}

fn run_one_scenario(
    spec: &ScenarioSpec,
    invariants: &[InvariantSpec],
    client: &mut Client,
) -> Result<(Vec<TestResult>, ScenarioStats)> {
    let mut results = Vec::new();

    // Determine seed: use explicit or generate random
    let seed = spec.seed.unwrap_or_else(generate_random_seed);

    if is_verbose() {
        info!("-- scenario '{}': seed={}", spec.name, seed);
    }

    // Build program with params (#10)
    let mut full_program = String::new();
    for (key, value) in &spec.params {
        full_program.push_str(&format!("#const {}={}.\n", key, value));
    }
    full_program.push_str(&spec.program);

    // Create clingo control and add the ASP program
    // Pass "0" to enumerate all models, plus --seed for reproducibility
    let mut ctl = clingo::control(vec!["0".into(), format!("--seed={}", seed)])
        .map_err(|e| anyhow::anyhow!("clingo control creation failed: {:?}", e))?;

    ctl.add("base", &[], &full_program)
        .map_err(|e| anyhow::anyhow!("clingo add program failed: {:?}", e))?;

    if let Some(num_steps) = spec.steps {
        // Multi-step: ground base + all step parts together
        let mut parts = vec![clingo::Part::new("base", vec![])
            .map_err(|e| anyhow::anyhow!("clingo part creation failed: {:?}", e))?];
        for t in 1..=num_steps {
            parts.push(clingo::Part::new("step", vec![clingo::Symbol::create_number(t as i32)])
                .map_err(|e| anyhow::anyhow!("clingo step part creation failed: {:?}", e))?);
        }
        ctl.ground(&parts)
            .map_err(|e| anyhow::anyhow!("clingo ground failed: {:?}", e))?;
    } else {
        // Single-step: existing behavior
        let parts = vec![clingo::Part::new("base", vec![])
            .map_err(|e| anyhow::anyhow!("clingo part creation failed: {:?}", e))?];
        ctl.ground(&parts)
            .map_err(|e| anyhow::anyhow!("clingo ground failed: {:?}", e))?;
    }

    // Solve and collect answer sets
    let mut solve_handle = ctl.solve(SolveMode::YIELD, &[])
        .map_err(|e| anyhow::anyhow!("clingo solve failed: {:?}", e))?;

    let mut answer_set_index = 0usize;
    let max_runs = if spec.runs == 0 { usize::MAX } else { spec.runs };

    loop {
        if answer_set_index >= max_runs {
            break;
        }

        solve_handle.resume()
            .map_err(|e| anyhow::anyhow!("clingo resume failed: {:?}", e))?;

        let model = match solve_handle.model() {
            Ok(Some(model)) => model,
            Ok(None) => break,
            Err(e) => return Err(anyhow::anyhow!("clingo model failed: {:?}", e)),
        };

        // Extract atoms from the model
        let symbols = model.symbols(ShowType::SHOWN)
            .map_err(|e| anyhow::anyhow!("clingo symbols failed: {:?}", e))?;

        // Build label from atoms (#9)
        let atoms_label = build_atoms_label(&symbols);
        let test_name = format!("{}[{}: {}]", spec.name, answer_set_index, atoms_label);
        answer_set_index += 1;

        if is_verbose() {
            info!("-- scenario '{}': atoms = [{}]", test_name, atoms_label);
        }

        // Execute this answer set as a test
        let result = execute_answer_set(
            &test_name,
            spec,
            invariants,
            &symbols,
            seed,
            client,
        );

        results.push(result);
    }

    solve_handle.close()
        .map_err(|e| anyhow::anyhow!("clingo close failed: {:?}", e))?;

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.len() - passed;

    let stats = ScenarioStats {
        scenario_name: spec.name.clone(),
        seed,
        models_tested: results.len(),
        passed,
        failed,
    };

    Ok((results, stats))
}

/// Execute map blocks against a transaction, matching atoms from the symbol set.
/// Returns Ok(Some("ok (expected error)")) if expect_error consumed an error,
/// Ok(None) on normal success, Err(msg) on unexpected failure.
fn execute_maps(
    tx: &mut Transaction,
    maps: &[ScenarioMapSpec],
    symbols: &[clingo::Symbol],
    expect_error: bool,
    atoms_label: &str,
    seed: u32,
) -> std::result::Result<Option<String>, String> {
    for map in maps {
        let mut matching_args: Vec<Vec<String>> = Vec::new();

        for sym in symbols {
            let sym_name = sym.name()
                .map_err(|e| format!("symbol name error: {:?}", e))?;
            if sym_name == map.atom_name {
                let args = sym.arguments()
                    .map_err(|e| format!("symbol arguments error: {:?}", e))?;
                let arg_strings: Vec<String> = args.iter().map(|a| {
                    if let Ok(n) = a.number() {
                        n.to_string()
                    } else if let Ok(s) = a.string() {
                        s.to_string()
                    } else {
                        a.name()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|_| a.to_string())
                    }
                }).collect();
                matching_args.push(arg_strings);
            }
        }

        // Sort by order_by index or alphabetically
        if let Some(order_idx) = map.order_by {
            let idx = if order_idx > 0 { order_idx - 1 } else { 0 };
            matching_args.sort_by(|a, b| {
                let a_val = a.get(idx).map(|s| s.as_str()).unwrap_or("");
                let b_val = b.get(idx).map(|s| s.as_str()).unwrap_or("");
                match (a_val.parse::<f64>(), b_val.parse::<f64>()) {
                    (Ok(an), Ok(bn)) => an.partial_cmp(&bn).unwrap_or(std::cmp::Ordering::Equal),
                    _ => a_val.cmp(b_val),
                }
            });
        } else {
            matching_args.sort();
        }

        for args in &matching_args {
            let sql = substitute_args(&map.sql, args);
            if is_verbose() {
                info!("-- map '{}': {}", map.atom_name, sql);
            }
            match tx.batch_execute(&sql) {
                Ok(()) => {}
                Err(e) => {
                    if expect_error {
                        return Ok(Some("ok (expected error)".to_string()));
                    }
                    return Err(format!("map '{}' SQL failed: {} [atoms: {}] (seed: {})", map.atom_name, e, atoms_label, seed));
                }
            }
        }
    }
    Ok(None)
}

fn execute_answer_set(
    test_name: &str,
    spec: &ScenarioSpec,
    invariants: &[InvariantSpec],
    symbols: &[clingo::Symbol],
    seed: u32,
    client: &mut Client,
) -> TestResult {
    let atoms_label = build_atoms_label(symbols);

    let run_result = if !spec.step_blocks.is_empty() {
        // Multi-step path: each step gets its own committed transaction
        execute_multi_step(spec, invariants, symbols, seed, client, &atoms_label)
    } else {
        // Single-step path: single transaction, rollback
        execute_single_step(spec, invariants, symbols, seed, client, &atoms_label)
    };

    // Run teardown outside transaction
    for s in &spec.teardown {
        if is_verbose() {
            info!("-- teardown: {}", s);
        }
        let _ = client.batch_execute(s);
    }

    match run_result {
        Ok(msg) => TestResult {
            name: test_name.to_string(),
            passed: true,
            message: msg,
        },
        Err(msg) => TestResult {
            name: test_name.to_string(),
            passed: false,
            message: msg,
        },
    }
}

fn execute_single_step(
    spec: &ScenarioSpec,
    invariants: &[InvariantSpec],
    symbols: &[clingo::Symbol],
    seed: u32,
    client: &mut Client,
    atoms_label: &str,
) -> std::result::Result<String, String> {
    let mut tx = client.transaction().map_err(|e| format!("begin transaction: {}", e))?;

    // 1. Run setup SQL
    for s in &spec.setup {
        if is_verbose() {
            info!("-- setup: {}", s);
        }
        tx.batch_execute(s).map_err(|e| format!("setup failed: {}", e))?;
    }

    // 2. Execute maps
    if let Some(msg) = execute_maps(&mut tx, &spec.maps, symbols, spec.expect_error, atoms_label, seed)? {
        let _ = tx.rollback();
        return Ok(msg);
    }

    // 3. Run assert_eq / assert_snapshot
    run_scenario_assertions(&mut tx, &spec.assert_eq, &spec.assert_snapshot)
        .map_err(|e| format!("{} [atoms: {}] (seed: {})", e, atoms_label, seed))?;

    // 4. Run scenario-scoped checks
    run_invariants(&mut tx, &spec.checks)
        .map_err(|e| format!("{} [atoms: {}] (seed: {})", e, atoms_label, seed))?;

    // 5. Run global invariants
    run_invariants(&mut tx, invariants)
        .map_err(|e| format!("{} [atoms: {}] (seed: {})", e, atoms_label, seed))?;

    // 6. Rollback
    let _ = tx.rollback();

    Ok("ok".to_string())
}

fn execute_multi_step(
    spec: &ScenarioSpec,
    invariants: &[InvariantSpec],
    symbols: &[clingo::Symbol],
    seed: u32,
    client: &mut Client,
    atoms_label: &str,
) -> std::result::Result<String, String> {
    // Base transaction: top-level setup + top-level maps → commit
    {
        let mut tx = client.transaction().map_err(|e| format!("begin base transaction: {}", e))?;
        for s in &spec.setup {
            if is_verbose() {
                info!("-- base setup: {}", s);
            }
            tx.batch_execute(s).map_err(|e| format!("base setup failed: {}", e))?;
        }

        if let Some(msg) = execute_maps(&mut tx, &spec.maps, symbols, spec.expect_error, atoms_label, seed)? {
            let _ = tx.rollback();
            return Ok(msg);
        }

        tx.commit().map_err(|e| format!("base commit failed: {}", e))?;
    }

    // Per-step transactions
    for (step_idx, step) in spec.step_blocks.iter().enumerate() {
        let mut tx = client.transaction().map_err(|e| format!("begin step {} transaction: {}", step_idx + 1, e))?;

        // Step setup SQL
        for s in &step.setup {
            if is_verbose() {
                info!("-- step {} setup: {}", step_idx + 1, s);
            }
            tx.batch_execute(s).map_err(|e| format!("step {} setup failed: {}", step_idx + 1, e))?;
        }

        // Step maps
        if let Some(msg) = execute_maps(&mut tx, &step.maps, symbols, spec.expect_error, atoms_label, seed)? {
            let _ = tx.rollback();
            return Ok(msg);
        }

        // Step assertions
        run_scenario_assertions(&mut tx, &step.assert_eq, &step.assert_snapshot)
            .map_err(|e| format!("step {}: {} [atoms: {}] (seed: {})", step_idx + 1, e, atoms_label, seed))?;

        // Step checks
        run_invariants(&mut tx, &step.checks)
            .map_err(|e| format!("step {}: {} [atoms: {}] (seed: {})", step_idx + 1, e, atoms_label, seed))?;

        tx.commit().map_err(|e| format!("step {} commit failed: {}", step_idx + 1, e))?;
    }

    // Final verification transaction: top-level assertions + checks + global invariants → rollback
    {
        let mut tx = client.transaction().map_err(|e| format!("begin final verification transaction: {}", e))?;

        run_scenario_assertions(&mut tx, &spec.assert_eq, &spec.assert_snapshot)
            .map_err(|e| format!("{} [atoms: {}] (seed: {})", e, atoms_label, seed))?;

        run_invariants(&mut tx, &spec.checks)
            .map_err(|e| format!("{} [atoms: {}] (seed: {})", e, atoms_label, seed))?;

        run_invariants(&mut tx, invariants)
            .map_err(|e| format!("{} [atoms: {}] (seed: {})", e, atoms_label, seed))?;

        let _ = tx.rollback();
    }

    Ok("ok".to_string())
}
