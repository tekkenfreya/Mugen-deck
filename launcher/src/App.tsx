import { useState } from "react";
import { Home } from "@/pages/Home";
import { AppDetail } from "@/pages/AppDetail";
import { Settings } from "@/pages/Settings";
import { SharkDeck } from "@/pages/SharkDeck";
import type { AppInfo } from "@/types";

type Page =
  | { kind: "home" }
  | { kind: "app-detail"; app: AppInfo }
  | { kind: "settings" }
  | { kind: "sharkdeck" };

export function App() {
  const [page, setPage] = useState<Page>({ kind: "home" });

  switch (page.kind) {
    case "home":
      return (
        <Home
          onSelectApp={(app) => {
            // Route to SharkDeck page if that app is selected
            if (app.id === "sharkdeck") {
              setPage({ kind: "sharkdeck" });
            } else {
              setPage({ kind: "app-detail", app });
            }
          }}
        />
      );
    case "app-detail":
      return (
        <AppDetail
          app={page.app}
          onBack={() => setPage({ kind: "home" })}
        />
      );
    case "settings":
      return <Settings onBack={() => setPage({ kind: "home" })} />;
    case "sharkdeck":
      return <SharkDeck onBack={() => setPage({ kind: "home" })} />;
  }
}
