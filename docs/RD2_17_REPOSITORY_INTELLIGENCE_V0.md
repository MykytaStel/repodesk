# RD2-17 — Repository Intelligence v0

> Implementation sequence note: the original RepoDesk 2 migration table listed
> Repo Intelligence as RD2-16 before the Semantic Code Surface slice was inserted.
> This document uses the current implementation sequence: Repository Intelligence
> follows the merged Semantic Code Surface.

## Purpose

Repository Intelligence answers a bounded set of engineering questions around the
file the user is actually working on:

```text
active file
  -> what local modules does it depend on?
  -> what local modules depend on it?
  -> which tests are the closest deterministic candidates?
  -> which files historically change with it?
  -> which files are explainable context candidates?
```

It is not a repository-wide knowledge graph UI and it is not an AI similarity
search. The first version is deliberately deterministic, bounded and inspectable.

## Product boundary

Repository Intelligence is a supporting capability for existing RepoDesk surfaces:

```text
Repository Intelligence
        |
        +--> Code / Repo context
        +--> future Context Builder ranking
        +--> future verification/test suggestions
        +--> future agent coordination intelligence
```

It does **not** add a permanent Activity Rail item.

The Code workspace owns the user-facing entry point. `Repo context` opens a
right-side overlay and the intelligence query is not started while that overlay
is closed.

## Sources of truth

### Rust local relationships

Rust module/import relationships come from `syn` AST parsing.

Supported in v0:

- external module declarations such as `mod foo;`
- `crate::...` use paths
- `self::...` use paths
- repeated `super::...` use paths
- grouped imports such as `use crate::engineering::{events, knowledge::Store};`
- local module resolution to `foo.rs` or `foo/mod.rs`

RepoDesk only emits a dependency edge when a matching visible repository file
exists. It does not infer an edge from a textual token alone.

### Git co-change evidence

Co-change relationships come from bounded local Git history:

```text
git log
  --no-merges
  latest 200 commits
  max 200 files per sampled commit
```

For a focus file, RepoDesk counts how often another path appears in the same
sampled commit. The UI shows both values:

```text
src/foo.rs  4 / 9
```

Meaning: `src/foo.rs` changed together with the focus file in 4 of the 9 sampled
commits that contained the focus file.

This is historical evidence, not proof of a dependency.

### Closest tests

Test candidates are deterministic topology signals. Current scoring:

| Signal | Score |
| --- | ---: |
| inline Rust `#[cfg(test)]` | 100 |
| test file directly depends on focus | 100 |
| test filename matches focus module | 88 |
| test in same directory | 72 |
| test in same crate/package area | 48 |

The score is only a ranking aid. It is not verification evidence.

### Context candidates

Repository Intelligence combines the explainable signals above into a bounded
candidate list:

| Signal | Base score |
| --- | ---: |
| dependency | 92 |
| dependent | 84 |
| closest test | >= 80 |
| co-change evidence | 45–70 |

Every candidate retains one or more textual reasons.

`context_candidates` are advisory only. This slice **does not** insert them into
the Work Item Context Manifest.

## Work Item and context safety

The Work Item Contract remains authoritative.

```text
Repository Intelligence candidate
             |
             v
       advisory ranking
             |
      +------+------+
      |             |
allowed scope   outside scope
      |             |
future bounded  never silently
context use     auto-included
```

A future Context Builder may use repository intelligence to rank files that are
already admissible by the typed Work Item scope, or surface an explicit request
to expand scope. Intelligence must not silently bypass `allowed_paths`,
`protected_paths`, security filtering, or token budgets.

## Focus-driven architecture

v0 deliberately avoids a persistent full-repository graph.

The core receives an optional focus path and returns one bounded neighborhood:

```rust
RepositoryIntelligenceSnapshot
  project
  index accounting
  focus: RepositoryFileIntelligence

RepositoryFileIntelligence
  dependencies[]
  dependents[]
  closest_tests[]
  co_changes[]
  context_candidates[]
```

The current budgets are:

```text
Rust files parsed          <= 4,000
Rust source indexed        <= 32 MiB
single AST-indexed file    <= 384 KiB
Git commits sampled        <= 200
files per sampled commit   <= 200
relations per direction    <= 24
closest tests              <= 12
co-change results          <= 12
context candidates         <= 24
```

The existing Code Workspace remains responsible for repository visibility and
blocked-path classification.

## Filesystem safety

Repository Intelligence must not become a second permissive file reader.

Before reading Rust source for AST indexing, v0 checks:

1. the file is visible through Code Workspace;
2. the path is not blocked;
3. `symlink_metadata` does not identify a symlink;
4. the canonical file remains inside the canonical project root;
5. the target is a regular file;
6. the intelligence-specific file-size cap is respected;
7. the file is valid text and contains no NUL byte.

Tracked symlinks therefore cannot make the indexer read source outside the
active project.

## UI contract

Code adds one contextual action:

```text
Repo context
```

It opens an overlay with:

- Dependencies
- Dependents
- Closest tests
- Co-change history
- Context candidates

Rows navigate through the existing guarded Code Workspace open request. The
drawer does not gain direct filesystem authority.

Only one right-side context overlay is visible at a time: Repo context and
RepoPilot Findings are mutually exclusive.

## Definition navigation

Semantic Code Surface already supports Rust definitions through `rust-analyzer`
and `F12`.

This slice adds positional definition navigation for modifier-click:

```text
macOS:        Cmd + click
Windows/Linux Ctrl + click
        |
        v
CodeMirror document position
        |
        v
same rust-analyzer textDocument/definition action as F12
        |
        v
RepoDesk guarded file:line:column navigation
```

There is no regex fallback. If the language service cannot prove a definition,
RepoDesk does not invent one.

This covers Rust variables, functions, types, modules and imports wherever
`rust-analyzer` returns a definition.

## Shell, `.gitignore`, and special files

The Code Workspace already admits safe UTF-8 files independently of parser
support. In particular:

- `.sh` / `.bash` / `.zsh` are identified as `shell`;
- `.gitignore` is readable/editable as a safe text file today;
- special-file syntax adapters are separate from file access.

Repository Intelligence v0 does not parse shell or `.gitignore` semantics.
Adding shell syntax highlighting or shell-language navigation later must use a
real parser/language adapter rather than treating regex matches as symbols.

Likewise, TypeScript/JavaScript already have parser-backed syntax highlighting,
but symbol-definition navigation should be enabled through a real TS language
service before RepoDesk claims semantic navigation parity.

## Performance contract

- no polling is added;
- Repository Intelligence is lazy in Code;
- AST/Git work runs through `spawn_blocking` rather than the async UI command;
- only bounded source is parsed;
- Git history is bounded;
- only the focused neighborhood is transported to React;
- no embeddings or model calls are used;
- no repository intelligence artifact is persisted in v0.

Saving a file invalidates the repository-intelligence query namespace so an open
or subsequently reopened drawer can rebuild from the new on-disk state.

## Evidence semantics

The UI must keep these meanings distinct:

```text
AST relationship       = statically resolved local module relation
co-change              = historical correlation
closest test           = deterministic candidate
context candidate      = advisory ranking
LSP definition         = language-server navigation result
VerificationReceipt    = canonical verification evidence
```

None of the first four may be presented as a successful verification result.

## Non-goals / known limitations

v0 intentionally does not cover:

- TypeScript/JavaScript import graph;
- Shell AST/import graph;
- `.gitignore` syntax parser;
- Python/Go/C++ semantic graphs;
- Rust `#[path = "..."]` module overrides;
- generated modules created only by build scripts/macros;
- external crate dependency internals;
- persistent whole-repository graph storage;
- graph visualization as a permanent workspace;
- embeddings/LLM similarity scoring;
- automatic mutation of Work Item scope;
- automatic context inclusion;
- Git paths containing embedded newlines in co-change parsing.

These constraints are preferable to emitting relationships that RepoDesk cannot
support with evidence.

## Next extensions

The next useful layers should build on this contract rather than introduce a
second repository model:

1. TypeScript/JavaScript language-service definitions and import edges.
2. Shell and special-file syntax adapters where justified by bundle/runtime cost.
3. Context Builder ranking constrained by Work Item scope.
4. symbol/reference neighborhoods from language servers.
5. test command suggestions linked to Task Runner evidence.
6. Engineering Knowledge relevance as an additional explained context signal.
7. Agent Coordination Intelligence consuming the same bounded repository facts.
