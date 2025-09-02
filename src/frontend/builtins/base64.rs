use base64::{engine::general_purpose, Engine as _};
use hcl::eval::{FuncArgs, FuncDef, ParamType};
use hcl::Value;

/// Base64 encoding/decoding functions
pub fn create_base64encode_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let input = args[0].as_str().unwrap();
            let encoded = general_purpose::STANDARD.encode(input.as_bytes());
            Ok(Value::from(encoded))
        })
}

pub fn create_base64decode_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let input = args[0].as_str().unwrap();
            match general_purpose::STANDARD.decode(input) {
                Ok(decoded_bytes) => match String::from_utf8(decoded_bytes) {
                    Ok(decoded_string) => Ok(Value::from(decoded_string)),
                    Err(_) => Err("Invalid UTF-8 in decoded data".to_string()),
                },
                Err(_) => Err("Invalid base64 string".to_string()),
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hcl::eval::{Context, Evaluate};

    fn create_test_context() -> Context<'static> {
        let mut ctx = Context::new();
        ctx.declare_func("base64encode", create_base64encode_func());
        ctx.declare_func("base64decode", create_base64decode_func());
        ctx
    }

    #[test]
    fn test_base64encode_function() {
        let ctx = create_test_context();
        let expr_str = "base64encode(\"hello world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let result = expr.evaluate(&ctx).unwrap();
        // Base64 of "hello world" is "aGVsbG8gd29ybGQ="
        assert_eq!(result, Value::from("aGVsbG8gd29ybGQ="));
    }

    #[test]
    fn test_base64decode_function() {
        let ctx = create_test_context();
        let expr_str = "base64decode(\"aGVsbG8gd29ybGQ=\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let result = expr.evaluate(&ctx).unwrap();
        assert_eq!(result, Value::from("hello world"));
    }

    #[test]
    fn test_base64_roundtrip() {
        let ctx = create_test_context();
        let original = "test data with spaces and symbols!@#$%^&*()";

        // First encode
        let encode_expr = format!("base64encode(\"{}\")", original);
        let body: hcl::Body = hcl::from_str(&format!("test = {}", encode_expr)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let encoded = expr.evaluate(&ctx).unwrap();
        let encoded_str = encoded.as_str().unwrap();

        // Then decode
        let decode_expr = format!("base64decode(\"{}\")", encoded_str);
        let body: hcl::Body = hcl::from_str(&format!("test = {}", decode_expr)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let decoded = expr.evaluate(&ctx).unwrap();

        assert_eq!(decoded, Value::from(original));
    }

    #[test]
    fn test_base64decode_invalid() {
        let ctx = create_test_context();
        let expr_str = "base64decode(\"invalid-base64!\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let result = expr.evaluate(&ctx);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid base64"));
    }

    #[test]
    fn test_base64encode_empty_string() {
        let ctx = create_test_context();
        let expr_str = "base64encode(\"\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let result = expr.evaluate(&ctx).unwrap();
        // Base64 of empty string is ""
        assert_eq!(result, Value::from(""));
    }

    #[test]
    fn test_base64decode_empty_string() {
        let ctx = create_test_context();
        let expr_str = "base64decode(\"\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let result = expr.evaluate(&ctx).unwrap();
        assert_eq!(result, Value::from(""));
    }
}
