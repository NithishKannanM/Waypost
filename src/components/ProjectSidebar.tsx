import type { ProjectView } from "../types";

interface Props {
  projects: ProjectView[];
  selected: string | null;
  onSelect: (dirName: string | null) => void;
  onStart: (p: ProjectView) => void;
}

const RESOLUTION_HINT: Record<string, string> = {
  resolved: "Directory verified on disk",
  ambiguous: "Directory name decodes several ways — cannot pick one safely",
  missing: "Directory not found (moved, deleted, or volume not mounted)",
};

export function ProjectSidebar({ projects, selected, onSelect, onStart }: Props) {
  return (
    <aside className="sidebar">
      <div className="sidebar-head">
        <h2>Projects</h2>
        <button
          className={selected === null ? "chip chip-on" : "chip"}
          onClick={() => onSelect(null)}
        >
          All
        </button>
      </div>

      <ul className="project-list">
        {projects.map((p) => {
          const unresolved = p.resolution !== "resolved";
          return (
            <li
              key={p.dir_name}
              className={p.dir_name === selected ? "project project-sel" : "project"}
              onClick={() => onSelect(p.dir_name)}
            >
              <div className="project-top">
                <span className="project-name" title={p.project_path ?? p.dir_name}>
                  {p.project_path ? basename(p.project_path) : p.dir_name}
                </span>
                <span className="count">{p.session_count}</span>
              </div>

              <div className="project-path" title={RESOLUTION_HINT[p.resolution]}>
                {p.project_path ?? (
                  <span className={`badge badge-${p.resolution}`}>{p.resolution}</span>
                )}
              </div>

              <div className="project-actions">
                <span className={p.tmux_running ? "dot dot-on" : "dot"} />
                <span className="tmux-name" title={p.tmux_session}>
                  {p.tmux_running ? "tmux running" : "no tmux session"}
                </span>
                <button
                  className="mini"
                  disabled={unresolved}
                  title={
                    unresolved
                      ? "Needs a verified project directory — cannot start a session without one"
                      : `tmux new-session -s ${p.tmux_session}`
                  }
                  onClick={(e) => {
                    e.stopPropagation();
                    onStart(p);
                  }}
                >
                  Start
                </button>
              </div>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}

function basename(p: string) {
  const parts = p.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? p;
}
