use std::fs;

use repodesk_core::code_workspace::{
    CodeWorkspaceSaveInput, CodeWorkspaceSource, load_code_workspace, read_code_document,
    save_code_document,
};
use tempfile::tempdir;

#[test]
fn non_git_repository_falls_back_to_bounded_filesystem_index() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn answer() -> u8 { 42 }\n",
    )
    .unwrap();
    fs::write(dir.path().join("README.md"), "# fixture\n").unwrap();
    fs::write(dir.path().join("node_modules/pkg/index.js"), "generated\n").unwrap();

    let snapshot = load_code_workspace("fixture", dir.path()).unwrap();

    assert_eq!(snapshot.source, CodeWorkspaceSource::FilesystemFallback);
    assert!(snapshot.files.iter().any(|file| file.path == "src/lib.rs"));
    assert!(snapshot.files.iter().any(|file| file.path == "README.md"));
    assert!(
        snapshot
            .files
            .iter()
            .all(|file| !file.path.starts_with("node_modules/"))
    );
}

#[test]
fn save_requires_the_fingerprint_that_was_opened() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    let path = dir.path().join("src/lib.rs");
    fs::write(&path, "pub fn value() -> u8 { 1 }\n").unwrap();

    let opened = read_code_document(dir.path(), "src/lib.rs").unwrap();
    fs::write(&path, "pub fn value() -> u8 { 2 }\n").unwrap();

    let error = save_code_document(
        dir.path(),
        CodeWorkspaceSaveInput {
            path: "src/lib.rs".into(),
            content: "pub fn value() -> u8 { 3 }\n".into(),
            expected_fingerprint: opened.fingerprint,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("changed outside RepoDesk"));
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "pub fn value() -> u8 { 2 }\n"
    );
}

#[test]
fn binary_like_save_is_rejected_before_the_live_file_is_changed() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    let path = dir.path().join("src/lib.rs");
    let original = b"pub fn value() -> u8 { 1 }\n";
    fs::write(&path, original).unwrap();

    let opened = read_code_document(dir.path(), "src/lib.rs").unwrap();
    let error = save_code_document(
        dir.path(),
        CodeWorkspaceSaveInput {
            path: "src/lib.rs".into(),
            content: "pub fn value() -> u8 { 2 }\0trailing binary marker".into(),
            expected_fingerprint: opened.fingerprint,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("Binary-like content"));
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(
        read_code_document(dir.path(), "src/lib.rs")
            .unwrap()
            .content,
        String::from_utf8(original.to_vec()).unwrap()
    );
}

#[test]
fn successful_save_returns_a_new_clean_document_fingerprint() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    let path = dir.path().join("src/lib.rs");
    fs::write(&path, "pub fn value() -> u8 { 1 }\n").unwrap();

    let opened = read_code_document(dir.path(), "src/lib.rs").unwrap();
    let result = save_code_document(
        dir.path(),
        CodeWorkspaceSaveInput {
            path: "src/lib.rs".into(),
            content: "pub fn value() -> u8 { 2 }\n".into(),
            expected_fingerprint: opened.fingerprint.clone(),
        },
    )
    .unwrap();

    assert!(result.changed);
    assert_eq!(result.previous_fingerprint, opened.fingerprint);
    assert_ne!(result.document.fingerprint, result.previous_fingerprint);
    assert_eq!(result.document.content, "pub fn value() -> u8 { 2 }\n");
    assert_eq!(fs::read_to_string(path).unwrap(), result.document.content);
}

#[cfg(unix)]
#[test]
fn atomic_save_preserves_existing_unix_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    let path = dir.path().join("src/script.sh");
    fs::write(&path, "#!/bin/sh\necho before\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();

    let opened = read_code_document(dir.path(), "src/script.sh").unwrap();
    save_code_document(
        dir.path(),
        CodeWorkspaceSaveInput {
            path: "src/script.sh".into(),
            content: "#!/bin/sh\necho after\n".into(),
            expected_fingerprint: opened.fingerprint,
        },
    )
    .unwrap();

    assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o777, 0o755);
}

#[test]
fn sensitive_paths_are_not_editable_even_inside_the_project() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join(".env"), "API_KEY=not-a-real-key\n").unwrap();

    let error = read_code_document(dir.path(), ".env").unwrap_err();
    assert!(error.to_string().contains("blocked"));
}
