use hcl::eval::{Context, FuncArgs, FuncDef, ParamType};
use hcl::Value;
use sha2::{Sha256, Sha512, Digest};
use base64::{Engine as _, engine::general_purpose};

/// Create a context with built-in functions
pub fn create_context() -> Context<'static> {
    let mut ctx = Context::new();

    // String functions
    ctx.declare_func("upper", create_upper_func());
    ctx.declare_func("lower", create_lower_func());
    ctx.declare_func("length", create_length_func());
    ctx.declare_func("substr", create_substr_func());
    ctx.declare_func("contains", create_contains_func());
    ctx.declare_func("startswith", create_startswith_func());
    ctx.declare_func("endswith", create_endswith_func());
    ctx.declare_func("trim", create_trim_func());
    ctx.declare_func("replace", create_replace_func());

    // Numeric functions
    ctx.declare_func("min", create_min_func());
    ctx.declare_func("max", create_max_func());
    ctx.declare_func("abs", create_abs_func());

    // Utility functions
    ctx.declare_func("coalesce", create_coalesce_func());
    ctx.declare_func("join", create_join_func());
    ctx.declare_func("split", create_split_func());

    // Cryptographic functions
    ctx.declare_func("md5", create_md5_func());
    ctx.declare_func("sha256", create_sha256_func());
    ctx.declare_func("sha512", create_sha512_func());

    // Base64 functions
    ctx.declare_func("base64encode", create_base64encode_func());
    ctx.declare_func("base64decode", create_base64decode_func());

    ctx
}

// String functions
fn create_upper_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            Ok(Value::from(args[0].as_str().unwrap().to_uppercase()))
        })
}

fn create_lower_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            Ok(Value::from(args[0].as_str().unwrap().to_lowercase()))
        })
}

fn create_length_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            Ok(Value::from(args[0].as_str().unwrap().len() as i64))
        })
}

fn create_substr_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::Number)
        .param(ParamType::Number)
        .build(|args: FuncArgs| {
            let s = args[0].as_str().unwrap();
            let start = args[1].as_i64().unwrap() as usize;
            let len = args[2].as_i64().unwrap() as usize;

            if start >= s.len() {
                Ok(Value::from(""))
            } else {
                let end = (start + len).min(s.len());
                Ok(Value::from(&s[start..end]))
            }
        })
}

fn create_contains_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let haystack = args[0].as_str().unwrap();
            let needle = args[1].as_str().unwrap();
            Ok(Value::from(haystack.contains(needle)))
        })
}

fn create_startswith_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let s = args[0].as_str().unwrap();
            let prefix = args[1].as_str().unwrap();
            Ok(Value::from(s.starts_with(prefix)))
        })
}

fn create_endswith_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let s = args[0].as_str().unwrap();
            let suffix = args[1].as_str().unwrap();
            Ok(Value::from(s.ends_with(suffix)))
        })
}

fn create_trim_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            Ok(Value::from(args[0].as_str().unwrap().trim()))
        })
}

fn create_replace_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let s = args[0].as_str().unwrap();
            let from = args[1].as_str().unwrap();
            let to = args[2].as_str().unwrap();
            Ok(Value::from(s.replace(from, to)))
        })
}

// Numeric functions
fn create_min_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Number)
        .param(ParamType::Number)
        .build(|args: FuncArgs| {
            let a = args[0].as_i64().unwrap();
            let b = args[1].as_i64().unwrap();
            Ok(Value::from(a.min(b)))
        })
}

fn create_max_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Number)
        .param(ParamType::Number)
        .build(|args: FuncArgs| {
            let a = args[0].as_i64().unwrap();
            let b = args[1].as_i64().unwrap();
            Ok(Value::from(a.max(b)))
        })
}

fn create_abs_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Number)
        .build(|args: FuncArgs| {
            let n = args[0].as_i64().unwrap();
            Ok(Value::from(n.abs()))
        })
}

// Utility functions
fn create_coalesce_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Any)
        .variadic_param(ParamType::Any)
        .build(|args: FuncArgs| {
            for i in 0..args.len() {
                if !args[i].is_null() {
                    return Ok(args[i].clone());
                }
            }
            Ok(Value::Null)
        })
}

fn create_join_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::Any) // Use Any for array-like values
        .build(|args: FuncArgs| {
            let separator = args[0].as_str().unwrap();
            let arr = args[1].as_array().unwrap();

            let strings: Vec<String> = arr
                .iter()
                .map(|v| v.to_string().trim_matches('"').to_string())
                .collect();

            Ok(Value::from(strings.join(separator)))
        })
}

fn create_split_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let separator = args[0].as_str().unwrap();
            let s = args[1].as_str().unwrap();

            let parts: Vec<Value> = s
                .split(separator)
                .map(|part| Value::from(part.trim()))
                .collect();

            Ok(Value::from(parts))
        })
}

// Cryptographic functions
fn create_md5_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let input = args[0].as_str().unwrap();
            let digest = md5::compute(input.as_bytes());
            Ok(Value::from(format!("{:x}", digest)))
        })
}

fn create_sha256_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let input = args[0].as_str().unwrap();
            let mut hasher = Sha256::new();
            hasher.update(input.as_bytes());
            let result = hasher.finalize();
            Ok(Value::from(format!("{:x}", result)))
        })
}

fn create_sha512_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let input = args[0].as_str().unwrap();
            let mut hasher = Sha512::new();
            hasher.update(input.as_bytes());
            let result = hasher.finalize();
            Ok(Value::from(format!("{:x}", result)))
        })
}

// Base64 functions
fn create_base64encode_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let input = args[0].as_str().unwrap();
            let encoded = general_purpose::STANDARD.encode(input.as_bytes());
            Ok(Value::from(encoded))
        })
}

fn create_base64decode_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let input = args[0].as_str().unwrap();
            match general_purpose::STANDARD.decode(input) {
                Ok(decoded_bytes) => {
                    match String::from_utf8(decoded_bytes) {
                        Ok(decoded_string) => Ok(Value::from(decoded_string)),
                        Err(_) => Err("Invalid UTF-8 in decoded data".to_string()),
                    }
                }
                Err(_) => Err("Invalid base64 string".to_string()),
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hcl::eval::Evaluate;

    #[test]
    fn test_upper_function() {
        let ctx = create_context();
        let expr_str = "upper(\"hello\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("HELLO"));
    }

    #[test]
    fn test_lower_function() {
        let ctx = create_context();
        let expr_str = "lower(\"HELLO\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("hello"));
    }

    #[test]
    fn test_length_function() {
        let ctx = create_context();
        let expr_str = "length(\"hello\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(5));
    }

    #[test]
    fn test_substr_function() {
        let ctx = create_context();
        let expr_str = "substr(\"hello world\", 6, 5)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("world"));
    }

    #[test]
    fn test_contains_function() {
        let ctx = create_context();
        let expr_str = "contains(\"hello world\", \"world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(true));
    }

    #[test]
    fn test_min_function() {
        let ctx = create_context();
        let expr_str = "min(10, 5)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(5));
    }

    #[test]
    fn test_max_function() {
        let ctx = create_context();
        let expr_str = "max(10, 5)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(10));
    }

    #[test]
    fn test_join_function() {
        let ctx = create_context();
        let expr_str = "join(\", \", [\"a\", \"b\", \"c\"])";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("a, b, c"));
    }

    #[test]
    fn test_split_function() {
        let ctx = create_context();
        let expr_str = "split(\",\", \"a,b,c\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        let result = expr.evaluate(&ctx).unwrap();
        let expected = Value::from(vec![
            Value::from("a"),
            Value::from("b"),
            Value::from("c"),
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_coalesce_function() {
        let ctx = create_context();
        let expr_str = "coalesce(null, \"default\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("default"));
    }

    #[test]
    fn test_md5_function() {
        let ctx = create_context();
        let expr_str = "md5(\"hello world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        let result = expr.evaluate(&ctx).unwrap();
        // MD5 of "hello world" is "5eb63bbbe01eeed093cb22bb8f5acdc3"
        assert_eq!(result, Value::from("5eb63bbbe01eeed093cb22bb8f5acdc3"));
    }

    #[test]
    fn test_sha256_function() {
        let ctx = create_context();
        let expr_str = "sha256(\"hello world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        let result = expr.evaluate(&ctx).unwrap();
        // SHA256 of "hello world" is "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        assert_eq!(result, Value::from("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"));
    }

    #[test]
    fn test_sha512_function() {
        let ctx = create_context();
        let expr_str = "sha512(\"hello world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        let result = expr.evaluate(&ctx).unwrap();
        // SHA512 of "hello world" is "309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f"
        assert_eq!(result, Value::from("309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f"));
    }

    #[test]
    fn test_base64encode_function() {
        let ctx = create_context();
        let expr_str = "base64encode(\"hello world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        let result = expr.evaluate(&ctx).unwrap();
        // Base64 of "hello world" is "aGVsbG8gd29ybGQ="
        assert_eq!(result, Value::from("aGVsbG8gd29ybGQ="));
    }

    #[test]
    fn test_base64decode_function() {
        let ctx = create_context();
        let expr_str = "base64decode(\"aGVsbG8gd29ybGQ=\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        let result = expr.evaluate(&ctx).unwrap();
        assert_eq!(result, Value::from("hello world"));
    }

    #[test]
    fn test_base64decode_invalid() {
        let ctx = create_context();
        let expr_str = "base64decode(\"invalid-base64!\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        let result = expr.evaluate(&ctx);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid base64"));
    }
}