use hcl::Value;
use hcl::eval::{FuncArgs, FuncDef, ParamType};

/// String manipulation functions
pub fn create_upper_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| Ok(Value::from(args[0].as_str().unwrap().to_uppercase())))
}

pub fn create_lower_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| Ok(Value::from(args[0].as_str().unwrap().to_lowercase())))
}

pub fn create_length_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| Ok(Value::from(args[0].as_str().unwrap().len() as i64)))
}

pub fn create_substr_func() -> FuncDef {
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

pub fn create_contains_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let haystack = args[0].as_str().unwrap();
            let needle = args[1].as_str().unwrap();
            Ok(Value::from(haystack.contains(needle)))
        })
}

pub fn create_startswith_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let s = args[0].as_str().unwrap();
            let prefix = args[1].as_str().unwrap();
            Ok(Value::from(s.starts_with(prefix)))
        })
}

pub fn create_endswith_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let s = args[0].as_str().unwrap();
            let suffix = args[1].as_str().unwrap();
            Ok(Value::from(s.ends_with(suffix)))
        })
}

pub fn create_trim_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| Ok(Value::from(args[0].as_str().unwrap().trim())))
}

pub fn create_trimspace_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .build(|args: FuncArgs| Ok(Value::from(args[0].as_str().unwrap().trim())))
}

pub fn create_replace_func() -> FuncDef {
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

#[cfg(test)]
mod tests {
    use super::*;
    use hcl::eval::{Context, Evaluate};

    fn create_test_context() -> Context<'static> {
        let mut ctx = Context::new();
        ctx.declare_func("upper", create_upper_func());
        ctx.declare_func("lower", create_lower_func());
        ctx.declare_func("length", create_length_func());
        ctx.declare_func("substr", create_substr_func());
        ctx.declare_func("contains", create_contains_func());
        ctx.declare_func("startswith", create_startswith_func());
        ctx.declare_func("endswith", create_endswith_func());
        ctx.declare_func("trim", create_trim_func());
        ctx.declare_func("trimspace", create_trimspace_func());
        ctx.declare_func("replace", create_replace_func());
        ctx
    }

    #[test]
    fn test_upper_function() {
        let ctx = create_test_context();
        let expr_str = "upper(\"hello\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("HELLO"));
    }

    #[test]
    fn test_lower_function() {
        let ctx = create_test_context();
        let expr_str = "lower(\"HELLO\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("hello"));
    }

    #[test]
    fn test_trimspace_function() {
        let ctx = create_test_context();
        let expr_str = "trimspace(\"  hello  \")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("hello"));
    }

    #[test]
    fn test_length_function() {
        let ctx = create_test_context();
        let expr_str = "length(\"hello\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(5));
    }

    #[test]
    fn test_substr_function() {
        let ctx = create_test_context();
        let expr_str = "substr(\"hello world\", 6, 5)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("world"));
    }

    #[test]
    fn test_contains_function() {
        let ctx = create_test_context();
        let expr_str = "contains(\"hello world\", \"world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(true));
    }

    #[test]
    fn test_startswith_function() {
        let ctx = create_test_context();
        let expr_str = "startswith(\"hello world\", \"hello\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(true));
    }

    #[test]
    fn test_endswith_function() {
        let ctx = create_test_context();
        let expr_str = "endswith(\"hello world\", \"world\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(true));
    }

    #[test]
    fn test_trim_function() {
        let ctx = create_test_context();
        let expr_str = "trim(\"  hello  \")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("hello"));
    }

    #[test]
    fn test_replace_function() {
        let ctx = create_test_context();
        let expr_str = "replace(\"hello world\", \"world\", \"universe\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("hello universe"));
    }
}
