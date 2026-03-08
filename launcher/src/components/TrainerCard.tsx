import type { SharkDeckStatus, TrainerInfo } from "@/types";

interface TrainerCardProps {
  trainer: TrainerInfo;
  focused: boolean;
  status: SharkDeckStatus;
  onSelect: () => void;
  onHover?: () => void;
}

function actionText(status: SharkDeckStatus): string {
  switch (status) {
    case "downloading":
      return "DOWNLOADING...";
    case "installing_deps":
      return "INSTALLING...";
    default:
      return "ENABLE";
  }
}

export function TrainerCard({ trainer, focused, status, onSelect, onHover }: TrainerCardProps) {
  const busy = status === "downloading" || status === "installing_deps";

  return (
    <button
      className={`trainer-card ${focused ? "focused" : ""}`}
      onClick={onSelect}
      onMouseEnter={onHover}
      disabled={busy}
      tabIndex={focused ? 0 : -1}
    >
      <div className="trainer-card-info">
        <span className="trainer-card-name">{trainer.name}</span>
        <span className="trainer-card-meta">
          <span className={`trainer-source source-${trainer.source}`}>
            {trainer.source === "gcw" ? "GCW" : "FLING"}
          </span>
          {trainer.version}
        </span>
      </div>
      <span className="trainer-card-action">{actionText(status)}</span>
    </button>
  );
}
