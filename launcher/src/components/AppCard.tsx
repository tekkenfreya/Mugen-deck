import type { AppInfo } from "@/types";

interface AppCardProps {
  app: AppInfo;
  focused: boolean;
  onSelect: () => void;
  onHover?: () => void;
}

export function AppCard({ app, focused, onSelect, onHover }: AppCardProps) {
  return (
    <button
      className={`app-card ${focused ? "focused" : ""}`}
      onClick={onSelect}
      onMouseEnter={onHover}
      tabIndex={focused ? 0 : -1}
    >
      <div className="app-card-icon">
        {app.name.charAt(0).toUpperCase()}
      </div>
      <div className="app-card-info">
        <span className="app-card-name">{app.name}</span>
        <span className="app-card-version">v{app.version}</span>
      </div>
      <span className={`app-card-status status-${app.status}`}>
        {app.status}
      </span>
    </button>
  );
}
