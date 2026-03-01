import type { TrainerDisplayInfo } from "@/hooks/useTrainers";

interface TrainerCardProps {
  trainer: TrainerDisplayInfo;
  focused: boolean;
  onSelect: () => void;
}

export function TrainerCard({ trainer, focused, onSelect }: TrainerCardProps) {
  return (
    <button
      className={`trainer-card ${focused ? "focused" : ""}`}
      onClick={onSelect}
      tabIndex={focused ? 0 : -1}
    >
      <div className="trainer-card-info">
        <span className="trainer-card-name">{trainer.name}</span>
        <span className="trainer-card-meta">
          {trainer.version} &middot; {trainer.source}
        </span>
      </div>
      <span className="trainer-card-action">Download</span>
    </button>
  );
}
