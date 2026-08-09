# RD2 IDE Polish v1

This slice is the ergonomic follow-up to the Work/Changes density reset.

It addresses visible friction without introducing a new permanent product surface.

## Goals

- make the lightweight Code editor read like an IDE rather than a textarea with a number strip;
- normalize spacing across the focus workspace;
- make project switching compact and keyboard-first;
- make clean/empty states intentional instead of looking unfinished;
- preserve the runtime-budget rule that stable workspace state should be quiet.

## Editor gutter

The gutter keeps one preformatted text node for all line numbers rather than rendering one React element per source line.

That matters for large files: visual polish must not create thousands of extra DOM nodes.

The editor and gutter now share exact metrics:

```text
font size      12.5px
line height    20px
top padding    12px
gutter width   62px desktop / 54px narrow
```

A single overlay marker highlights the active line number. Its position is updated from the editor scroll offset through a CSS custom property, so scrolling does not require a React state update for every scroll event.

The gutter also gets:

- stronger separation from the document;
- tabular line-number digits;
- more right padding;
- a wider readable strip;
- exact vertical alignment with source text.

## Code chrome

The document toolbar and tabs receive a small spacing pass:

- document toolbar height: 40px;
- workspace toolbar height: 40px;
- open-file strip: 36px;
- path display is plain text rather than inheriting generic inline-code chrome;
- source content gets more left/right breathing room;
- status bar spacing is normalized.

No editor engine or filesystem contract changes are included.

## Project switcher

The previous switcher mixed project choices and repository-management actions in one large menu.

The new hierarchy is:

```text
[ active-project ▾ ]

Search projects…
──────────────────
✓ active-project       RUST-TAURI
  project-two          RUST-CLI
  project-three        NEXT.JS
──────────────────
Open folder… | Connect project…
──────────────────
↑↓ navigate  ↵ open  esc close
```

### Interaction contract

- connected-project data loads only when the popover is opened;
- search matches project name, path and project type;
- Arrow Up / Arrow Down changes the highlighted project;
- Enter activates the highlighted project;
- Escape closes the popover;
- outside click closes the popover;
- choosing the already-active project simply closes the popover;
- project-management actions are visually separated from project choices.

A project switch still invalidates the whole query cache intentionally because it re-scopes the entire workspace.

## Spacing contract

Primary focus surfaces use a consistent rhythm rather than local ad-hoc margins.

The polish layer normalizes:

- main workbench canvas padding;
- sidebar/inspector padding;
- Work phase/header spacing;
- current-step and CTA spacing;
- Changes header/gate/pane padding;
- Changes preview content padding;
- narrow-screen reductions.

This remains a density-oriented IDE layout, not a return to large dashboard cards.

## Empty states

Changes now distinguishes between a problem and a clean repository.

A clean file list reads as:

```text
Working tree is clean
No changed files in this project.
Edit in Code or start a Work Item to produce a ChangeSet.
```

The preview pane similarly uses `Nothing to review` when the working tree is clean.

## Performance boundaries

This UI pass deliberately keeps the performance rules from the previous slices:

- no fixed polling is added;
- project-list loading is on demand;
- line numbers remain one text node, not N React rows;
- editor scroll does not update React state solely to move the active gutter marker;
- no source code is persisted in browser storage;
- Code save/fingerprint/security semantics are unchanged.

## Non-goals

This slice does not:

- add Monaco or CodeMirror;
- implement syntax highlighting;
- implement LSP diagnostics;
- change Work Item / ChangeSet semantics;
- change PTY lifetime;
- redesign the activity rail;
- add another permanent navigation destination.

## Manual verification

### Code

1. Open a file with at least 30 lines.
2. Confirm the gutter is visibly wider and separated from source text.
3. Scroll vertically and confirm line numbers stay aligned with source lines.
4. Move the caret through different lines and confirm the active line number follows it.
5. Confirm horizontal source scrolling never moves the gutter.
6. Confirm Edit / Diff / Save remain aligned in the document toolbar.
7. Confirm the path no longer renders as an oversized generic inline-code pill.

### Project switcher

1. Open the workspace drawer.
2. Confirm the active-project trigger is compact.
3. Open it and confirm focus enters Search automatically.
4. Type part of a project name or project type and confirm filtering.
5. Navigate with Arrow Up / Arrow Down and activate with Enter.
6. Close with Escape and outside click.
7. Confirm Open folder and Connect project live in a separate footer area.
8. Confirm project-list IPC is not required merely because the drawer exists; it is loaded when the switcher opens.

### Changes

1. Open Changes on a clean repository.
2. Confirm the file pane has a deliberate clean-state message with padding.
3. Confirm the preview reads `Nothing to review` rather than looking broken.
4. Modify a file and refresh; normal master/detail behavior should remain unchanged.

### General spacing

Review Work, Changes, Code and the workspace drawer at desktop width and a narrow window. No primary content should touch panel edges or appear to lose spacing when switching surfaces.
