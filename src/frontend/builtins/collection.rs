use hcl::eval::{FuncArgs, FuncDef, ParamType};
use hcl::Value;

/// Concatenate multiple arrays into a single array
pub fn create_concat_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::array_of(ParamType::Any))
        .variadic_param(ParamType::array_of(ParamType::Any))
        .build(|args: FuncArgs| {
            let mut result: Vec<Value> = Vec::new();
            for arg in args.iter() {
                let arr = arg.as_array().unwrap();
                result.extend(arr.clone());
            }
            Ok(Value::from(result))
        })
}

/// Flatten a nested array into a single level array
pub fn create_flatten_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::array_of(ParamType::Any))
        .build(|args: FuncArgs| {
            fn flatten(values: &Vec<Value>, out: &mut Vec<Value>) {
                for v in values {
                    if let Some(arr) = v.as_array() {
                        flatten(arr, out);
                    } else {
                        out.push(v.clone());
                    }
                }
            }

            let arr = args[0].as_array().unwrap();
            let mut result = Vec::new();
            flatten(arr, &mut result);
            Ok(Value::from(result))
        })
}

/// Remove duplicate values from an array preserving the first occurrence
pub fn create_distinct_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::array_of(ParamType::Any))
        .build(|args: FuncArgs| {
            let arr = args[0].as_array().unwrap();
            let mut result: Vec<Value> = Vec::new();
            for v in arr.iter() {
                if !result.contains(v) {
                    result.push(v.clone());
                }
            }
            Ok(Value::from(result))
        })
}

/// Return a slice of the array from start (inclusive) to end (exclusive)
pub fn create_slice_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::array_of(ParamType::Any))
        .param(ParamType::Number)
        .param(ParamType::Number)
        .build(|args: FuncArgs| {
            let arr = args[0].as_array().unwrap();
            let len = arr.len() as i64;
            let mut start = args[1].as_i64().unwrap();
            let mut end = args[2].as_i64().unwrap();

            if start < 0 {
                start = len + start;
            }
            if end < 0 {
                end = len + end;
            }

            if start < 0 || end < 0 || start > len || end > len || start >= end {
                return Ok(Value::from(Vec::<Value>::new()));
            }

            let slice: Vec<Value> = arr[start as usize..end as usize].to_vec();
            Ok(Value::from(slice))
        })
}

/// Sort an array of strings or numbers
pub fn create_sort_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::array_of(ParamType::Any))
        .build(|args: FuncArgs| {
            let arr = args[0].as_array().unwrap();
            let mut result = arr.clone();
            result.sort_by(|a, b| {
                if let (Some(sa), Some(sb)) = (a.as_str(), b.as_str()) {
                    sa.cmp(sb)
                } else if let (Some(na), Some(nb)) = (a.as_i64(), b.as_i64()) {
                    na.cmp(&nb)
                } else {
                    a.to_string().cmp(&b.to_string())
                }
            });
            Ok(Value::from(result))
        })
}

/// Reverse the order of elements in an array
pub fn create_reverse_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::array_of(ParamType::Any))
        .build(|args: FuncArgs| {
            let arr = args[0].as_array().unwrap();
            let result: Vec<Value> = arr.iter().cloned().rev().collect();
            Ok(Value::from(result))
        })
}

/// Return the index of a value in an array, or error if not found
pub fn create_index_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::array_of(ParamType::Any))
        .param(ParamType::Any)
        .build(|args: FuncArgs| {
            let arr = args[0].as_array().unwrap();
            let value = &args[1];
            if let Some(pos) = arr.iter().position(|v| v == value) {
                Ok(Value::from(pos as i64))
            } else {
                Err("value not found".to_string())
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hcl::eval::{Context, Evaluate};

    fn create_test_context() -> Context<'static> {
        let mut ctx = Context::new();
        ctx.declare_func("concat", create_concat_func());
        ctx.declare_func("flatten", create_flatten_func());
        ctx.declare_func("distinct", create_distinct_func());
        ctx.declare_func("slice", create_slice_func());
        ctx.declare_func("sort", create_sort_func());
        ctx.declare_func("reverse", create_reverse_func());
        ctx.declare_func("index", create_index_func());
        ctx
    }

    #[test]
    fn test_concat_function() {
        let ctx = create_test_context();
        let expr_str = "concat([1,2], [3])";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let expected = Value::from(vec![Value::from(1), Value::from(2), Value::from(3)]);
        assert_eq!(expr.evaluate(&ctx).unwrap(), expected);
    }

    #[test]
    fn test_flatten_function() {
        let ctx = create_test_context();
        let expr_str = "flatten([[1,2],[3,4]])";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let expected = Value::from(
            vec![1, 2, 3, 4]
                .into_iter()
                .map(Value::from)
                .collect::<Vec<_>>(),
        );
        assert_eq!(expr.evaluate(&ctx).unwrap(), expected);
    }

    #[test]
    fn test_distinct_function() {
        let ctx = create_test_context();
        let expr_str = "distinct([1,2,2,3])";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let expected = Value::from(
            vec![1, 2, 3]
                .into_iter()
                .map(Value::from)
                .collect::<Vec<_>>(),
        );
        assert_eq!(expr.evaluate(&ctx).unwrap(), expected);
    }

    #[test]
    fn test_slice_function() {
        let ctx = create_test_context();
        let expr_str = "slice([1,2,3,4,5], 1, 3)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let expected = Value::from(vec![Value::from(2), Value::from(3)]);
        assert_eq!(expr.evaluate(&ctx).unwrap(), expected);
    }

    #[test]
    fn test_sort_function() {
        let ctx = create_test_context();
        let expr_str = "sort([\"b\", \"a\", \"c\"])";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let expected = Value::from(vec![Value::from("a"), Value::from("b"), Value::from("c")]);
        assert_eq!(expr.evaluate(&ctx).unwrap(), expected);
    }

    #[test]
    fn test_reverse_function() {
        let ctx = create_test_context();
        let expr_str = "reverse([1,2,3])";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let expected = Value::from(vec![Value::from(3), Value::from(2), Value::from(1)]);
        assert_eq!(expr.evaluate(&ctx).unwrap(), expected);
    }

    #[test]
    fn test_index_function_success() {
        let ctx = create_test_context();
        let expr_str = "index([1,2,3], 2)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(1));
    }

    #[test]
    fn test_index_function_error() {
        let ctx = create_test_context();
        let expr_str = "index([1,2,3], 5)";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert!(expr.evaluate(&ctx).is_err());
    }
}
