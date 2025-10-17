use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use hcl::value::Map;
use hcl::Body;

use crate::frontend::core::get_attr_string;
use crate::frontend::env::EnvVars;
use crate::prisma::{
    self, BlockAttribute, ConfigBlock, DefaultValue, FieldAttribute, Model, Schema, View,
};
use crate::Loader;

/// Load all `data` blocks in the current body and populate the evaluation environment.
pub fn load_data_sources(
    loader: &dyn Loader,
    base: &Path,
    body: &Body,
    env: &mut EnvVars,
) -> Result<()> {
    for blk in body.blocks().filter(|b| b.identifier() == "data") {
        let dtype = blk
            .labels()
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("data block missing type label"))?
            .as_str()
            .to_string();
        let name = blk
            .labels()
            .get(1)
            .ok_or_else(|| anyhow::anyhow!("data block missing name label"))?
            .as_str()
            .to_string();

        let value = match dtype.as_str() {
            "prisma_schema" => load_prisma_schema(loader, base, blk.body(), env)?,
            other => bail!("unsupported data source type '{other}'"),
        };

        env.data
            .entry(dtype)
            .or_insert_with(HashMap::new)
            .insert(name, value);
    }
    Ok(())
}

fn load_prisma_schema(
    loader: &dyn Loader,
    base: &Path,
    body: &Body,
    env: &EnvVars,
) -> Result<hcl::Value> {
    let file = get_attr_string(body, "file", env)?
        .context("prisma_schema data source requires 'file' attribute")?;
    let path = resolve_relative(base, &file);
    let contents = loader
        .load(&path)
        .with_context(|| format!("reading Prisma schema from {}", path.display()))?;
    let schema = prisma::parse_schema_str(&contents)?;
    Ok(schema_to_value(schema))
}

fn resolve_relative(base: &Path, value: &str) -> PathBuf {
    let p = Path::new(value);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

fn schema_to_value(schema: Schema) -> hcl::Value {
    let mut root = Map::<String, hcl::Value>::new();
    root.insert("models".into(), models_to_value(&schema.models));
    root.insert("views".into(), views_to_value(&schema.views));
    root.insert(
        "composite_types".into(),
        composite_types_to_value(&schema.composite_types),
    );
    root.insert("enums".into(), enums_to_value(&schema.enums));
    root.insert(
        "datasources".into(),
        config_blocks_to_value(&schema.datasources),
    );
    root.insert(
        "generators".into(),
        config_blocks_to_value(&schema.generators),
    );
    hcl::Value::Object(root)
}

fn models_to_value(models: &[Model]) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    for model in models {
        map.insert(
            model.name.clone(),
            model_like_to_value(&model.name, &model.fields, &model.attributes),
        );
    }
    hcl::Value::Object(map)
}

fn views_to_value(views: &[View]) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    for view in views {
        map.insert(
            view.name.clone(),
            model_like_to_value(&view.name, &view.fields, &view.attributes),
        );
    }
    hcl::Value::Object(map)
}

fn model_like_to_value(
    name: &str,
    fields: &[prisma::Field],
    attributes: &[BlockAttribute],
) -> hcl::Value {
    let mut model_map = Map::new();
    model_map.insert("name".into(), hcl::Value::String(name.to_string()));
    model_map.insert("fields".into(), fields_to_value(fields));
    model_map.insert("attributes".into(), block_attributes_to_value(attributes));
    hcl::Value::Object(model_map)
}

fn composite_types_to_value(types: &[prisma::CompositeType]) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    for ct in types {
        let mut ct_map = Map::new();
        ct_map.insert("name".into(), hcl::Value::String(ct.name.clone()));
        ct_map.insert("fields".into(), fields_to_value(&ct.fields));
        map.insert(ct.name.clone(), hcl::Value::Object(ct_map));
    }
    hcl::Value::Object(map)
}

fn fields_to_value(fields: &[prisma::Field]) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    for field in fields {
        let mut field_map = Map::new();
        field_map.insert("name".into(), hcl::Value::String(field.name.clone()));
        field_map.insert("type".into(), type_to_value(&field.r#type));
        field_map.insert(
            "attributes".into(),
            field_attributes_to_value(&field.attributes),
        );
        map.insert(field.name.clone(), hcl::Value::Object(field_map));
    }
    hcl::Value::Object(map)
}

fn enums_to_value(enums: &[prisma::Enum]) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    for enm in enums {
        let mut enum_map = Map::new();
        enum_map.insert("name".into(), hcl::Value::String(enm.name.clone()));
        enum_map.insert("values".into(), enum_values_to_value(&enm.values));
        enum_map.insert(
            "attributes".into(),
            block_attributes_to_value(&enm.attributes),
        );
        map.insert(enm.name.clone(), hcl::Value::Object(enum_map));
    }
    hcl::Value::Object(map)
}

fn enum_values_to_value(values: &[prisma::EnumValue]) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    for value in values {
        let mut value_map = Map::new();
        value_map.insert("name".into(), hcl::Value::String(value.name.clone()));
        if let Some(mapped) = &value.mapped_name {
            value_map.insert("mapped_name".into(), hcl::Value::String(mapped.clone()));
        }
        map.insert(value.name.clone(), hcl::Value::Object(value_map));
    }
    hcl::Value::Object(map)
}

fn config_blocks_to_value(blocks: &[ConfigBlock]) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    for block in blocks {
        let mut block_map = Map::new();
        block_map.insert("name".into(), hcl::Value::String(block.name.clone()));
        if let Some(doc) = &block.documentation {
            block_map.insert("documentation".into(), hcl::Value::String(doc.clone()));
        }
        block_map.insert(
            "properties".into(),
            config_properties_to_value(&block.properties),
        );
        map.insert(block.name.clone(), hcl::Value::Object(block_map));
    }
    hcl::Value::Object(map)
}

fn config_properties_to_value(properties: &[prisma::ConfigProperty]) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    for prop in properties {
        let mut prop_map = Map::new();
        prop_map.insert("name".into(), hcl::Value::String(prop.name.clone()));
        if let Some(value) = &prop.value {
            prop_map.insert("value".into(), hcl::Value::String(value.clone()));
        }
        map.insert(prop.name.clone(), hcl::Value::Object(prop_map));
    }
    hcl::Value::Object(map)
}

fn type_to_value(ty: &prisma::Type) -> hcl::Value {
    let mut map = Map::new();
    map.insert("name".into(), hcl::Value::String(ty.name.clone()));
    map.insert("optional".into(), hcl::Value::Bool(ty.optional));
    map.insert("list".into(), hcl::Value::Bool(ty.list));
    hcl::Value::Object(map)
}

fn field_attributes_to_value(attrs: &[FieldAttribute]) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    let raw: Vec<hcl::Value> = attrs
        .iter()
        .map(|a| hcl::Value::String(format!("{}", a)))
        .collect();
    map.insert("raw".into(), hcl::Value::Array(raw));

    if attrs.iter().any(|a| matches!(a, FieldAttribute::Id)) {
        map.insert("id".into(), hcl::Value::Bool(true));
    }
    if attrs.iter().any(|a| matches!(a, FieldAttribute::Unique)) {
        map.insert("unique".into(), hcl::Value::Bool(true));
    }
    if let Some(default) = attrs.iter().find_map(|a| match a {
        FieldAttribute::Default(d) => Some(d),
        _ => None,
    }) {
        map.insert("default".into(), default_to_value(default));
    }
    if let Some(map_attr) = attrs.iter().find_map(|a| match a {
        FieldAttribute::Map(m) => Some(m.clone()),
        _ => None,
    }) {
        map.insert("map".into(), hcl::Value::String(map_attr));
    }
    if let Some(db) = attrs.iter().find_map(|a| match a {
        FieldAttribute::DbNative(db) => Some(db.clone()),
        _ => None,
    }) {
        map.insert("db_native".into(), hcl::Value::String(db));
    }
    if let Some(relation) = attrs.iter().find_map(|a| match a {
        FieldAttribute::Relation(rel) => Some(rel),
        _ => None,
    }) {
        map.insert("relation".into(), relation_to_value(relation));
    }

    hcl::Value::Object(map)
}

fn block_attributes_to_value(attrs: &[BlockAttribute]) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    let raw: Vec<hcl::Value> = attrs
        .iter()
        .map(|a| hcl::Value::String(format!("{}", a)))
        .collect();
    map.insert("raw".into(), hcl::Value::Array(raw));

    if let Some(name) = attrs.iter().find_map(|a| match a {
        BlockAttribute::Map(m) => Some(m.clone()),
        _ => None,
    }) {
        map.insert("map".into(), hcl::Value::String(name));
    }

    hcl::Value::Object(map)
}

fn relation_to_value(relation: &prisma::RelationAttribute) -> hcl::Value {
    let mut map = Map::<String, hcl::Value>::new();
    if let Some(name) = &relation.name {
        map.insert("name".into(), hcl::Value::String(name.clone()));
    }
    map.insert("fields".into(), strings_to_array(&relation.fields));
    map.insert("references".into(), strings_to_array(&relation.references));
    if let Some(map_name) = &relation.map {
        map.insert("map".into(), hcl::Value::String(map_name.clone()));
    }
    if let Some(on_delete) = &relation.on_delete {
        map.insert("on_delete".into(), hcl::Value::String(on_delete.clone()));
    }
    if let Some(on_update) = &relation.on_update {
        map.insert("on_update".into(), hcl::Value::String(on_update.clone()));
    }

    hcl::Value::Object(map)
}

fn default_to_value(value: &DefaultValue) -> hcl::Value {
    hcl::Value::String(format!("{}", value))
}

fn strings_to_array(values: &[String]) -> hcl::Value {
    hcl::Value::Array(values.iter().cloned().map(hcl::Value::String).collect())
}
