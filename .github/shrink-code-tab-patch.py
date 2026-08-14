from pathlib import Path

path = Path("apps/desktop/src/features/code/CodeTab.tsx")
text = path.read_text()


def replace_once(old: str, new: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected one replacement target, found {count}: {old[:80]!r}")
    text = text.replace(old, new, 1)


replace_once(
    '''  flushCodeWorkspaceDraft,
  stageCodeWorkspaceDrafts,
  subscribeCodeDraftPersistence,
''',
    '''  flushCodeWorkspaceDraft,
  stageCodeWorkspaceTabDrafts,
  subscribeCodeDraftPersistence,
  workspaceProjectFromDraftTab,
''',
)
replace_once(
    '''function workspaceTabProject(tab: EditorTab): string | null {
  if (tab.kind !== "workspace") return null;
  const prefix = "workspace:";
  const suffix = `:${tab.path}`;
  if (!tab.id.startsWith(prefix) || !tab.id.endsWith(suffix)) return null;
  return tab.id.slice(prefix.length, tab.id.length - suffix.length) || null;
}

function dirtyDraftSnapshots(project: string, tabs: EditorTab[]) {
  return tabs
    .filter((tab) => tab.kind === "workspace" && tab.dirty && tab.id === workspaceTabId(project, tab.path))
    .map((tab) => ({ path: tab.path, content: tab.content, baseFingerprint: tab.fingerprint }));
}

''',
    '''''',
)
replace_once(
    '''      stageCodeWorkspaceDrafts(previousProject, dirtyDraftSnapshots(previousProject, tabs));
''',
    '''      stageCodeWorkspaceTabDrafts(previousProject, tabs);
''',
)
replace_once(
    '''    stageCodeWorkspaceDrafts(projectName, dirtyDraftSnapshots(projectName, tabs));
''',
    '''    stageCodeWorkspaceTabDrafts(projectName, tabs);
''',
)
count = text.count("workspaceTabProject(")
if count != 3:
    raise RuntimeError(f"expected three workspaceTabProject uses, found {count}")
text = text.replace("workspaceTabProject(", "workspaceProjectFromDraftTab(")

path.write_text(text)
