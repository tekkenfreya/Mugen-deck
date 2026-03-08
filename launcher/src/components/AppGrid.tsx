import { useEffect, useState } from "react";
import { useController } from "@/hooks/useController";
import { AppCard } from "@/components/AppCard";
import type { AppInfo } from "@/types";

interface AppGridProps {
  apps: AppInfo[];
  columns?: number;
  onSelect: (app: AppInfo) => void;
}

export function AppGrid({ apps, onSelect }: AppGridProps) {
  const [focusIndex, setFocusIndex] = useState(0);

  // Move native DOM focus to the highlighted app card
  useEffect(() => {
    if (apps.length > 0) {
      const cards = document.querySelectorAll('.app-card');
      const el = cards[focusIndex] as HTMLElement | undefined;
      el?.focus({ preventScroll: false });
    }
  }, [focusIndex, apps.length]);

  useController({
    onNavigate: (dir) => {
      setFocusIndex((prev) => {
        const total = apps.length;
        if (total === 0) return 0;

        switch (dir) {
          case "up":
            return Math.max(0, prev - 1);
          case "down":
            return Math.min(total - 1, prev + 1);
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
        <p>NO APPS INSTALLED</p>
        <p className="dim">APPS WILL APPEAR HERE ONCE REGISTERED WITH THE DAEMON</p>
      </div>
    );
  }

  return (
    <div className="app-grid">
      {apps.map((app, i) => (
        <AppCard
          key={app.id}
          app={app}
          focused={i === focusIndex}
          onSelect={() => onSelect(app)}
          onHover={() => setFocusIndex(i)}
        />
      ))}
    </div>
  );
}
