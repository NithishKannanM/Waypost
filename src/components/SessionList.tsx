import type { SessionView } from "../types";

interface Props {
  sessions: SessionView[];
  onCopyResume: (s: SessionView) => void;
}

export function SessionList({ sessions, onCopyResume }: Props) {
  if (sessions.length === 0) {
    return <p className="empty">No sessions found.</p>;
  }

  return (
    <ul className="session-list">
      {sessions.map((s) => (
        <li key={s.session_id} className="session">
          <div className="session-main">
            <p className="preview">
              {s.preview ?? <span className="muted">no preview available</span>}
            </p>
            <div className="session-meta">
              <span title={s.project_path ?? s.dir_name}>
                {s.project_path ?? s.dir_name}
              </span>
              {s.resolution !== "resolved" && (
                <span className={`badge badge-${s.resolution}`}>{s.resolution}</span>
              )}
              <span className="mono">{s.session_id.slice(0, 8)}</span>
              <time dateTime={s.last_active_at}>{formatWhen(s.last_active_at)}</time>
            </div>
          </div>
          <button className="mini" onClick={() => onCopyResume(s)}>
            Copy resume
          </button>
        </li>
      ))}
    </ul>
  );
}

function formatWhen(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const mins = Math.round((Date.now() - d.getTime()) / 60000);
  if (mins < 60) return `${mins}m ago`;
  if (mins < 60 * 24) return `${Math.round(mins / 60)}h ago`;
  const days = Math.round(mins / 1440);
  if (days < 30) return `${days}d ago`;
  return d.toLocaleDateString();
}
