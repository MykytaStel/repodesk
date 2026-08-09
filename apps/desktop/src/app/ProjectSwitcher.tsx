import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { projectUse, projectAdd } from "../shared/api/workflow";
import { pickDirectory, basename } from "../shared/api/dialog";
import { useToast } from "../shared/ui/Toast";
import { CheckIcon, ChevronIcon, FolderIcon, PlusIcon } from "./NavIcons";

type ProjectConfig = { name: string; path?: string; project_type?: string };

function projectTypeLabel(value?: string): string {
  if (!value) return "";
  return value.replace(/[_\s]+/g, "-").toUpperCase();
}

/**
 * Compact workspace-context switcher. The connected-project registry stays
 * secondary to the active engineering surface: one small trigger opens a
 * searchable, keyboard-friendly popover only when the user asks for it.
 * Switching project re-scopes the entire application, so broad invalidation is
 * intentional here even though routine workflow mutations use domain scopes.
 */
export function ProjectSwitcher({ projectName, onConnectProject }: { projectName: string; onConnectProject: () => void }) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [highlightedIndex, setHighlightedIndex] = useState(0);
  const ref = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const { data: projects = [], isFetching } = useQuery({
    queryKey: ["project_list_configs"],
    queryFn: () => invoke<ProjectConfig[]>("project_list_configs").catch(() => [] as ProjectConfig[]),
    enabled: open,
    staleTime: 60_000,
  });

  const filteredProjects = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return projects;
    return projects.filter((project) =>
      [project.name, project.path ?? "", project.project_type ?? ""]
        .some((value) => value.toLowerCase().includes(query)),
    );
  }, [projects, search]);

  const switchProject = useMutation({
    mutationFn: (name: string) => projectUse(name),
    onSuccess: (result, name) => {
      if (!result.ok) {
        toast.error(result.stderr || "Could not switch project");
        return;
      }
      toast.success(`Switched to ${name}`);
      setOpen(false);
      void queryClient.invalidateQueries();
    },
    onError: (error: any) => toast.error(error?.message || "Could not switch project"),
  });

  const openFromFolder = useMutation({
    mutationFn: async () => {
      const path = await pickDirectory("Open project folder");
      if (!path) return null;
      const name = basename(path);
      const added = await projectAdd({ name, path, project_type: "", main_language: null });
      const alreadyExists = !added.ok && /already exists/i.test(added.stderr);
      if (!added.ok && !alreadyExists) throw new Error(added.stderr || "Could not open folder");
      const activated = await projectUse(name);
      if (!activated.ok) throw new Error(activated.stderr || "Could not activate project");
      return name;
    },
    onSuccess: (name) => {
      if (!name) return;
      toast.success(`Opened ${name}`);
      setOpen(false);
      void queryClient.invalidateQueries();
    },
    onError: (error: any) => toast.error(error?.message || "Could not open folder"),
  });

  const selectProject = (project: ProjectConfig) => {
    if (project.name === projectName) {
      setOpen(false);
      return;
    }
    switchProject.mutate(project.name);
  };

  useEffect(() => {
    if (!open) return;
    setSearch("");
    setHighlightedIndex(0);
    requestAnimationFrame(() => searchRef.current?.focus());
  }, [open]);

  useEffect(() => {
    setHighlightedIndex((current) => Math.min(current, Math.max(0, filteredProjects.length - 1)));
  }, [filteredProjects.length]);

  useEffect(() => {
    if (!open) return;
    const onClick = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) setOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("mousedown", onClick);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("mousedown", onClick);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  const handleSearchKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    const lastIndex = Math.max(0, filteredProjects.length - 1);
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setHighlightedIndex((current) => Math.min(lastIndex, current + 1));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setHighlightedIndex((current) => Math.max(0, current - 1));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const project = filteredProjects[highlightedIndex];
      if (project) selectProject(project);
    }
  };

  return (
    <div className="project-switcher" ref={ref}>
      <button
        type="button"
        className={`project-switcher-trigger${open ? " open" : ""}`}
        onClick={() => setOpen((value) => !value)}
        aria-haspopup="dialog"
        aria-expanded={open}
      >
        <span className="project-switcher-current">{projectName || "Select project"}</span>
        <span className="project-switcher-caret" aria-hidden="true">
          <ChevronIcon open={open} />
        </span>
      </button>

      {open ? (
        <div className="project-switcher-menu" role="dialog" aria-label="Switch project">
          <div className="project-switcher-search">
            <input
              ref={searchRef}
              value={search}
              onChange={(event) => {
                setSearch(event.target.value);
                setHighlightedIndex(0);
              }}
              onKeyDown={handleSearchKeyDown}
              placeholder="Search projects…"
              aria-label="Search projects"
            />
            <span>{filteredProjects.length}</span>
          </div>

          <div className="project-switcher-list" role="list">
            {isFetching && projects.length === 0 ? (
              <p className="project-switcher-empty">Loading projects…</p>
            ) : filteredProjects.length === 0 ? (
              <p className="project-switcher-empty">No matching projects.</p>
            ) : (
              filteredProjects.map((project, index) => {
                const active = project.name === projectName;
                const highlighted = index === highlightedIndex;
                return (
                  <button
                    type="button"
                    key={project.name}
                    className={`project-switcher-item${active ? " active" : ""}${highlighted ? " highlighted" : ""}`}
                    disabled={switchProject.isPending}
                    onMouseEnter={() => setHighlightedIndex(index)}
                    onClick={() => selectProject(project)}
                    title={project.path}
                    aria-current={active ? "true" : undefined}
                  >
                    <span className="project-switcher-check" aria-hidden="true">
                      {active ? <CheckIcon /> : null}
                    </span>
                    <span className="project-switcher-name">{project.name}</span>
                    {project.project_type ? (
                      <span className="project-switcher-type">{projectTypeLabel(project.project_type)}</span>
                    ) : null}
                  </button>
                );
              })
            )}
          </div>

          <div className="project-switcher-footer">
            <button
              type="button"
              className="project-switcher-action"
              disabled={openFromFolder.isPending}
              onClick={() => openFromFolder.mutate()}
            >
              <span className="project-switcher-action-icon" aria-hidden="true"><FolderIcon /></span>
              <span>{openFromFolder.isPending ? "Opening…" : "Open folder…"}</span>
            </button>
            <button
              type="button"
              className="project-switcher-action"
              onClick={() => {
                setOpen(false);
                onConnectProject();
              }}
            >
              <span className="project-switcher-action-icon" aria-hidden="true"><PlusIcon /></span>
              <span>Connect project…</span>
            </button>
          </div>

          <div className="project-switcher-hint" aria-hidden="true">
            <span>↑↓ navigate</span><span>↵ open</span><span>esc close</span>
          </div>
        </div>
      ) : null}
    </div>
  );
}
