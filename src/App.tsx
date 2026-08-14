import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

import { ProjectSidebar } from "./components/ProjectSidebar";
import { SessionList } from "./components/SessionList";
import type { ProjectView, ScanSummary, SessionView } from "./types";
import "./App.css";

export default function App() {
  const [projects, setProjects] = useState<ProjectView[]>([]);
  const [sessions, setSessions] = useState<SessionView[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [summary, setSummary] = useState<ScanSummary | null>(null);
  const [tmuxOk, setTmuxOk] = useState(true);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setBusy(true);
    try {
      const s = await invoke<ScanSummary>("refresh");
      setSummary(s);
      setProjects(await invoke<ProjectView[]>("list_projects"));
      setSessions(await invoke<SessionView[]>("list_sessions"));
    } catch (e) {
      setNotice(String(e));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    invoke<boolean>("tmux_available").then(setTmuxOk).catch(() => setTmuxOk(false));
    reload();
  }, [reload]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return sessions.filter((s) => {
      if (selected && s.dir_name !== selected) return false;
      if (!q) return true;
      return (
        s.preview?.toLowerCase().includes(q) ||
        s.session_id.toLowerCase().includes(q) ||
        (s.project_path ?? s.dir_name).toLowerCase().includes(q)
      );
    });
  }, [sessions, selected, query]);

  const copyResume = async (s: SessionView) => {
    const cmd = await invoke<string>("resume_command", { sessionId: s.session_id });
    await writeText(cmd);
    setNotice(`Copied: ${cmd}`);
  };

  const startSession = async (p: ProjectView) => {
    try {
      const name = await invoke<string>("start_project_session", { dirName: p.dir_name });
      const attach = await invoke<string>("attach_command", { name });
      await writeText(attach);
      setNotice(`Session ${name} ready — attach command copied: ${attach}`);
      setProjects(await invoke<ProjectView[]>("list_projects"));
    } catch (e) {
      setNotice(String(e));
    }
  };

  return (
    <div className="app">
      <header className="topbar">
        <h1>cc-console</h1>
        <input
          className="search"
          placeholder="Filter sessions…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <button className="mini" onClick={reload} disabled={busy}>
          {busy ? "Scanning…" : "Rescan"}
        </button>
        {summary && (
          <span className="summary">
            {summary.sessions} sessions · {summary.projects} projects
            {summary.unresolved > 0 && (
              <span className="badge badge-missing">{summary.unresolved} unresolved</span>
            )}
          </span>
        )}
      </header>

      {!tmuxOk && (
        <div className="warn">
          tmux was not found on PATH. It is a hard dependency — session
          persistence is unavailable until it is installed.
        </div>
      )}

      {notice && (
        <div className="notice" onClick={() => setNotice(null)}>
          {notice}
        </div>
      )}

      <div className="body">
        <ProjectSidebar
          projects={projects}
          selected={selected}
          onSelect={setSelected}
          onStart={startSession}
        />
        <main className="main">
          <SessionList sessions={visible} onCopyResume={copyResume} />
        </main>
      </div>
    </div>
  );
}
