use hcl::Value;
use hcl::eval::{FuncArgs, FuncDef, ParamType};
use sha2::{Digest, Sha256, Sha512};

/// Cryptographic functions
pub fn create_md5_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let input = args[0].as_str().unwrap();
            let digest = md5::compute(input.as_bytes());
            Ok(Value::from(format!("{:x}", digest)))
        })
}

pub fn create_sha256_func() -> FuncDef {
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

pub fn create_sha512_func() -> FuncDef {
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

#[cfg(test)]
mod tests {
    use super::*;
    use hcl::eval::{Context, Evaluate};

    fn create_test_context() -> Context<'static> {
        let mut ctx = Context::new();
        ctx.declare_func("md5", create_md5_func());
        ctx.declare_func("sha256", create_sha256_func());
        ctx.declare_func("sha512", create_sha512_func());
        ctx
    }

    #[test]
    fn test_md5_function() {
        let ctx = create_test_context();
        let expr_str = "md5(\"hello world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let result = expr.evaluate(&ctx).unwrap();
        // MD5 of "hello world" is "5eb63bbbe01eeed093cb22bb8f5acdc3"
        assert_eq!(result, Value::from("5eb63bbbe01eeed093cb22bb8f5acdc3"));
    }

    #[test]
    fn test_sha256_function() {
        let ctx = create_test_context();
        let expr_str = "sha256(\"hello world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let result = expr.evaluate(&ctx).unwrap();
        // SHA256 of "hello world" is "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        assert_eq!(
            result,
            Value::from("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9")
        );
    }

    #[test]
    fn test_sha512_function() {
        let ctx = create_test_context();
        let expr_str = "sha512(\"hello world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let result = expr.evaluate(&ctx).unwrap();
        // SHA512 of "hello world" is "309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f"
        assert_eq!(
            result,
            Value::from(
                "309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f"
            )
        );
    }

    #[test]
    fn test_crypto_functions_empty_string() {
        let ctx = create_test_context();

        // Test MD5 with empty string
        let expr_str = "md5(\"\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let result = expr.evaluate(&ctx).unwrap();
        // MD5 of empty string is "d41d8cd98f00b204e9800998ecf8427e"
        assert_eq!(result, Value::from("d41d8cd98f00b204e9800998ecf8427e"));

        // Test SHA256 with empty string
        let expr_str = "sha256(\"\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let result = expr.evaluate(&ctx).unwrap();
        // SHA256 of empty string is "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        assert_eq!(
            result,
            Value::from("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
    }
}
