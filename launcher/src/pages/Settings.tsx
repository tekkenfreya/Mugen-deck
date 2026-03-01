import { useController } from "@/hooks/useController";
import { useDaemon } from "@/hooks/useDaemon";

interface SettingsProps {
  onBack: () => void;
}

export function Settings({ onBack }: SettingsProps) {
  const { connected, health } = useDaemon();

  useController({ onBack });

  return (
    <div className="page">
      <div className="page-content">
        <button className="back-btn" onClick={onBack}>
          Back
        </button>
        <h2 className="page-title">Settings</h2>
        <div className="settings-section">
          <h3>Daemon</h3>
          <div className="settings-row">
            <span>Status</span>
            <span className={connected ? "text-success" : "text-danger"}>
              {connected ? "Connected" : "Disconnected"}
            </span>
          </div>
          {health && (
            <>
              <div className="settings-row">
                <span>Version</span>
                <span>{health.version}</span>
              </div>
              <div className="settings-row">
                <span>Uptime</span>
                <span>{health.uptime_secs}s</span>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
