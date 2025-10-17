use crate::frontend::ast;
use crate::frontend::core;
use crate::frontend::env::EnvVars;
use anyhow::{Result, bail};

/// Trait for types that support for_each iteration
pub trait ForEachSupport {
    /// The type of item that gets created for each iteration
    type Item;

    /// Parse a single item from the HCL block
    fn parse_one(name: &str, body: &hcl::Body, env: &EnvVars) -> Result<Self::Item>;

    /// Add the parsed item to the configuration
    fn add_to_config(item: Self::Item, config: &mut ast::Config);
}

/// Execute for_each iteration for any type that implements ForEachSupport
pub fn execute_for_each<T: ForEachSupport>(
    name: &str,
    body: &hcl::Body,
    env: &EnvVars,
    config: &mut ast::Config,
    for_each_expr: Option<&hcl::Attribute>,
    count_expr: Option<&hcl::Attribute>,
) -> Result<()> {
    if for_each_expr.is_some() && count_expr.is_some() {
        bail!("cannot use both for_each and count on the same block");
    }
    if let Some(fe) = for_each_expr {
        let coll = core::expr_to_value(fe.expr(), env)?;
        for_each_iter(&coll, &mut |k, v| {
            let mut iter_env = env.clone();
            iter_env.each = Some((k.clone(), v.clone()));
            let item = T::parse_one(name, body, &iter_env)?;
            T::add_to_config(item, config);
            Ok(())
        })?;
    } else if let Some(ce) = count_expr {
        let val = core::expr_to_value(ce.expr(), env)?;
        let times = match val {
            hcl::Value::Number(n) => n
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("count must be a non-negative integer"))?
                as usize,
            other => bail!("count expects number, got {other:?}"),
        };
        for i in 0..times {
            let mut iter_env = env.clone();
            iter_env.count = Some(i);
            let item = T::parse_one(name, body, &iter_env)?;
            T::add_to_config(item, config);
        }
    } else {
        let item = T::parse_one(name, body, env)?;
        T::add_to_config(item, config);
    }
    Ok(())
}

/// Iterator function for for_each loops
pub fn for_each_iter<F>(collection: &hcl::Value, f: &mut F) -> Result<()>
where
    F: FnMut(hcl::Value, hcl::Value) -> Result<()>,
{
    match collection {
        hcl::Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                f(hcl::Value::Number(hcl::Number::from(i as u64)), v.clone())?;
            }
        }
        hcl::Value::Object(obj) => {
            for (k, v) in obj.iter() {
                f(hcl::Value::String(k.clone()), v.clone())?;
            }
        }
        other => bail!("for_each expects array or object, got {other:?}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementation for testing
    #[allow(dead_code)]
    struct MockResource;

    impl ForEachSupport for MockResource {
        type Item = String;

        fn parse_one(name: &str, _body: &hcl::Body, env: &EnvVars) -> Result<Self::Item> {
            // Simple mock that returns the name with each.value if available
            if let Some((_key, value)) = &env.each {
                Ok(format!("{}-{}", name, value))
            } else {
                Ok(name.to_string())
            }
        }

        fn add_to_config(_item: Self::Item, _config: &mut ast::Config) {
            // Mock implementation - do nothing
        }
    }

    #[test]
    fn test_for_each_iter_with_array() {
        let collection = hcl::Value::Array(vec![
            hcl::Value::String("item1".to_string()),
            hcl::Value::String("item2".to_string()),
            hcl::Value::String("item3".to_string()),
        ]);

        let mut results = Vec::new();
        for_each_iter(&collection, &mut |k, v| {
            results.push((k, v));
            Ok(())
        })
        .unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, hcl::Value::Number(hcl::Number::from(0)));
        assert_eq!(results[0].1, hcl::Value::String("item1".to_string()));
        assert_eq!(results[1].0, hcl::Value::Number(hcl::Number::from(1)));
        assert_eq!(results[1].1, hcl::Value::String("item2".to_string()));
        assert_eq!(results[2].0, hcl::Value::Number(hcl::Number::from(2)));
        assert_eq!(results[2].1, hcl::Value::String("item3".to_string()));
    }

    #[test]
    fn test_for_each_iter_with_object() {
        let mut obj = hcl::value::Map::new();
        obj.insert("key1".to_string(), hcl::Value::String("value1".to_string()));
        obj.insert("key2".to_string(), hcl::Value::String("value2".to_string()));
        let collection = hcl::Value::Object(obj);

        let mut results = Vec::new();
        for_each_iter(&collection, &mut |k, v| {
            results.push((k, v));
            Ok(())
        })
        .unwrap();

        assert_eq!(results.len(), 2);
        // Results may be in any order due to HashMap iteration
        let mut sorted_results = results;
        sorted_results.sort_by(|a, b| match (&a.0, &b.0) {
            (hcl::Value::String(s1), hcl::Value::String(s2)) => s1.cmp(s2),
            _ => std::cmp::Ordering::Equal,
        });

        assert_eq!(sorted_results[0].0, hcl::Value::String("key1".to_string()));
        assert_eq!(
            sorted_results[0].1,
            hcl::Value::String("value1".to_string())
        );
        assert_eq!(sorted_results[1].0, hcl::Value::String("key2".to_string()));
        assert_eq!(
            sorted_results[1].1,
            hcl::Value::String("value2".to_string())
        );
    }

    #[test]
    fn test_for_each_iter_with_invalid_type() {
        let collection = hcl::Value::String("invalid".to_string());

        let result = for_each_iter(&collection, &mut |_, _| Ok(()));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("for_each expects array or object")
        );
    }
}
