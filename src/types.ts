export type ResolutionKind = "resolved" | "ambiguous" | "missing";

export interface SessionView {
  session_id: string;
  dir_name: string;
  /** null when the project directory could not be verified on disk. */
  project_path: string | null;
  resolution: ResolutionKind;
  transcript_path: string;
  last_active_at: string;
  preview: string | null;
}

export interface ProjectView {
  dir_name: string;
  project_path: string | null;
  resolution: ResolutionKind;
  session_count: number;
  last_active_at: string;
  tmux_session: string;
  tmux_running: boolean;
}

export interface ScanSummary {
  projects: number;
  sessions: number;
  unresolved: number;
}
