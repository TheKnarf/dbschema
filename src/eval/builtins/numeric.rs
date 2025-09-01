use hcl::eval::{FuncArgs, FuncDef, ParamType};
use hcl::Value;

/// Numeric functions
pub fn create_min_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Number)
        .param(ParamType::Number)
        .build(|args: FuncArgs| {
            let a = args[0].as_i64().unwrap();
            let b = args[1].as_i64().unwrap();
            Ok(Value::from(a.min(b)))
        })
}

pub fn create_max_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Number)
        .param(ParamType::Number)
        .build(|args: FuncArgs| {
            let a = args[0].as_i64().unwrap();
            let b = args[1].as_i64().unwrap();
            Ok(Value::from(a.max(b)))
        })
}

pub fn create_abs_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Number)
        .build(|args: FuncArgs| {
            let n = args[0].as_i64().unwrap();
            Ok(Value::from(n.abs()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hcl::eval::{Context, Evaluate};

    fn create_test_context() -> Context<'static> {
        let mut ctx = Context::new();
        ctx.declare_func("min", create_min_func());
        ctx.declare_func("max", create_max_func());
        ctx.declare_func("abs", create_abs_func());
        ctx
    }

    #[test]
    fn test_min_function() {
        let ctx = create_test_context();
        let expr_str = "min(10, 5)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(5));
    }

    #[test]
    fn test_max_function() {
        let ctx = create_test_context();
        let expr_str = "max(10, 5)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(10));
    }

    #[test]
    fn test_abs_function() {
        let ctx = create_test_context();
        let expr_str = "abs(-5)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(5));
    }

    #[test]
    fn test_abs_positive_function() {
        let ctx = create_test_context();
        let expr_str = "abs(5)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(5));
    }
}
