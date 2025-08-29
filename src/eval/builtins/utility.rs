use hcl::eval::{FuncArgs, FuncDef, ParamType};
use hcl::Value;

/// Utility functions
pub fn create_coalesce_func() -> FuncDef {
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

pub fn create_join_func() -> FuncDef {
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

pub fn create_split_func() -> FuncDef {
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

#[cfg(test)]
mod tests {
    use super::*;
    use hcl::eval::{Context, Evaluate};

    fn create_test_context() -> Context<'static> {
        let mut ctx = Context::new();
        ctx.declare_func("coalesce", create_coalesce_func());
        ctx.declare_func("join", create_join_func());
        ctx.declare_func("split", create_split_func());
        ctx
    }

    #[test]
    fn test_coalesce_function() {
        let ctx = create_test_context();
        let expr_str = "coalesce(null, \"default\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("default"));
    }

    #[test]
    fn test_coalesce_first_non_null() {
        let ctx = create_test_context();
        let expr_str = "coalesce(null, null, \"first\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("first"));
    }

    #[test]
    fn test_join_function() {
        let ctx = create_test_context();
        let expr_str = "join(\", \", [\"a\", \"b\", \"c\"])";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body.attributes().find(|a| a.key() == "test").unwrap().expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("a, b, c"));
    }

    #[test]
    fn test_split_function() {
        let ctx = create_test_context();
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
    fn test_split_with_spaces() {
        let ctx = create_test_context();
        let expr_str = "split(\", \", \"a, b, c\")";
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
}