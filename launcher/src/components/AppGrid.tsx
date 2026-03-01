import { useState } from "react";
import { useController } from "@/hooks/useController";
import { AppCard } from "@/components/AppCard";
import type { AppInfo } from "@/types";

interface AppGridProps {
  apps: AppInfo[];
  columns?: number;
  onSelect: (app: AppInfo) => void;
}

export function AppGrid({ apps, columns = 3, onSelect }: AppGridProps) {
  const [focusIndex, setFocusIndex] = useState(0);

  useController({
    onNavigate: (dir) => {
      setFocusIndex((prev) => {
        const total = apps.length;
        if (total === 0) return 0;

        switch (dir) {
          case "up":
            return Math.max(0, prev - columns);
          case "down":
            return Math.min(total - 1, prev + columns);
          case "left":
            return Math.max(0, prev - 1);
          case "right":
            return Math.min(total - 1, prev + 1);
          default:
            return prev;
        }
      });
    },
    onConfirm: () => {
      const app = apps[focusIndex];
      if (app) onSelect(app);
    },
  });

  if (apps.length === 0) {
    return (
      <div className="app-grid-empty">
        <p>No apps installed</p>
        <p className="dim">Apps will appear here once registered with the daemon</p>
      </div>
    );
  }

  return (
    <div
      className="app-grid"
      style={{ gridTemplateColumns: `repeat(${columns}, 1fr)` }}
    >
      {apps.map((app, i) => (
        <AppCard
          key={app.id}
          app={app}
          focused={i === focusIndex}
          onSelect={() => onSelect(app)}
        />
      ))}
    </div>
  );
}
