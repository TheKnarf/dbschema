pub mod builtins;
pub mod core;
pub mod for_each;
pub mod resource_impls;

// Re-export commonly used functions for convenience
pub use core::{
    expr_to_string, expr_to_string_vec, expr_to_value, find_attr, get_attr_string, get_attr_bool,
    load_root_with_loader, resolve_module_path
};
pub use builtins::create_context;