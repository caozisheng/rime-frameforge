import type { RuntimeLogEntry } from '../../../../web/src/contracts.js';

interface LogConsoleProps {
  readonly entries: readonly RuntimeLogEntry[];
  readonly collapsed: boolean;
  readonly onToggle: () => void;
}

export function LogConsole({ entries, collapsed, onToggle }: LogConsoleProps) {
  return (
    <section className={`panel log-panel ${collapsed ? 'log-collapsed' : ''}`} aria-labelledby="logs-heading">
      <div className="log-heading">
        <button className="log-toggle" onClick={onToggle} type="button" aria-expanded={!collapsed}>
          <span className="log-chevron" aria-hidden="true">{collapsed ? '▸' : '▾'}</span>
          <span>
            <span className="eyebrow">Runtime trace</span>
            <strong id="logs-heading">Logs</strong>
          </span>
        </button>
        <span className="log-count">{entries.length.toString().padStart(2, '0')} events</span>
      </div>
      {!collapsed && (
        <div className="log-list" role="log" aria-live="polite">
          {entries.length === 0 && <div className="empty-log">Awaiting Worker events…</div>}
          {entries.map((entry, index) => (
            <div className={`log-entry log-${entry.level}`} key={`${entry.message}-${index}`}>
              <span className="log-level">{entry.level}</span>
              <span>{entry.message}</span>
              {entry.framePhase && <span className="log-phase">{entry.framePhase}</span>}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
