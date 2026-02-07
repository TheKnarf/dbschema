use anyhow::Result;
use clingo::{ShowType, SolveMode};
use postgres::{Client, Transaction};

use crate::ir::{Config, EqAssertSpec, InvariantSpec, ScenarioSpec, SnapshotAssertSpec};
use crate::test_runner::{TestResult, is_verbose};
use log::info;

/// Substitute `{1}`, `{2}`, ... in a SQL template with atom arguments.
fn substitute_args(template: &str, args: &[String]) -> String {
    let mut result = template.to_string();
    for (i, arg) in args.iter().enumerate() {
        result = result.replace(&format!("{{{}}}", i + 1), arg);
    }
    result
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

/// Run all scenarios in the config, returning test results.
pub fn run_scenarios(
    cfg: &Config,
    client: &mut Client,
) -> Result<Vec<TestResult>> {
    let mut all_results = Vec::new();

    for spec in &cfg.scenarios {
        let results = run_one_scenario(spec, &cfg.invariants, client)?;
        all_results.extend(results);
    }

    Ok(all_results)
}

fn run_one_scenario(
    spec: &ScenarioSpec,
    invariants: &[InvariantSpec],
    client: &mut Client,
) -> Result<Vec<TestResult>> {
    let mut results = Vec::new();

    // Build program with params (#10)
    let mut full_program = String::new();
    for (key, value) in &spec.params {
        full_program.push_str(&format!("#const {}={}.\n", key, value));
    }
    full_program.push_str(&spec.program);

    // Create clingo control and add the ASP program
    // Pass "0" to enumerate all models (by default clingo stops after the first)
    let mut ctl = clingo::control(vec!["0".into()])
        .map_err(|e| anyhow::anyhow!("clingo control creation failed: {:?}", e))?;

    ctl.add("base", &[], &full_program)
        .map_err(|e| anyhow::anyhow!("clingo add program failed: {:?}", e))?;

    let parts = vec![clingo::Part::new("base", vec![])
        .map_err(|e| anyhow::anyhow!("clingo part creation failed: {:?}", e))?];
    ctl.ground(&parts)
        .map_err(|e| anyhow::anyhow!("clingo ground failed: {:?}", e))?;

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
            client,
        );

        results.push(result);
    }

    solve_handle.close()
        .map_err(|e| anyhow::anyhow!("clingo close failed: {:?}", e))?;

    Ok(results)
}

fn execute_answer_set(
    test_name: &str,
    spec: &ScenarioSpec,
    invariants: &[InvariantSpec],
    symbols: &[clingo::Symbol],
    client: &mut Client,
) -> TestResult {
    let atoms_label = build_atoms_label(symbols);

    let run_result = (|| -> std::result::Result<String, String> {
        let mut tx = client.transaction().map_err(|e| format!("begin transaction: {}", e))?;

        // 1. Run setup SQL
        for s in &spec.setup {
            if is_verbose() {
                info!("-- setup: {}", s);
            }
            tx.batch_execute(s).map_err(|e| format!("setup failed: {}", e))?;
        }

        // 2. For each map block (in declaration order), find matching atoms,
        //    sort by order_by or alphabetically, execute SQL
        for map in &spec.maps {
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
                            // For identifiers like alice, bob — name() returns the atom name
                            a.name()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|_| a.to_string())
                        }
                    }).collect();
                    matching_args.push(arg_strings);
                }
            }

            // Sort by order_by index (#5) or alphabetically
            if let Some(order_idx) = map.order_by {
                let idx = if order_idx > 0 { order_idx - 1 } else { 0 }; // 1-based to 0-based
                matching_args.sort_by(|a, b| {
                    let a_val = a.get(idx).map(|s| s.as_str()).unwrap_or("");
                    let b_val = b.get(idx).map(|s| s.as_str()).unwrap_or("");
                    // Try numeric comparison first, fall back to string
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
                        if spec.expect_error {
                            // Expected error — pass immediately (#7)
                            let _ = tx.rollback();
                            return Ok("ok (expected error)".to_string());
                        }
                        return Err(format!("map '{}' SQL failed: {} [atoms: {}]", map.atom_name, e, atoms_label));
                    }
                }
            }
        }

        // 4. Run assert_eq / assert_snapshot (#8)
        run_scenario_assertions(&mut tx, &spec.assert_eq, &spec.assert_snapshot)
            .map_err(|e| format!("{} [atoms: {}]", e, atoms_label))?;

        // 5. Run scenario-scoped checks (#6)
        run_invariants(&mut tx, &spec.checks)
            .map_err(|e| format!("{} [atoms: {}]", e, atoms_label))?;

        // 6. Run global invariants
        run_invariants(&mut tx, invariants)
            .map_err(|e| format!("{} [atoms: {}]", e, atoms_label))?;

        // 7. Rollback
        let _ = tx.rollback();

        Ok("ok".to_string())
    })();

    // 8. Run teardown outside transaction (#11)
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
