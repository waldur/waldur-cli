//! Tests for `--jmespath` client-side reshaping (src/query.rs).

use serde_json::json;
use waldur_cli::query::apply;

#[test]
fn projects_a_field_across_an_array() {
    let value = json!([
        {"name": "Acme", "uuid": "1"},
        {"name": "Beta", "uuid": "2"},
    ]);
    let result = apply(value, "[].name").unwrap();
    assert_eq!(result, json!(["Acme", "Beta"]));
}

#[test]
fn filters_with_a_predicate() {
    let value = json!([
        {"name": "Acme", "blocked": true},
        {"name": "Beta", "blocked": false},
    ]);
    let result = apply(value, "[?blocked==`true`].name").unwrap();
    assert_eq!(result, json!(["Acme"]));
}

#[test]
fn reshapes_into_an_object_projection() {
    let value = json!([{"name": "Acme", "state": "OK", "extra": "ignored"}]);
    let result = apply(value, "[].{n: name, s: state}").unwrap();
    assert_eq!(result, json!([{"n": "Acme", "s": "OK"}]));
}

#[test]
fn indexes_a_single_element() {
    let value = json!([{"name": "Acme"}, {"name": "Beta"}]);
    let result = apply(value, "[0]").unwrap();
    assert_eq!(result, json!({"name": "Acme"}));
}

#[test]
fn invalid_expression_is_a_clear_error() {
    let value = json!([]);
    let err = apply(value, "[").unwrap_err();
    assert!(err.to_string().contains("invalid --query expression"));
}

#[test]
fn nonexistent_field_yields_null_not_error() {
    // JMESPath semantics: a missing field projects to null, not a hard error --
    // matches AWS CLI's --query behavior.
    let value = json!({"name": "Acme"});
    let result = apply(value, "nope").unwrap();
    assert_eq!(result, serde_json::Value::Null);
}
