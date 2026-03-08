import { useCallback, useEffect, useState } from "react";
import { AppGrid } from "@/components/AppGrid";
import { StatusBar } from "@/components/StatusBar";
import { MugenLogo } from "@/components/MugenLogo";
import { useApps } from "@/hooks/useApps";
import { getCurrentGame } from "@/api/daemon";
import type { AppInfo, RunningGame } from "@/types";

interface HomeProps {
  onSelectApp: (app: AppInfo) => void;
}

export function Home({ onSelectApp }: HomeProps) {
  const { apps, loading } = useApps();
  const [currentGame, setCurrentGame] = useState<RunningGame | null>(null);

  const pollCurrentGame = useCallback(async () => {
    try {
      const resp = await getCurrentGame();
      if (resp.ok) {
        setCurrentGame(resp.data ?? null);
      }
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    pollCurrentGame();
    const id = setInterval(pollCurrentGame, 5000);
    return () => clearInterval(id);
  }, [pollCurrentGame]);

  return (
    <div className="page">
      <StatusBar currentGame={currentGame} />
      <div className="page-content">
        <div className="mugen-banner">
          <MugenLogo size={100} />
          <div className="mugen-banner-title">MUGEN</div>
          <div className="mugen-banner-sub">GAME ENHANCEMENT SYSTEM</div>
        </div>
        <h2 className="page-title">SELECT APPLICATION</h2>
        {loading ? (
          <p className="dim">LOADING...</p>
        ) : (
          <AppGrid apps={apps} onSelect={onSelectApp} />
        )}
      </div>
    </div>
  );
}
