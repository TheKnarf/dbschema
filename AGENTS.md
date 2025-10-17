- Always run `cargo check 2>&1 | cat` after every change

- Delete unused imports, variables and functions afterwards. We want to leave a clean cargo check.

- run `cargo test` in the end to ensure tests still work
