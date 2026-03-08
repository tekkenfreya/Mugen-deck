import { useEffect, useState } from "react";
import { SharkDeck } from "@/pages/SharkDeck";
import { getAuthToken, setSessionToken } from "@/api/daemon";

export function App() {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    getAuthToken()
      .then((resp) => {
        if (resp.ok && resp.data) {
          setSessionToken(resp.data.token);
        }
      })
      .catch(() => {
        // Daemon unreachable — continue anyway
      })
      .finally(() => {
        setReady(true);
      });
  }, []);

  if (!ready) return null;

  return <SharkDeck />;
}
