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
          &lt; BACK
        </button>
        <h2 className="page-title">SYSTEM CONFIG</h2>
        <div className="settings-section">
          <h3>DAEMON</h3>
          <div className="settings-row">
            <span>STATUS</span>
            <span className={connected ? "text-success" : "text-danger"}>
              {connected ? "ONLINE" : "OFFLINE"}
            </span>
          </div>
          {health && (
            <>
              <div className="settings-row">
                <span>VERSION</span>
                <span>{health.version}</span>
              </div>
              <div className="settings-row">
                <span>UPTIME</span>
                <span>{health.uptime_secs}S</span>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
