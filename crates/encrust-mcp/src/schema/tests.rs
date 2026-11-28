use serde_json::{Value, json};

use super::portable;

fn folded(schema: Value) -> Value {
    let Value::Object(schema) = schema else {
        panic!("a schema is an object");
    };
    Value::Object(portable(&schema))
}

#[test]
fn an_optional_reference_reads_as_what_it_names() {
    let schema = folded(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": { "Kind": { "type": "string", "enum": ["drain", "walled"], "description": "A hole." } },
        "type": "object",
        "properties": {
            "kind": { "anyOf": [{ "$ref": "#/$defs/Kind" }, { "type": "null" }], "description": "How." },
            "revision": { "type": ["integer", "null"], "format": "uint32", "default": null }
        }
    }));
    assert_eq!(
        schema,
        json!({
            "type": "object",
            "properties": {
                "kind": { "type": "string", "enum": ["drain", "walled"], "description": "How. A hole." },
                "revision": { "type": "integer" }
            }
        })
    );
}

#[test]
fn string_choices_read_as_one_enum_their_descriptions_kept() {
    let schema = folded(json!({
        "description": "A side.",
        "oneOf": [
            { "type": "string", "enum": ["front", "back"] },
            { "type": "string", "const": "iso", "description": "From a corner." }
        ]
    }));
    assert_eq!(
        schema,
        json!({
            "description": "A side. `iso`: From a corner.",
            "type": "string",
            "enum": ["front", "back", "iso"]
        })
    );
}

#[test]
fn objects_told_apart_by_a_field_read_as_one_object() {
    let schema = folded(json!({
        "oneOf": [
            {
                "description": "The search.",
                "properties": {
                    "by": { "type": "string", "const": "search" },
                    "goal": { "type": "string" }
                },
                "required": ["by", "goal"]
            },
            {
                "description": "A turn.",
                "properties": {
                    "by": { "type": "string", "const": "rotate" },
                    "degrees": { "type": "array", "items": { "type": "number", "format": "float" } }
                },
                "required": ["by", "degrees"]
            }
        ]
    }));
    assert_eq!(
        schema,
        json!({
            "description": "`by: search`: The search. `by: rotate`: A turn.",
            "type": "object",
            "properties": {
                "by": { "type": "string", "enum": ["search", "rotate"] },
                "goal": { "type": "string", "description": "Needed with `by: search`." },
                "degrees": {
                    "type": "array",
                    "items": { "type": "number" },
                    "description": "Needed with `by: rotate`."
                }
            },
            "required": ["by"]
        })
    );
}
