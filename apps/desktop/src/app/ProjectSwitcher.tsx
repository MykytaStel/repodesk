import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { projectUse, projectAdd } from "../shared/api/workflow";
import { pickDirectory, basename } from "../shared/api/dialog";
import { useToast } from "../shared/ui/Toast";
import { CheckIcon, ChevronIcon, FolderIcon, PlusIcon } from "./NavIcons";

type ProjectConfig = { name: string; path?: string; project_type?: string };

const SEARCH_THRESHOLD = 6;

/**
 * Project switching is a context change, not a project-management form. Keep
 * the common small-list path extremely compact; search appears only once the
 * connected-project set is large enough to need it.
 */
export function ProjectSwitcher({ projectName, onConnectProject }: { projectName: string; onConnectProject: () => void }) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [highlightedIndex, setHighlightedIndex] = useState(0);
  const ref = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const { data: projects = [], isFetching, isError, error, refetch } = useQuery({
    queryKey: ["project_list_configs"],
    queryFn: () => invoke<ProjectConfig[]>("project_list_configs"),
    enabled: open,
    staleTime: 60_000,
  });

  const showSearch = projects.length >= SEARCH_THRESHOLD;
  const filteredProjects = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!showSearch || !query) return projects;
    return projects.filter((project) =>
      [project.name, project.path ?? "", project.project_type ?? ""]
        .some((value) => value.toLowerCase().includes(query)),
    );
  }, [projects, search, showSearch]);

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
    const activeIndex = projects.findIndex((project) => project.name === projectName);
    setHighlightedIndex(activeIndex >= 0 ? activeIndex : 0);
    requestAnimationFrame(() => menuRef.current?.focus());
  // Project data may arrive after the popover opens; do not refocus on every
  // query result update and steal the user's keyboard position.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    if (!open || !showSearch) return;
    if (document.activeElement === menuRef.current) searchRef.current?.focus();
  }, [open, showSearch]);

  useEffect(() => {
    setHighlightedIndex((current) => Math.min(current, Math.max(0, filteredProjects.length - 1)));
  }, [filteredProjects.length]);

  useEffect(() => {
    if (!open) return;
    requestAnimationFrame(() => {
      ref.current
        ?.querySelector<HTMLElement>(".project-switcher-item.highlighted")
        ?.scrollIntoView({ block: "nearest" });
    });
  }, [highlightedIndex, open]);

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

  const handleNavigationKeyDown = (event: ReactKeyboardEvent<HTMLElement>) => {
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
    <div className="project-switcher project-switcher-v2" ref={ref}>
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
        <div
          ref={menuRef}
          className="project-switcher-menu"
          role="dialog"
          aria-label="Switch project"
          tabIndex={-1}
          onKeyDown={handleNavigationKeyDown}
        >
          {showSearch ? (
            <div className="project-switcher-search compact">
              <input
                ref={searchRef}
                value={search}
                onChange={(event) => {
                  setSearch(event.target.value);
                  setHighlightedIndex(0);
                }}
                onKeyDown={handleNavigationKeyDown}
                placeholder="Find project…"
                aria-label="Find project"
              />
            </div>
          ) : null}

          <div className="project-switcher-list" role="list">
            {isError ? (
              <div className="project-switcher-error" role="alert">
                <strong>Could not load projects</strong>
                <span>{error instanceof Error ? error.message : String(error)}</span>
                <button type="button" className="tiny-button" onClick={() => void refetch()}>
                  Retry loading projects
                </button>
              </div>
            ) : isFetching && projects.length === 0 ? (
              <p className="project-switcher-empty">Loading…</p>
            ) : filteredProjects.length === 0 ? (
              <p className="project-switcher-empty">No matching projects.</p>
            ) : (
              filteredProjects.map((project, index) => {
                const active = project.name === projectName;
                const highlighted = index === highlightedIndex;
                const meta = [project.project_type, project.path].filter(Boolean).join(" · ");
                return (
                  <button
                    type="button"
                    key={project.name}
                    className={`project-switcher-item${active ? " active" : ""}${highlighted ? " highlighted" : ""}`}
                    disabled={switchProject.isPending}
                    onMouseEnter={() => setHighlightedIndex(index)}
                    onClick={() => selectProject(project)}
                    title={meta || project.name}
                    aria-current={active ? "true" : undefined}
                  >
                    <span className="project-switcher-check" aria-hidden="true">
                      {active ? <CheckIcon /> : null}
                    </span>
                    <span className="project-switcher-name">{project.name}</span>
                  </button>
                );
              })
            )}
          </div>

          <div className="project-switcher-footer compact">
            <button
              type="button"
              className="project-switcher-action"
              disabled={openFromFolder.isPending}
              onClick={() => openFromFolder.mutate()}
              title="Open a project folder"
            >
              <span className="project-switcher-action-icon" aria-hidden="true"><FolderIcon /></span>
              <span>{openFromFolder.isPending ? "Opening…" : "Open folder"}</span>
            </button>
            <button
              type="button"
              className="project-switcher-action"
              onClick={() => {
                setOpen(false);
                onConnectProject();
              }}
              title="Open the project registry"
            >
              <span className="project-switcher-action-icon" aria-hidden="true"><PlusIcon /></span>
              <span>Projects…</span>
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
