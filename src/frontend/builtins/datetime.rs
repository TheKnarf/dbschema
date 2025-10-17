use chrono::{DateTime, Duration, Utc};
use hcl::Value;
use hcl::eval::{FuncArgs, FuncDef, ParamType};

/// Return the current timestamp in RFC3339 format
pub fn create_timestamp_func() -> FuncDef {
    FuncDef::builder().build(|_: FuncArgs| Ok(Value::from(Utc::now().to_rfc3339())))
}

/// Format a timestamp using the provided format string
pub fn create_formatdate_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let fmt = args[0].as_str().unwrap();
            let ts = args[1].as_str().unwrap();
            let dt = DateTime::parse_from_rfc3339(ts).map_err(|e| e.to_string())?;
            Ok(Value::from(dt.format(fmt).to_string()))
        })
}

/// Add a duration to a timestamp
pub fn create_timeadd_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let ts = args[0].as_str().unwrap();
            let dur_str = args[1].as_str().unwrap();
            let dt = DateTime::parse_from_rfc3339(ts)
                .map_err(|e| e.to_string())?
                .with_timezone(&Utc);

            if dur_str.is_empty() {
                return Err("invalid duration".to_string());
            }

            let (num_part, unit_part) = dur_str.split_at(dur_str.len() - 1);
            let n: i64 = num_part
                .parse()
                .map_err(|_| "invalid duration".to_string())?;
            let dur = match unit_part {
                "s" => Duration::seconds(n),
                "m" => Duration::minutes(n),
                "h" => Duration::hours(n),
                "d" => Duration::days(n),
                _ => return Err("invalid duration".to_string()),
            };

            let new_dt = dt + dur;
            Ok(Value::from(new_dt.to_rfc3339()))
        })
}

/// Compare two timestamps returning -1, 0, or 1
pub fn create_timecmp_func() -> FuncDef {
    FuncDef::builder()
        .param(ParamType::String)
        .param(ParamType::String)
        .build(|args: FuncArgs| {
            let ts1 = DateTime::parse_from_rfc3339(args[0].as_str().unwrap())
                .map_err(|e| e.to_string())?;
            let ts2 = DateTime::parse_from_rfc3339(args[1].as_str().unwrap())
                .map_err(|e| e.to_string())?;
            let ord = ts1.cmp(&ts2);
            let result = match ord {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            Ok(Value::from(result))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hcl::eval::{Context, Evaluate};

    fn create_test_context() -> Context<'static> {
        let mut ctx = Context::new();
        ctx.declare_func("timestamp", create_timestamp_func());
        ctx.declare_func("formatdate", create_formatdate_func());
        ctx.declare_func("timeadd", create_timeadd_func());
        ctx.declare_func("timecmp", create_timecmp_func());
        ctx
    }

    #[test]
    fn test_timestamp_function() {
        let ctx = create_test_context();
        let expr_str = "timestamp()";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        let val = expr.evaluate(&ctx).unwrap();
        assert!(DateTime::parse_from_rfc3339(val.as_str().unwrap()).is_ok());
    }

    #[test]
    fn test_formatdate_function() {
        let ctx = create_test_context();
        let expr_str = "formatdate(\"%Y-%m-%d\", \"2020-01-01T00:00:00Z\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from("2020-01-01"));
    }

    #[test]
    fn test_formatdate_error() {
        let ctx = create_test_context();
        let expr_str = "formatdate(\"%Y\", \"invalid\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert!(expr.evaluate(&ctx).is_err());
    }

    #[test]
    fn test_timeadd_function() {
        let ctx = create_test_context();
        let expr_str = "timeadd(\"2020-01-01T00:00:00Z\", \"1h\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(
            expr.evaluate(&ctx).unwrap(),
            Value::from("2020-01-01T01:00:00+00:00")
        );
    }

    #[test]
    fn test_timeadd_error() {
        let ctx = create_test_context();
        let expr_str = "timeadd(\"2020-01-01T00:00:00Z\", \"10x\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert!(expr.evaluate(&ctx).is_err());
    }

    #[test]
    fn test_timecmp_function() {
        let ctx = create_test_context();
        let expr_str = "timecmp(\"2020-01-01T00:00:00Z\", \"2020-01-02T00:00:00Z\")";
        let body: hcl::Body = hcl::from_str(&format!("test = {}", expr_str)).unwrap();
        let expr = body
            .attributes()
            .find(|a| a.key() == "test")
            .unwrap()
            .expr();
        assert_eq!(expr.evaluate(&ctx).unwrap(), Value::from(-1));
    }
}
