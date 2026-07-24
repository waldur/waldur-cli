//! Tests for `--filter KEY=VALUE` parsing/validation (src/filter.rs).

use waldur_cli::filter::{parse_filters, FilterKind};

const SPEC: &[(&str, FilterKind)] = &[
    ("name", FilterKind::Str),
    ("archived", FilterKind::Bool),
    ("age", FilterKind::I64),
];

#[test]
fn accepts_valid_values_of_each_kind() {
    let raw = vec![
        "name=Acme".to_string(),
        "archived=true".to_string(),
        "age=42".to_string(),
    ];
    let parsed = parse_filters(&raw, SPEC).unwrap();
    assert_eq!(
        parsed,
        vec![
            ("name".to_string(), "Acme".to_string()),
            ("archived".to_string(), "true".to_string()),
            ("age".to_string(), "42".to_string()),
        ]
    );
}

#[test]
fn accepts_false_for_bool() {
    let raw = vec!["archived=false".to_string()];
    let parsed = parse_filters(&raw, SPEC).unwrap();
    assert_eq!(parsed, vec![("archived".to_string(), "false".to_string())]);
}

#[test]
fn repeated_key_is_allowed_ord_style() {
    // --filter type=A --filter type=B both come through; server ORs them.
    let raw = vec!["name=A".to_string(), "name=B".to_string()];
    let parsed = parse_filters(&raw, SPEC).unwrap();
    assert_eq!(parsed.len(), 2);
}

#[test]
fn rejects_malformed_entry_without_equals() {
    let raw = vec!["archived".to_string()];
    let err = parse_filters(&raw, SPEC).unwrap_err();
    assert!(err.to_string().contains("expected KEY=VALUE"));
}

#[test]
fn rejects_unknown_key_and_lists_valid_ones() {
    let raw = vec!["bogus=1".to_string()];
    let err = parse_filters(&raw, SPEC).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown filter key `bogus`"));
    assert!(msg.contains("name"));
    assert!(msg.contains("archived"));
    assert!(msg.contains("age"));
}

#[test]
fn rejects_invalid_bool_value() {
    let raw = vec!["archived=maybe".to_string()];
    let err = parse_filters(&raw, SPEC).unwrap_err();
    assert!(err.to_string().contains("expected true or false"));
}

#[test]
fn rejects_invalid_integer_value() {
    let raw = vec!["age=old".to_string()];
    let err = parse_filters(&raw, SPEC).unwrap_err();
    assert!(err.to_string().contains("expected an integer"));
}

#[test]
fn empty_input_yields_empty_params() {
    let parsed = parse_filters(&[], SPEC).unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn value_may_itself_contain_an_equals_sign() {
    // split_once('=') only splits on the FIRST '=', so a value with '=' in it
    // (e.g. a base64-ish token) round-trips intact.
    let raw = vec!["name=a=b=c".to_string()];
    let parsed = parse_filters(&raw, SPEC).unwrap();
    assert_eq!(parsed, vec![("name".to_string(), "a=b=c".to_string())]);
}
