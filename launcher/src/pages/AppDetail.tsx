import { useController } from "@/hooks/useController";
import { useApps } from "@/hooks/useApps";
import type { AppInfo } from "@/types";

interface AppDetailProps {
  app: AppInfo;
  onBack: () => void;
}

export function AppDetail({ app, onBack }: AppDetailProps) {
  const { launch, close } = useApps();

  useController({
    onBack,
    onConfirm: () => {
      if (app.status === "running") {
        close(app.id);
      } else {
        launch(app.id);
      }
    },
  });

  const isRunning = app.status === "running";

  return (
    <div className="page">
      <div className="page-content app-detail">
        <button className="back-btn" onClick={onBack}>
          Back
        </button>
        <div className="app-detail-header">
          <div className="app-detail-icon">
            {app.name.charAt(0).toUpperCase()}
          </div>
          <div>
            <h2>{app.name}</h2>
            <p className="dim">v{app.version}</p>
          </div>
        </div>
        <p className="app-detail-desc">{app.description}</p>
        <button
          className={`action-btn ${isRunning ? "danger" : "primary"}`}
          onClick={() => (isRunning ? close(app.id) : launch(app.id))}
        >
          {isRunning ? "Stop" : "Launch"}
        </button>
      </div>
    </div>
  );
}
