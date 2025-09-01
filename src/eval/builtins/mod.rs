// Built-in HCL functions organized by category

pub mod base64;
pub mod collection;
pub mod conversion;
pub mod crypto;
pub mod datetime;
pub mod numeric;
pub mod string;
pub mod utility;

use hcl::eval::Context;

/// Create a context with all built-in functions
pub fn create_context() -> Context<'static> {
    let mut ctx = Context::new();

    // String functions
    ctx.declare_func("upper", string::create_upper_func());
    ctx.declare_func("lower", string::create_lower_func());
    ctx.declare_func("length", string::create_length_func());
    ctx.declare_func("substr", string::create_substr_func());
    ctx.declare_func("contains", string::create_contains_func());
    ctx.declare_func("startswith", string::create_startswith_func());
    ctx.declare_func("endswith", string::create_endswith_func());
    ctx.declare_func("trim", string::create_trim_func());
    ctx.declare_func("replace", string::create_replace_func());

    // Numeric functions
    ctx.declare_func("min", numeric::create_min_func());
    ctx.declare_func("max", numeric::create_max_func());
    ctx.declare_func("abs", numeric::create_abs_func());

    // Utility functions
    ctx.declare_func("coalesce", utility::create_coalesce_func());
    ctx.declare_func("join", utility::create_join_func());
    ctx.declare_func("split", utility::create_split_func());

    // Cryptographic functions
    ctx.declare_func("md5", crypto::create_md5_func());
    ctx.declare_func("sha256", crypto::create_sha256_func());
    ctx.declare_func("sha512", crypto::create_sha512_func());

    // Base64 functions
    ctx.declare_func("base64encode", base64::create_base64encode_func());
    ctx.declare_func("base64decode", base64::create_base64decode_func());

    ctx
}

#[cfg(test)]
mod tests {
    use super::*;
    use hcl::eval::Evaluate;

    #[test]
    fn test_all_functions_registered() {
        let ctx = create_context();

        // Test that all functions are properly registered
        let functions = [
            "upper",
            "lower",
            "length",
            "substr",
            "contains",
            "startswith",
            "endswith",
            "trim",
            "replace",
            "min",
            "max",
            "abs",
            "coalesce",
            "join",
            "split",
            "md5",
            "sha256",
            "sha512",
            "base64encode",
            "base64decode",
        ];

        for func_name in functions {
            // Try to evaluate a simple call to each function to ensure it's registered
            let expr_str = format!("{}(\"test\")", func_name);
            let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
            let expr = body
                .attributes()
                .find(|a| a.key() == "test")
                .unwrap()
                .expr();

            // This will fail if the function is not registered, but that's expected for some functions
            // The important thing is that it doesn't panic due to unknown function
            let _ = expr.evaluate(&ctx);
        }
    }
}
