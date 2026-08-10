use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::code_workspace::CodeWorkspaceFile;

use super::{MAX_RELATIONS, safe_read_index_text};

const MAX_SCRIPT_FILES: usize = 6_000;
const MAX_SCRIPT_INDEX_BYTES: u64 = 48 * 1024 * 1024;
const MAX_IMPORT_SCAN_BYTES: usize = 4 * 1024;
const SCRIPT_EXTENSIONS: [&str; 6] = ["ts", "tsx", "js", "jsx", "mjs", "cjs"];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ScriptImport {
    specifier: String,
    reason: String,
}

#[derive(Debug, Clone)]
pub(super) struct ScriptIndex {
    facts: BTreeMap<String, Vec<ScriptImport>>,
    pub(super) truncated: bool,
}

pub(super) fn index_script_files(
    root: &Path,
    files: &[CodeWorkspaceFile],
    all_paths: &BTreeSet<String>,
) -> ScriptIndex {
    let mut facts = BTreeMap::new();
    let mut indexed_bytes = 0_u64;
    let mut truncated = false;

    for file in files.iter().filter(|file| {
        matches!(file.language.as_str(), "typescript" | "javascript") && !file.blocked
    }) {
        if facts.len() >= MAX_SCRIPT_FILES
            || indexed_bytes.saturating_add(file.bytes) > MAX_SCRIPT_INDEX_BYTES
        {
            truncated = true;
            break;
        }
        if !all_paths.contains(&file.path) {
            continue;
        }
        let Some(source) = safe_read_index_text(root, &file.path) else {
            continue;
        };

        indexed_bytes = indexed_bytes.saturating_add(file.bytes);
        facts.insert(file.path.clone(), import_specifiers(&source));
    }

    ScriptIndex { facts, truncated }
}

pub(super) fn extend_dependency_map(
    dependencies: &mut BTreeMap<String, Vec<super::RepositoryRelation>>,
    index: &ScriptIndex,
    all_paths: &BTreeSet<String>,
) {
    for (source, imports) in &index.facts {
        let mut relations = BTreeMap::<String, BTreeSet<String>>::new();
        for import in imports {
            let Some(target) = resolve_script_import(source, &import.specifier, all_paths) else {
                continue;
            };
            if target == *source {
                continue;
            }
            relations
                .entry(target)
                .or_default()
                .insert(import.reason.clone());
        }

        dependencies.insert(
            source.clone(),
            relations
                .into_iter()
                .take(MAX_RELATIONS)
                .map(|(path, reasons)| super::RepositoryRelation {
                    path,
                    reason: reasons.into_iter().collect::<Vec<_>>().join(", "),
                })
                .collect(),
        );
    }
}

fn import_specifiers(source: &str) -> Vec<ScriptImport> {
    let bytes = source.as_bytes();
    let mut imports = BTreeSet::new();
    let mut cursor = 0_usize;

    while cursor < bytes.len() {
        cursor = skip_trivia(bytes, cursor);
        if cursor >= bytes.len() {
            break;
        }

        match bytes[cursor] {
            b'\'' | b'"' | b'`' => {
                cursor = skip_string(bytes, cursor);
                continue;
            }
            value if is_identifier_start(value) => {
                let start = cursor;
                let (identifier, next) = read_identifier(bytes, cursor);
                cursor = next;

                match identifier {
                    "import" if !is_member_access(bytes, start) => {
                        if let Some((import, next)) = parse_import(bytes, cursor) {
                            imports.insert(import);
                            cursor = next;
                        }
                    }
                    "export" if !is_member_access(bytes, start) => {
                        if let Some((import, next)) = parse_export(bytes, cursor) {
                            imports.insert(import);
                            cursor = next;
                        }
                    }
                    "require" if !is_member_access(bytes, start) => {
                        if let Some((specifier, next)) = parse_literal_call(bytes, cursor) {
                            imports.insert(ScriptImport {
                                reason: format!("require {specifier}"),
                                specifier,
                            });
                            cursor = next;
                        }
                    }
                    _ => {}
                }
            }
            _ => cursor += 1,
        }
    }

    imports.into_iter().collect()
}

fn parse_import(bytes: &[u8], cursor: usize) -> Option<(ScriptImport, usize)> {
    let cursor = skip_trivia(bytes, cursor);
    match bytes.get(cursor).copied()? {
        b'(' => {
            let (specifier, next) = parse_literal_call_from_open_paren(bytes, cursor)?;
            Some((
                ScriptImport {
                    reason: format!("dynamic import {specifier}"),
                    specifier,
                },
                next,
            ))
        }
        b'\'' | b'"' => {
            let (specifier, next) = read_quoted(bytes, cursor)?;
            Some((
                ScriptImport {
                    reason: format!("import {specifier}"),
                    specifier,
                },
                next,
            ))
        }
        b'.' => None, // import.meta
        _ => find_from_specifier(bytes, cursor, "import"),
    }
}

fn parse_export(bytes: &[u8], cursor: usize) -> Option<(ScriptImport, usize)> {
    let cursor = skip_trivia(bytes, cursor);
    match bytes.get(cursor).copied()? {
        b'{' | b'*' => find_from_specifier(bytes, cursor, "export from"),
        value if is_identifier_start(value) => {
            let (identifier, next) = read_identifier(bytes, cursor);
            if identifier != "type" {
                return None;
            }
            let next = skip_trivia(bytes, next);
            if !matches!(bytes.get(next), Some(b'{') | Some(b'*')) {
                return None;
            }
            find_from_specifier(bytes, next, "export type from")
        }
        _ => None,
    }
}

fn find_from_specifier(
    bytes: &[u8],
    cursor: usize,
    reason_prefix: &str,
) -> Option<(ScriptImport, usize)> {
    let limit = bytes.len().min(cursor.saturating_add(MAX_IMPORT_SCAN_BYTES));
    let mut cursor = cursor;

    while cursor < limit {
        cursor = skip_trivia(bytes, cursor);
        if cursor >= limit || bytes.get(cursor) == Some(&b';') {
            return None;
        }
        match bytes[cursor] {
            b'\'' | b'"' | b'`' => cursor = skip_string(bytes, cursor),
            value if is_identifier_start(value) => {
                let (identifier, next) = read_identifier(bytes, cursor);
                cursor = next;
                if identifier == "from" {
                    let quote = skip_trivia(bytes, cursor);
                    let (specifier, next) = read_quoted(bytes, quote)?;
                    return Some((
                        ScriptImport {
                            reason: format!("{reason_prefix} {specifier}"),
                            specifier,
                        },
                        next,
                    ));
                }
            }
            _ => cursor += 1,
        }
    }
    None
}

fn parse_literal_call(bytes: &[u8], cursor: usize) -> Option<(String, usize)> {
    let cursor = skip_trivia(bytes, cursor);
    if bytes.get(cursor) != Some(&b'(') {
        return None;
    }
    parse_literal_call_from_open_paren(bytes, cursor)
}

fn parse_literal_call_from_open_paren(bytes: &[u8], open: usize) -> Option<(String, usize)> {
    let quote = skip_trivia(bytes, open + 1);
    let (specifier, next) = read_quoted(bytes, quote)?;
    let close = skip_trivia(bytes, next);
    if bytes.get(close) != Some(&b')') {
        return None;
    }
    Some((specifier, close + 1))
}

fn read_quoted(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    let quote = *bytes.get(start)?;
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }
    let content_start = start + 1;
    let mut cursor = content_start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => return None, // escaped specifiers are intentionally not guessed
            value if value == quote => {
                let value = std::str::from_utf8(&bytes[content_start..cursor])
                    .ok()?
                    .to_string();
                return Some((value, cursor + 1));
            }
            b'\n' | b'\r' => return None,
            _ => cursor += 1,
        }
    }
    None
}

fn skip_trivia(bytes: &[u8], mut cursor: usize) -> usize {
    loop {
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor..cursor + 2) == Some(b"//") {
            cursor += 2;
            while bytes.get(cursor).is_some_and(|value| *value != b'\n') {
                cursor += 1;
            }
            continue;
        }
        if bytes.get(cursor..cursor + 2) == Some(b"/*") {
            cursor += 2;
            while cursor + 1 < bytes.len() && bytes.get(cursor..cursor + 2) != Some(b"*/") {
                cursor += 1;
            }
            cursor = (cursor + 2).min(bytes.len());
            continue;
        }
        return cursor;
    }
}

fn skip_string(bytes: &[u8], start: usize) -> usize {
    let Some(&quote) = bytes.get(start) else {
        return start;
    };
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = (cursor + 2).min(bytes.len()),
            value if value == quote => return cursor + 1,
            _ => cursor += 1,
        }
    }
    bytes.len()
}

fn read_identifier(bytes: &[u8], start: usize) -> (&str, usize) {
    let mut cursor = start + 1;
    while bytes
        .get(cursor)
        .is_some_and(|value| is_identifier_continue(*value))
    {
        cursor += 1;
    }
    (
        std::str::from_utf8(&bytes[start..cursor]).unwrap_or_default(),
        cursor,
    )
}

fn is_identifier_start(value: u8) -> bool {
    value.is_ascii_alphabetic() || matches!(value, b'_' | b'$')
}

fn is_identifier_continue(value: u8) -> bool {
    is_identifier_start(value) || value.is_ascii_digit()
}

fn is_member_access(bytes: &[u8], start: usize) -> bool {
    bytes[..start]
        .iter()
        .rev()
        .copied()
        .find(|value| !value.is_ascii_whitespace())
        == Some(b'.')
}

fn resolve_script_import(
    current: &str,
    specifier: &str,
    all_paths: &BTreeSet<String>,
) -> Option<String> {
    let specifier = specifier
        .split(['?', '#'])
        .next()
        .unwrap_or(specifier)
        .trim();
    if !(specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier == "."
        || specifier == "..")
    {
        return None;
    }

    let normalized = normalize_relative_import(current, specifier)?;
    if all_paths.contains(&normalized) {
        return Some(normalized);
    }

    if Path::new(&normalized).extension().is_some() {
        return None;
    }

    for extension in SCRIPT_EXTENSIONS {
        let candidate = format!("{normalized}.{extension}");
        if all_paths.contains(&candidate) {
            return Some(candidate);
        }
    }
    for extension in SCRIPT_EXTENSIONS {
        let candidate = format!("{normalized}/index.{extension}");
        if all_paths.contains(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn normalize_relative_import(current: &str, specifier: &str) -> Option<String> {
    let parent = current
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let mut parts = parent
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    for part in specifier.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            value => parts.push(value.to_string()),
        }
    }

    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_supported_literal_import_forms_without_comment_or_string_noise() {
        let source = r#"
            import value from "./value";
            import type { Shape } from '../types';
            import "./setup";
            export { helper } from "./helper";
            export type { Shape as PublicShape } from "./public-types";
            const lazy = import("./lazy");
            const legacy = require("./legacy");
            const noise = "require('./string-noise')";
            export const from = "./not-a-reexport";
            // import ignored from "./comment";
            /* const ignored = require("./block-comment"); */
            object.require("./member-call");
            console.log(import.meta.url);
        "#;

        let values = import_specifiers(source);
        let specifiers = values
            .iter()
            .map(|value| value.specifier.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            specifiers,
            BTreeSet::from([
                "../types",
                "./helper",
                "./lazy",
                "./legacy",
                "./public-types",
                "./setup",
                "./value",
            ])
        );
        assert!(
            values
                .iter()
                .any(|value| value.reason == "dynamic import ./lazy")
        );
        assert!(
            values
                .iter()
                .any(|value| value.reason == "require ./legacy")
        );
    }

    #[test]
    fn resolves_relative_script_files_and_index_modules() {
        let paths = BTreeSet::from([
            "src/app.tsx".to_string(),
            "src/lib/value.ts".to_string(),
            "src/widgets/index.tsx".to_string(),
            "src/styles.css".to_string(),
        ]);

        assert_eq!(
            resolve_script_import("src/app.tsx", "./lib/value", &paths).as_deref(),
            Some("src/lib/value.ts")
        );
        assert_eq!(
            resolve_script_import("src/app.tsx", "./widgets", &paths).as_deref(),
            Some("src/widgets/index.tsx")
        );
        assert_eq!(
            resolve_script_import("src/app.tsx", "./styles.css?raw", &paths).as_deref(),
            Some("src/styles.css")
        );
        assert_eq!(resolve_script_import("src/app.tsx", "react", &paths), None);
    }

    #[test]
    fn relative_resolution_never_escapes_repository_root() {
        let paths = BTreeSet::from(["outside.ts".to_string()]);
        assert_eq!(
            resolve_script_import("src/deep/app.ts", "../../../outside", &paths),
            None
        );
    }
}
