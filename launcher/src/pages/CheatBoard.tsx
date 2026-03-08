import { useCallback, useEffect, useRef, useState } from "react";

async function sendHotkey(key: string, ctrl: boolean, shift: boolean, alt: boolean) {
  await fetch("http://127.0.0.1:7331/sharkdeck/hotkey", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ key, ctrl, shift, alt }),
  });
}

interface KeyDef {
  label: string;
  xdotool: string;
  width?: number;
  height?: number;
}

const F_KEYS: KeyDef[] = Array.from({ length: 12 }, (_, i) => ({
  label: `F${i + 1}`,
  xdotool: `F${i + 1}`,
}));

const NAV_KEYS: KeyDef[] = [
  { label: "PrtSc", xdotool: "Print" },
  { label: "Ins", xdotool: "Insert" },
  { label: "Home", xdotool: "Home" },
  { label: "PgUp", xdotool: "Prior" },
  { label: "Del", xdotool: "Delete" },
  { label: "End", xdotool: "End" },
  { label: "PgDn", xdotool: "Next" },
];

// Numpad laid out as a grid. We use a CSS grid approach.
// Standard numpad layout (4 columns, 5 rows):
//   NumLk  /    *    -
//   7      8    9    +  (+ spans 2 rows)
//   4      5    6
//   1      2    3    Enter (Enter spans 2 rows)
//   0 (spans 2 cols) .

interface NumpadKey extends KeyDef {
  gridColumn?: string;
  gridRow?: string;
}

const NUMPAD_KEYS: NumpadKey[] = [
  { label: "Num", xdotool: "Num_Lock", gridColumn: "1", gridRow: "1" },
  { label: "/", xdotool: "KP_Divide", gridColumn: "2", gridRow: "1" },
  { label: "*", xdotool: "KP_Multiply", gridColumn: "3", gridRow: "1" },
  { label: "-", xdotool: "KP_Subtract", gridColumn: "4", gridRow: "1" },
  { label: "7", xdotool: "KP_7", gridColumn: "1", gridRow: "2" },
  { label: "8", xdotool: "KP_8", gridColumn: "2", gridRow: "2" },
  { label: "9", xdotool: "KP_9", gridColumn: "3", gridRow: "2" },
  { label: "+", xdotool: "KP_Add", gridColumn: "4", gridRow: "2 / 4" },
  { label: "4", xdotool: "KP_4", gridColumn: "1", gridRow: "3" },
  { label: "5", xdotool: "KP_5", gridColumn: "2", gridRow: "3" },
  { label: "6", xdotool: "KP_6", gridColumn: "3", gridRow: "3" },
  { label: "1", xdotool: "KP_1", gridColumn: "1", gridRow: "4" },
  { label: "2", xdotool: "KP_2", gridColumn: "2", gridRow: "4" },
  { label: "3", xdotool: "KP_3", gridColumn: "3", gridRow: "4" },
  { label: "Ent", xdotool: "KP_Enter", gridColumn: "4", gridRow: "4 / 6" },
  { label: "0", xdotool: "KP_0", gridColumn: "1 / 3", gridRow: "5" },
  { label: ".", xdotool: "KP_Decimal", gridColumn: "3", gridRow: "5" },
];

type Modifier = "ctrl" | "shift" | "alt";

const STYLE = `
/* ═══════════════════════════════════════════════════════════════
   CHEATBOARD — Trainer Hotkey Overlay
   ═══════════════════════════════════════════════════════════════ */

.cb-root {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: linear-gradient(180deg, #1a1e2a 0%, #0e1018 100%);
  overflow: hidden;
  user-select: none;
}

/* ─── Title Bar ───────────────────────────────────────────── */

.cb-titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  background: linear-gradient(180deg, #222838 0%, #1a1e2a 100%);
  border-bottom: 2px solid #2a3040;
  min-height: 44px;
  flex-shrink: 0;
}

.cb-title {
  font-family: var(--font-display);
  font-size: 14px;
  font-weight: 700;
  color: #70b0e0;
  letter-spacing: 3px;
  text-transform: uppercase;
}

.cb-minimize-btn {
  padding: 6px 14px;
  background: linear-gradient(180deg, var(--silver-light) 0%, var(--silver) 100%);
  border: 2px solid;
  border-color: var(--bevel-hi) var(--bevel-shadow) var(--bevel-shadow) var(--bevel-hi);
  color: var(--text);
  font-family: var(--font-display);
  font-size: 9px;
  font-weight: 700;
  cursor: pointer;
  letter-spacing: 1px;
  text-transform: uppercase;
  min-height: 32px;
  min-width: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.cb-minimize-btn:active {
  border-color: var(--bevel-shadow) var(--bevel-hi) var(--bevel-hi) var(--bevel-shadow);
}

/* ─── Minimized Bar ───────────────────────────────────────── */

.cb-minimized {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  background: linear-gradient(180deg, #222838 0%, #14181f 100%);
  border-top: 2px solid #2a3040;
  min-height: 48px;
  z-index: 100;
}

.cb-minimized-label {
  font-family: var(--font-display);
  font-size: 12px;
  font-weight: 700;
  color: #70b0e0;
  letter-spacing: 3px;
}

/* ─── Keyboard Area ───────────────────────────────────────── */

.cb-keyboard {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
  overflow: hidden;
}

/* ─── Row: F-Keys ─────────────────────────────────────────── */

.cb-row-fkeys {
  display: grid;
  grid-template-columns: repeat(12, 1fr);
  gap: 4px;
}

/* ─── Row: Nav Keys ───────────────────────────────────────── */

.cb-row-nav {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 4px;
}

/* ─── Row: Numpad ─────────────────────────────────────────── */

.cb-row-numpad {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  grid-template-rows: repeat(5, 1fr);
  gap: 4px;
  flex: 1;
  min-height: 0;
}

/* ─── Row: Modifiers ──────────────────────────────────────── */

.cb-row-modifiers {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 4px;
  flex-shrink: 0;
}

/* ─── Key Button Base ─────────────────────────────────────── */

.cb-key {
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(180deg, var(--silver-light) 0%, var(--silver) 100%);
  border: 3px solid;
  border-color: var(--bevel-hi) var(--bevel-shadow) var(--bevel-shadow) var(--bevel-hi);
  color: var(--text);
  font-family: var(--font-display);
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
  min-height: 48px;
  min-width: 48px;
  letter-spacing: 1px;
  text-transform: uppercase;
  transition: background 0.05s, border-color 0.05s;
  padding: 4px;
}

.cb-key:hover {
  background: linear-gradient(180deg, var(--white) 0%, var(--silver-light) 100%);
}

.cb-key:active,
.cb-key--pressed {
  border-color: var(--bevel-shadow) var(--bevel-hi) var(--bevel-hi) var(--bevel-shadow) !important;
  background: linear-gradient(180deg, var(--silver) 0%, var(--silver-mid) 100%) !important;
}

/* ─── Key Flash Animation ─────────────────────────────────── */

.cb-key--flash {
  background: linear-gradient(180deg, var(--ocean-light) 0%, var(--ocean) 100%) !important;
  border-color: var(--ocean-light) var(--ocean-deep) var(--ocean-deep) var(--ocean-light) !important;
  color: #fff !important;
}

/* ─── Modifier Active State ───────────────────────────────── */

.cb-key--modifier-active {
  background: linear-gradient(180deg, var(--ocean-light) 0%, var(--ocean) 100%);
  border-color: var(--ocean-light) var(--ocean-deep) var(--ocean-deep) var(--ocean-light);
  color: #fff;
  text-shadow: 1px 1px 2px rgba(0, 0, 0, 0.3);
  box-shadow: 0 0 12px rgba(26, 95, 168, 0.4);
}

/* ─── Modifier Button (larger) ────────────────────────────── */

.cb-key--modifier {
  min-height: 56px;
  font-size: 14px;
  letter-spacing: 2px;
}

/* ─── Section Labels ──────────────────────────────────────── */

.cb-section-label {
  font-family: var(--font-display);
  font-size: 8px;
  font-weight: 700;
  color: #5a6a80;
  letter-spacing: 2px;
  text-transform: uppercase;
  padding: 0 2px;
  flex-shrink: 0;
}

/* ─── Layout wrapper for numpad + modifiers side by side ──── */

.cb-bottom-area {
  display: flex;
  gap: 8px;
  flex: 1;
  min-height: 0;
}

.cb-numpad-col {
  flex: 3;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-height: 0;
}

.cb-modifier-col {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-height: 0;
}

.cb-modifier-col .cb-row-modifiers {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.cb-modifier-col .cb-key--modifier {
  flex: 1;
}
`;

export function CheatBoard() {
  const [minimized, setMinimized] = useState(false);
  const [modifiers, setModifiers] = useState<Record<Modifier, boolean>>({
    ctrl: false,
    shift: false,
    alt: false,
  });
  const [flashKey, setFlashKey] = useState<string | null>(null);
  const flashTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Inject styles on mount
  useEffect(() => {
    const id = "cb-styles";
    if (document.getElementById(id)) return;
    const style = document.createElement("style");
    style.id = id;
    style.textContent = STYLE;
    document.head.appendChild(style);
    return () => {
      const el = document.getElementById(id);
      if (el) el.remove();
    };
  }, []);

  const toggleModifier = useCallback((mod: Modifier) => {
    setModifiers((prev) => ({ ...prev, [mod]: !prev[mod] }));
  }, []);

  const handleKeyPress = useCallback(
    (xdotool: string) => {
      // Flash the key
      setFlashKey(xdotool);
      if (flashTimeout.current) clearTimeout(flashTimeout.current);
      flashTimeout.current = setTimeout(() => setFlashKey(null), 150);

      // Send hotkey
      sendHotkey(xdotool, modifiers.ctrl, modifiers.shift, modifiers.alt).catch(
        () => {
          // silently ignore network errors
        },
      );

      // Auto-release modifiers after sending
      setModifiers({ ctrl: false, shift: false, alt: false });
    },
    [modifiers],
  );

  const renderKey = useCallback(
    (keyDef: KeyDef, style?: React.CSSProperties) => {
      const isFlashing = flashKey === keyDef.xdotool;
      const classes = ["cb-key"];
      if (isFlashing) classes.push("cb-key--flash");

      return (
        <button
          key={keyDef.xdotool}
          className={classes.join(" ")}
          style={style}
          onClick={() => handleKeyPress(keyDef.xdotool)}
        >
          {keyDef.label}
        </button>
      );
    },
    [flashKey, handleKeyPress],
  );

  // ─── Minimized State ──────────────────────────────────────────
  if (minimized) {
    return (
      <div className="cb-root" style={{ background: "transparent", height: "auto" }}>
        <div className="cb-minimized">
          <span className="cb-minimized-label">CHEATBOARD</span>
          <button
            className="cb-minimize-btn"
            onClick={() => setMinimized(false)}
          >
            EXPAND
          </button>
        </div>
      </div>
    );
  }

  // ─── Full Layout ──────────────────────────────────────────────
  return (
    <div className="cb-root">
      <div className="cb-titlebar">
        <span className="cb-title">CheatBoard</span>
        <button
          className="cb-minimize-btn"
          onClick={() => setMinimized(true)}
        >
          MINIMIZE
        </button>
      </div>

      <div className="cb-keyboard">
        {/* Row 1: F-Keys */}
        <span className="cb-section-label">FUNCTION KEYS</span>
        <div className="cb-row-fkeys">
          {F_KEYS.map((k) => renderKey(k))}
        </div>

        {/* Row 2: Nav Keys */}
        <span className="cb-section-label">NAVIGATION</span>
        <div className="cb-row-nav">
          {NAV_KEYS.map((k) => renderKey(k))}
        </div>

        {/* Row 3: Numpad + Row 4: Modifiers side by side */}
        <div className="cb-bottom-area">
          <div className="cb-numpad-col">
            <span className="cb-section-label">NUMPAD</span>
            <div className="cb-row-numpad">
              {NUMPAD_KEYS.map((k) => {
                const style: React.CSSProperties = {};
                if (k.gridColumn) style.gridColumn = k.gridColumn;
                if (k.gridRow) style.gridRow = k.gridRow;
                return renderKey(k, style);
              })}
            </div>
          </div>

          <div className="cb-modifier-col">
            <span className="cb-section-label">MODIFIERS</span>
            <div className="cb-row-modifiers">
              {(["ctrl", "shift", "alt"] as const).map((mod) => {
                const isActive = modifiers[mod];
                const classes = ["cb-key", "cb-key--modifier"];
                if (isActive) classes.push("cb-key--modifier-active");

                return (
                  <button
                    key={mod}
                    className={classes.join(" ")}
                    onClick={() => toggleModifier(mod)}
                  >
                    {mod.toUpperCase()}
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
