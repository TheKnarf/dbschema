use hcl::eval::{FuncArgs, FuncDef, ParamType};
use hcl::Value;

/// Convert a value to a string
pub fn create_tostring_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Any)
        .build(|args: FuncArgs| Ok(Value::from(args[0].to_string())))
}

/// Convert a value to a number
pub fn create_tonumber_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Any)
        .build(|args: FuncArgs| {
            let v = &args[0];
            if let Some(n) = v.as_i64() {
                Ok(Value::from(n))
            } else if let Some(s) = v.as_str() {
                s.parse::<i64>()
                    .map(Value::from)
                    .map_err(|_| "invalid number".to_string())
            } else if let Some(b) = v.as_bool() {
                Ok(Value::from(if b { 1 } else { 0 }))
            } else {
                Err("cannot convert to number".to_string())
            }
        })
}

/// Convert a value to a boolean
pub fn create_tobool_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Any)
        .build(|args: FuncArgs| {
            let v = &args[0];
            if let Some(b) = v.as_bool() {
                Ok(Value::from(b))
            } else if let Some(s) = v.as_str() {
                match s.parse::<bool>() {
                    Ok(b) => Ok(Value::from(b)),
                    Err(_) => Err("invalid bool".to_string()),
                }
            } else if let Some(n) = v.as_i64() {
                Ok(Value::from(n != 0))
            } else {
                Err("cannot convert to bool".to_string())
            }
        })
}

/// Convert a value to a list
pub fn create_tolist_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Any)
        .build(|args: FuncArgs| {
            let v = &args[0];
            if let Some(arr) = v.as_array() {
                Ok(Value::from(arr.clone()))
            } else {
                Ok(Value::from(vec![v.clone()]))
            }
        })
}

/// Convert a value to a map
pub fn create_tomap_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::Any)
        .build(|args: FuncArgs| {
            let v = &args[0];
            if let Some(obj) = v.as_object() {
                Ok(Value::from(obj.clone()))
            } else {
                Err("cannot convert to map".to_string())
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hcl::eval::{Context, Evaluate};

    fn create_test_context() -> Context<'static> {
        let mut ctx = Context::new();
        ctx.declare_func("tostring", create_tostring_func());
        ctx.declare_func("tonumber", create_tonumber_func());
        ctx.declare_func("tobool", create_tobool_func());
        ctx.declare_func("tolist", create_tolist_func());
        ctx.declare_func("tomap", create_tomap_func());
        ctx
    }

    #[test]
    fn test_tostring_function() {
        let ctx = create_test_context();
        let expr_str = "tostring(123)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("123"));
    }

    #[test]
    fn test_tonumber_function() {
        let ctx = create_test_context();
        let expr_str = "tonumber(\"123\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(123));
    }

    #[test]
    fn test_tonumber_error() {
        let ctx = create_test_context();
        let expr_str = "tonumber(\"abc\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert!(expr.evaluate(&ctx).is_err());
    }

    #[test]
    fn test_tobool_function() {
        let ctx = create_test_context();
        let expr_str = "tobool(\"true\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(true));
    }

    #[test]
    fn test_tobool_error() {
        let ctx = create_test_context();
        let expr_str = "tobool(\"maybe\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert!(expr.evaluate(&ctx).is_err());
    }

    #[test]
    fn test_tolist_function() {
        let ctx = create_test_context();
        let expr_str = "tolist(1)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let expected = Value::from(vec![Value::from(1)]);
        assert_eq!(expr.evaluate(&ctx).unwrap(), expected);
    }

    #[test]
    fn test_tomap_function() {
        let ctx = create_test_context();
        let expr_str = "tomap({a = 1})";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let mut map = hcl::value::Map::new();
        map.insert("a".to_string(), Value::from(1));
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(map));
    }

    #[test]
    fn test_tomap_error() {
        let ctx = create_test_context();
        let expr_str = "tomap([1,2])";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert!(expr.evaluate(&ctx).is_err());
    }
}
