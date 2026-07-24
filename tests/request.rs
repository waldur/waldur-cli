//! Tests for request-body loading/validation (src/request.rs): the
//! null-stripping `load_body` does for skeleton round-tripping, and
//! `validate_request_body`'s JSON Schema validation (replacing the old
//! rs-client-typed-struct approach).

use std::io::Write;
use waldur_cli::request::{load_body, validate_request_body};

const SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "name": {"type": "string"},
        "grace_period_days": {"type": "integer"},
        "accounting_start_date": {"type": "string", "format": "date-time"},
        "country": {"enum": ["EE", "US", "DE"]}
    },
    "required": ["name"]
}"#;

#[test]
fn valid_body_passes() {
    validate_request_body(SCHEMA, r#"{"name": "Acme"}"#).unwrap();
}

#[test]
fn missing_required_field_fails() {
    let err = validate_request_body(SCHEMA, r#"{}"#).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not valid JSON for this resource's request schema"));
    assert!(msg.contains("required property"));
}

#[test]
fn wrong_type_fails_with_field_and_reason() {
    let err = validate_request_body(SCHEMA, r#"{"name": 123}"#).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("/name"));
    assert!(msg.contains("not of type"));
}

#[test]
fn invalid_enum_value_fails() {
    let err = validate_request_body(SCHEMA, r#"{"name": "x", "country": "ZZ"}"#).unwrap_err();
    assert!(err.to_string().contains("/country"));
}

#[test]
fn malformed_date_format_fails() {
    // The regression case this whole design exists to catch: a plain
    // {"type": "string"} would accept any string. should_validate_formats(true)
    // is what makes this fail instead.
    let err = validate_request_body(
        SCHEMA,
        r#"{"name": "x", "accounting_start_date": "not-a-date"}"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("accounting_start_date"));
}

#[test]
fn valid_date_format_passes() {
    validate_request_body(
        SCHEMA,
        r#"{"name": "x", "accounting_start_date": "2024-01-01T00:00:00Z"}"#,
    )
    .unwrap();
}

#[test]
fn unknown_extra_field_still_passes() {
    // Matches the old lax behavior (no deny_unknown_fields on the rs-client
    // structs this replaced) -- additionalProperties defaults to allowed.
    validate_request_body(SCHEMA, r#"{"name": "x", "totally_bogus_field": true}"#).unwrap();
}

// -- load_body --------------------------------------------------------------

#[test]
fn load_body_strips_top_level_null_keys() {
    let body = load_body(Some(r#"{"name": "Acme", "email": null}"#), None).unwrap();
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value, serde_json::json!({"name": "Acme"}));
}

#[test]
fn load_body_strips_nested_null_keys() {
    let body = load_body(
        Some(r#"{"attributes": {"name": "vpc", "flavor": null}, "limits": null}"#),
        None,
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value, serde_json::json!({"attributes": {"name": "vpc"}}));
}

#[test]
fn load_body_preserves_null_array_elements() {
    // Only object *keys* with a null value are dropped -- an array element
    // that happens to be null is real list content, not "unset."
    let body = load_body(Some(r#"{"items": ["a", null, "b"]}"#), None).unwrap();
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value, serde_json::json!({"items": ["a", null, "b"]}));
}

#[test]
fn load_body_rejects_malformed_json() {
    let err = load_body(Some("{not json"), None).unwrap_err();
    assert!(err.to_string().contains("not valid JSON"));
}

#[test]
fn load_body_requires_exactly_one_source() {
    let err = load_body(None, None).unwrap_err();
    assert!(err.to_string().contains("exactly one"));
}

#[test]
fn load_body_reads_json_from_file() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(file, r#"{{"name": "Acme"}}"#).unwrap();
    let body = load_body(None, Some(file.path())).unwrap();
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value, serde_json::json!({"name": "Acme"}));
}

#[test]
fn load_body_reads_yaml_from_file() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(file, "name: Acme\nabbreviation: null\n").unwrap();
    let body = load_body(None, Some(file.path())).unwrap();
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    // abbreviation: null is stripped, same as the inline-JSON path.
    assert_eq!(value, serde_json::json!({"name": "Acme"}));
}
