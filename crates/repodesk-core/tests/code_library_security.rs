use std::fs;
use std::path::Path;

use chrono::{Duration, TimeZone, Utc};
use repodesk_core::code_library::{
    CodeLibraryGrantRequest, CodeLibraryRegistry, CodeLibraryRoot, reject_library_save,
};
use repodesk_core::code_workspace::MAX_EDITABLE_FILE_BYTES;
use tempfile::TempDir;

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

struct Fixture {
    _temp: TempDir,
    project: std::path::PathBuf,
    dependency_root: std::path::PathBuf,
    registry: CodeLibraryRegistry,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        let dependency_root = project.join("node_modules");
        fs::create_dir_all(&dependency_root).expect("dependency root");
        Self {
            _temp: temp,
            project,
            dependency_root,
            registry: CodeLibraryRegistry::default(),
        }
    }

    fn issue(
        &self,
        uri: String,
    ) -> repodesk_core::errors::RepoDeskResult<repodesk_core::code_library::CodeLibraryDefinition>
    {
        self.registry.issue_definition(CodeLibraryGrantRequest {
            project: "RepoDesk".into(),
            server_id: "typescript-language-server".into(),
            project_root: self.project.clone(),
            uri,
            allowed_roots: vec![CodeLibraryRoot {
                label: "node_modules".into(),
                path: self.dependency_root.clone(),
            }],
            issued_at: Utc.with_ymd_and_hms(2026, 8, 10, 8, 0, 0).unwrap(),
        })
    }
}

#[test]
fn reads_only_a_definition_inside_an_approved_dependency_root() {
    let fixture = Fixture::new();
    let declaration = fixture.dependency_root.join("pkg/index.d.ts");
    fs::create_dir_all(declaration.parent().unwrap()).unwrap();
    fs::write(&declaration, "export declare const ready: boolean;\n").unwrap();

    let definition = fixture
        .issue(file_uri(&declaration))
        .expect("definition grant");
    assert_eq!(definition.display_path, "node_modules/pkg/index.d.ts");
    assert_eq!(definition.language, "typescript");

    let document = fixture
        .registry
        .read_at(
            "RepoDesk",
            &definition.handle,
            Utc.with_ymd_and_hms(2026, 8, 10, 8, 1, 0).unwrap(),
        )
        .expect("read library document");
    assert_eq!(document.content, "export declare const ready: boolean;\n");
    assert_eq!(document.display_path, "node_modules/pkg/index.d.ts");
    assert!(document.read_only);
}

#[test]
fn rejects_non_file_uris_traversal_and_arbitrary_home_files() {
    let fixture = Fixture::new();
    let outside = fixture.project.parent().unwrap().join("notes.ts");
    fs::write(&outside, "export const privateNote = true;").unwrap();

    assert!(
        fixture
            .issue("https://example.test/index.d.ts".into())
            .is_err()
    );
    assert!(
        fixture
            .issue(file_uri(&fixture.dependency_root.join("../src/lib.ts")))
            .is_err()
    );
    assert!(fixture.issue(file_uri(&outside)).is_err());
}

#[test]
fn rejects_sensitive_unsupported_and_oversized_library_files() {
    let fixture = Fixture::new();
    let sensitive = fixture.dependency_root.join("pkg/credentials.json");
    let binary = fixture.dependency_root.join("pkg/image.bin");
    let oversized = fixture.dependency_root.join("pkg/large.ts");
    fs::create_dir_all(sensitive.parent().unwrap()).unwrap();
    fs::write(&sensitive, "{}\n").unwrap();
    fs::write(&binary, [0_u8, 1, 2]).unwrap();
    fs::write(&oversized, vec![b'x'; MAX_EDITABLE_FILE_BYTES as usize + 1]).unwrap();

    assert!(fixture.issue(file_uri(&sensitive)).is_err());
    assert!(fixture.issue(file_uri(&binary)).is_err());
    assert!(fixture.issue(file_uri(&oversized)).is_err());
}

#[test]
fn rejects_expired_cross_project_handles_and_all_save_attempts() {
    let fixture = Fixture::new();
    let declaration = fixture.dependency_root.join("pkg/index.d.ts");
    fs::create_dir_all(declaration.parent().unwrap()).unwrap();
    fs::write(&declaration, "export declare const ready: boolean;\n").unwrap();
    let definition = fixture.issue(file_uri(&declaration)).unwrap();
    let issued_at = Utc.with_ymd_and_hms(2026, 8, 10, 8, 0, 0).unwrap();

    assert!(
        fixture
            .registry
            .read_at("Other", &definition.handle, issued_at)
            .is_err()
    );
    assert!(
        fixture
            .registry
            .read_at(
                "RepoDesk",
                &definition.handle,
                issued_at + Duration::minutes(11)
            )
            .is_err()
    );
    assert!(reject_library_save(&definition.handle).is_err());
}
