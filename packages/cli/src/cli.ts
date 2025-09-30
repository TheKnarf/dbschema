#!/usr/bin/env node

import { Command } from 'commander';
import { readFileSync, readdirSync, statSync, writeFileSync, mkdirSync } from 'fs';
import { join, resolve, dirname } from 'path';
import chalk from 'chalk';
import { validate_hcl, generate, format_hcl, version, ValidateOptions, GenerateOptions } from 'dbschema';

const program = new Command();

// Create a file loader that resolves paths relative to the current working directory
function createFileLoader() {
  const cwd = process.cwd();

  return function loadFile(path: string): string {
    console.error(`[DEBUG] loadFile called with path: ${JSON.stringify(path)}`);

    if (!path || path.trim() === '') {
      throw new Error(`File path is empty or undefined (received: ${JSON.stringify(path)})`);
    }

    // Resolve relative to current working directory
    const resolvedPath = resolve(cwd, path);
    console.error(`[DEBUG] Resolved to: ${resolvedPath}`);

    try {
      const content = readFileSync(resolvedPath, 'utf-8');
      console.error(`[DEBUG] Successfully loaded ${resolvedPath} (${content.length} bytes)`);
      return content;
    } catch (error) {
      throw new Error(`Failed to load file ${resolvedPath}: ${error}`);
    }
  };
}

// Recursively format HCL files
function formatPath(path: string): number {
  let count = 0;
  const stat = statSync(path);

  if (stat.isFile() && path.endsWith('.hcl')) {
    try {
      const content = readFileSync(path, 'utf-8');
      const formatted = format_hcl(content);
      writeFileSync(path, formatted, 'utf-8');
      console.log(chalk.green(`✓ Formatted ${path}`));
      count++;
    } catch (error) {
      console.error(chalk.red(`✗ Failed to format ${path}: ${error}`));
    }
  } else if (stat.isDirectory()) {
    const entries = readdirSync(path);
    for (const entry of entries) {
      count += formatPath(join(path, entry));
    }
  }

  return count;
}

program
  .name('dbschema')
  .description('HCL-driven database schema tool (Node.js edition)')
  .version(version());

program
  .command('validate')
  .description('Validate HCL and print a summary')
  .option('--input <file>', 'Root HCL file', 'main.hcl')
  .option('--strict', 'Enable strict mode (errors on undefined enums)', false)
  .option('--include <resources...>', 'Include only these resources')
  .option('--exclude <resources...>', 'Exclude these resources')
  .action((options) => {
    try {
      const opts = new ValidateOptions();
      opts.strict = options.strict;

      if (options.include) {
        opts.include_resources = options.include;
      }
      if (options.exclude) {
        opts.exclude_resources = options.exclude;
      }

      // Resolve input path to absolute before passing to WASM
      const absoluteInputPath = resolve(options.input);
      const loadFile = createFileLoader();
      console.error(`[DEBUG] Calling validate_hcl with:`);
      console.error(`  absoluteInputPath: ${absoluteInputPath}`);
      console.error(`  loadFile type: ${typeof loadFile}`);
      console.error(`  opts: ${JSON.stringify(opts)}`);
      const result = validate_hcl(absoluteInputPath, loadFile, opts);

      if (result.success) {
        console.log(chalk.green('✓'), result.summary);
        process.exit(0);
      } else {
        console.error(chalk.red('✗'), result.error);
        process.exit(1);
      }
    } catch (error) {
      console.error(chalk.red('Error:'), error);
      process.exit(1);
    }
  });

program
  .command('fmt')
  .description('Format HCL files in place')
  .argument('[paths...]', 'Files or directories to format', ['.'])
  .action((paths: string[]) => {
    try {
      let totalFormatted = 0;

      for (const path of paths) {
        const resolvedPath = resolve(path);
        totalFormatted += formatPath(resolvedPath);
      }

      console.log(chalk.green(`\n✓ Formatted ${totalFormatted} file(s)`));
      process.exit(0);
    } catch (error) {
      console.error(chalk.red('Error:'), error);
      process.exit(1);
    }
  });

program
  .command('create-migration')
  .description('Create a SQL migration file from the HCL')
  .option('--input <file>', 'Root HCL file', 'main.hcl')
  .option('--backend <backend>', 'Backend to use (postgres, prisma, json)', 'postgres')
  .option('--out-dir <dir>', 'Output directory for migration files')
  .option('--name <name>', 'Migration name')
  .option('--strict', 'Enable strict mode', false)
  .option('--include <resources...>', 'Include only these resources')
  .option('--exclude <resources...>', 'Exclude these resources')
  .action((options) => {
    try {
      const opts = new GenerateOptions(options.backend);
      opts.strict = options.strict;

      if (options.include) {
        opts.include_resources = options.include;
      }
      if (options.exclude) {
        opts.exclude_resources = options.exclude;
      }

      // Resolve input path to absolute before passing to WASM
      const absoluteInputPath = resolve(options.input);
      const loadFile = createFileLoader();
      const output = generate(absoluteInputPath, loadFile, opts);

      if (options.outDir) {
        // Create output directory if it doesn't exist
        mkdirSync(options.outDir, { recursive: true });

        // Generate filename with timestamp
        const timestamp = new Date().toISOString().replace(/[-:T]/g, '').slice(0, 14);
        const name = options.name || 'migration';
        const extension = options.backend === 'postgres' ? 'sql' :
                         options.backend === 'prisma' ? 'prisma' : 'json';
        const filename = `${timestamp}_${name}.${extension}`;
        const filepath = join(options.outDir, filename);

        writeFileSync(filepath, output, 'utf-8');
        console.log(chalk.green('✓'), `Wrote migration: ${filepath}`);
      } else {
        // Print to stdout
        console.log(output);
      }

      process.exit(0);
    } catch (error) {
      console.error(chalk.red('Error:'), error);
      process.exit(1);
    }
  });

// Note: 'test' command is not available in WASM version (requires postgres connection)
program
  .command('test')
  .description('Run tests (not available in Node.js CLI - use native Rust CLI)')
  .action(() => {
    console.error(chalk.yellow('⚠'), 'The test command requires the native Rust CLI.');
    console.error(chalk.yellow('  Install with: cargo install dbschema'));
    console.error(chalk.yellow('  Reason: requires PostgreSQL connection (not available in WASM)'));
    process.exit(1);
  });

program
  .command('lint')
  .description('Lint schema (partial support - sql-syntax check not available)')
  .action(() => {
    console.error(chalk.yellow('⚠'), 'The lint command is not fully supported in Node.js CLI.');
    console.error(chalk.yellow('  Some lint checks require the native Rust CLI.'));
    console.error(chalk.yellow('  Install with: cargo install dbschema'));
    console.error(chalk.yellow('  Missing: sql-syntax validation (requires pg_query)'));
    process.exit(1);
  });

program.parse();