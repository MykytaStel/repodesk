use repodesk_core::project_checks::{
    default_checks_for_project_type, normalize_legacy_commands, stable_check_id,
};
use repodesk_core::projects::ProjectConfig;

#[test]
fn legacy_commands_normalize_without_index_based_ids() {
    let first = normalize_legacy_commands(vec!["cargo test --all".into()]).unwrap();
    let reordered = normalize_legacy_commands(vec![
        "cargo fmt --all -- --check".into(),
        "cargo test --all".into(),
    ])
    .unwrap();

    assert_eq!(first[0].id, reordered[1].id);
    assert_eq!(first[0].title, "cargo test --all");
    assert_eq!(first[0].timeout_secs, 120);
    assert!(!first[0].required);
    assert!(first[0].relevant_paths.is_empty());
}

#[test]
fn legacy_project_toml_loads_into_normalized_descriptors() {
    let config: ProjectConfig = toml::from_str(
        r#"
name = "demo"
path = "/tmp/demo"
project_type = "rust"
main_language = "rust"
checks = ["cargo test --all"]
context_ignore = []
created_at = "2026-09-07T00:00:00Z"
updated_at = "2026-09-07T00:00:00Z"
"#,
    )
    .unwrap();

    assert_eq!(config.checks.len(), 1);
    assert_eq!(config.checks[0].id, stable_check_id("cargo test --all"));
    assert_eq!(config.checks[0].kind, "test");
}

#[test]
fn structured_project_check_keeps_explicit_metadata() {
    let config: ProjectConfig = toml::from_str(
        r#"
name = "demo"
path = "/tmp/demo"
project_type = "rust"
main_language = "rust"
checks = [{ id = "unit-tests", title = "Unit tests", command = "cargo test --lib", kind = "test", required = true, relevant_paths = ["crates/repodesk-core"], timeout_secs = 45 }]
context_ignore = []
created_at = "2026-09-07T00:00:00Z"
updated_at = "2026-09-07T00:00:00Z"
"#,
    )
    .unwrap();

    let check = &config.checks[0];
    assert_eq!(check.id, "unit-tests");
    assert_eq!(check.title, "Unit tests");
    assert_eq!(check.kind, "test");
    assert!(check.required);
    assert_eq!(check.relevant_paths, vec!["crates/repodesk-core"]);
    assert_eq!(check.timeout_secs, 45);
}

#[test]
fn rust_tauri_receives_rust_defaults_for_new_projects() {
    let checks = default_checks_for_project_type("rust-tauri");

    assert_eq!(checks.len(), 3);
    assert!(checks.iter().all(|check| check.timeout_secs == 120));
    assert!(checks.iter().any(|check| check.kind == "test"));
}
