//! Each tool's input schema in the subset every local client reads: no `$ref`, union, `null`,
//! `const` or number format (`docs/design/ai-integration.md`, "Any other local client").

use rmcp::model::JsonObject;
use serde_json::{Value, json};

/// How deep `$ref`s are written in place; the parameters nest a few deep and none holds itself.
const MOST_DEPTH: usize = 16;
/// Where a folded field holds, until every variant is read, the choice it goes with.
const ONLY_WITH: &str = "x-only-with";

/// `schema` with each `$ref` written in place, each union folded into one type and `null`,
/// `const` and number formats dropped; the arguments it accepts read as before.
pub fn portable(schema: &JsonObject) -> JsonObject {
    let defs = match schema.get("$defs") {
        Some(Value::Object(defs)) => defs.clone(),
        _ => JsonObject::new(),
    };
    let mut out = schema.clone();
    out.remove("$defs");
    out.remove("$schema");
    fold(&mut out, &defs, 0);
    out
}

/// Folds `node` and everything under it, the children first so a union sees folded variants.
fn fold(node: &mut JsonObject, defs: &JsonObject, depth: usize) {
    if depth <= MOST_DEPTH {
        write_ref_in_place(node, defs);
    }
    for child in children(node) {
        fold(child, defs, depth + 1);
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(Value::Array(variants)) = node.remove(key) {
            fold_union(node, variants);
        }
    }
    drop_null_type(node);
    if node.get("default") == Some(&Value::Null) {
        node.remove("default");
    }
    if let Some(value) = node.remove("const") {
        node.insert("enum".into(), json!([value]));
    }
    if node.get("type").and_then(Value::as_str) != Some("string") {
        node.remove("format");
    }
}

/// The schemas directly under `node`: its properties, its items and its union's variants.
fn children(node: &mut JsonObject) -> Vec<&mut JsonObject> {
    let mut found = Vec::new();
    for (key, value) in node.iter_mut() {
        match (key.as_str(), value) {
            ("properties", Value::Object(properties)) => {
                found.extend(properties.values_mut().filter_map(Value::as_object_mut));
            }
            ("items", Value::Object(items)) => found.push(items),
            ("anyOf" | "oneOf", Value::Array(variants)) => {
                found.extend(variants.iter_mut().filter_map(Value::as_object_mut));
            }
            _ => {}
        }
    }
    found
}

/// Replaces a `$ref` by what it names, `node`'s own keys, such as its description, kept.
fn write_ref_in_place(node: &mut JsonObject, defs: &JsonObject) {
    let Some(Value::String(reference)) = node.remove("$ref") else {
        return;
    };
    let name = reference.trim_start_matches("#/$defs/");
    if let Some(Value::Object(named)) = defs.get(name) {
        for (key, value) in named {
            node.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
}

/// `["integer", "null"]` read as `"integer"`: an absent field already says none.
fn drop_null_type(node: &mut JsonObject) {
    if let Some(Value::Array(types)) = node.get_mut("type") {
        types.retain(|kind| kind != "null");
        if let [only] = types.as_slice() {
            let only = only.clone();
            node.insert("type".into(), only);
        }
    }
}

/// Folds a union into `node`: an optional value into the value, choices of strings into one
/// `enum`, objects told apart by a field into one object with every field.
fn fold_union(node: &mut JsonObject, variants: Vec<Value>) {
    let variants: Vec<JsonObject> = (variants.into_iter())
        .filter_map(|variant| match variant {
            Value::Object(variant) if variant.get("type") != Some(&json!("null")) => Some(variant),
            _ => None,
        })
        .collect();
    match variants.as_slice() {
        [only] => {
            for (key, value) in only {
                match (key.as_str(), value) {
                    ("description", Value::String(said)) if node.contains_key(key) => {
                        append_description(node, said);
                    }
                    _ => {
                        node.entry(key.clone()).or_insert_with(|| value.clone());
                    }
                }
            }
        }
        _ if variants
            .iter()
            .all(|v| v.get("type") == Some(&json!("string"))) =>
        {
            fold_strings(node, &variants);
        }
        _ if variants.iter().all(|v| v.contains_key("properties")) => {
            fold_objects(node, &variants);
        }
        _ => {
            node.insert(
                "anyOf".into(),
                variants.into_iter().map(Value::Object).collect(),
            );
        }
    }
}

/// String choices as one `enum`, each choice's description added to `node`'s.
fn fold_strings(node: &mut JsonObject, variants: &[JsonObject]) {
    let mut choices = Vec::new();
    for variant in variants {
        let values = enum_of(variant);
        if let (Some(Value::String(said)), [value]) = (variant.get("description"), values) {
            append_description(node, &format!("`{}`: {said}", plain(value)));
        }
        choices.extend_from_slice(values);
    }
    node.insert("type".into(), json!("string"));
    node.insert("enum".into(), Value::Array(choices));
}

/// Objects told apart by a one-value field as one object: that field an `enum` of them all,
/// each other field marked with the choice it belongs to, needed only where every one needs it.
fn fold_objects(node: &mut JsonObject, variants: &[JsonObject]) {
    let mut properties = JsonObject::new();
    let mut required: Option<Vec<Value>> = None;
    for variant in variants {
        let tag = tag_of(variant);
        if let (Some(Value::String(said)), Some(tag)) = (variant.get("description"), &tag) {
            append_description(node, &format!("`{tag}`: {said}"));
        }
        let needs = match variant.get("required") {
            Some(Value::Array(needs)) => needs.clone(),
            _ => Vec::new(),
        };
        for (name, field) in variant
            .get("properties")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            merge_field(
                &mut properties,
                name,
                field,
                (tag.as_deref(), needs.contains(&json!(name))),
            );
        }
        required = Some(match required {
            None => needs,
            Some(before) => before
                .into_iter()
                .filter(|name| needs.contains(name))
                .collect(),
        });
    }
    let shared: Vec<&str> = (required.iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    for (name, field) in &mut properties {
        let Some(field) = field.as_object_mut() else {
            continue;
        };
        if let Some(Value::String(note)) = field.remove(ONLY_WITH)
            && !shared.contains(&name.as_str())
        {
            append_description(field, &note);
        }
    }
    node.insert("type".into(), json!("object"));
    node.insert("properties".into(), Value::Object(properties));
    node.insert("required".into(), json!(shared));
}

/// Adds one variant's `field` to `properties`: a second one-value field joins the first's
/// `enum`; any other field notes the choice it goes with, and whether that choice needs it.
fn merge_field(
    properties: &mut JsonObject,
    name: &str,
    field: &Value,
    (tag, needed): (Option<&str>, bool),
) {
    if let Some(Value::Object(before)) = properties.get_mut(name) {
        if let (Some(Value::Array(had)), Some(Value::Array(more))) =
            (before.get_mut("enum"), field.get("enum"))
        {
            had.extend(more.iter().cloned());
        }
        before.remove(ONLY_WITH);
        return;
    }
    let mut field = field.clone();
    if let (Some(object), Some(tag)) = (field.as_object_mut(), tag) {
        let note = if needed {
            format!("Needed with `{tag}`.")
        } else {
            format!("Only with `{tag}`.")
        };
        object.insert(ONLY_WITH.into(), json!(note));
    }
    properties.insert(name.to_owned(), field);
}

/// The field a variant is told apart by, as `by: search`: the first with a single value.
fn tag_of(variant: &JsonObject) -> Option<String> {
    let properties = variant.get("properties")?.as_object()?;
    properties
        .iter()
        .find_map(|(name, field)| match enum_of(field.as_object()?) {
            [value] => Some(format!("{name}: {}", plain(value))),
            _ => None,
        })
}

fn enum_of(schema: &JsonObject) -> &[Value] {
    schema
        .get("enum")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// A JSON value as a reader writes it: a string without its quotes.
fn plain(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

fn append_description(node: &mut JsonObject, more: &str) {
    let said = match node.get("description") {
        Some(Value::String(said)) => format!("{said} {more}"),
        _ => more.to_owned(),
    };
    node.insert("description".into(), json!(said));
}

#[cfg(test)]
mod tests;
