import { useCallback, useEffect, useRef, useState } from "react";

/** Gamepad button indices (standard mapping). */
const BUTTON_A = 0;
const BUTTON_B = 1;
const DPAD_UP = 12;
const DPAD_DOWN = 13;
const DPAD_LEFT = 14;
const DPAD_RIGHT = 15;

/** Left stick axis indices. */
const AXIS_LEFT_X = 0;
const AXIS_LEFT_Y = 1;

/** Deadzone for analog sticks — ignore small drift. */
const STICK_DEADZONE = 0.5;

/** Minimum ms between repeated stick-navigation events. */
const STICK_REPEAT_MS = 200;

interface UseControllerOptions {
  onNavigate?: (direction: "up" | "down" | "left" | "right") => void;
  onConfirm?: () => void;
  onBack?: () => void;
}

interface UseControllerResult {
  /** True once a gamepad has been detected by the Gamepad API. */
  gamepadActive: boolean;
}

/** Find the first connected gamepad across all slots. */
function findGamepad(): Gamepad | null {
  const gamepads = navigator.getGamepads();
  for (let i = 0; i < gamepads.length; i++) {
    const gp = gamepads[i];
    if (gp && gp.connected) return gp;
  }
  return null;
}

/** Polls the Gamepad API each frame and listens for keyboard arrows as fallback. */
export function useController(options: UseControllerOptions): UseControllerResult {
  const prevButtons = useRef<Map<number, boolean>>(new Map());
  const prevStickDir = useRef<{ x: string; y: string }>({ x: "", y: "" });
  const lastStickNav = useRef<number>(0);
  const optionsRef = useRef(options);
  optionsRef.current = options;

  const [gamepadActive, setGamepadActive] = useState(false);

  const poll = useCallback(() => {
    const gp = findGamepad();
    if (!gp) return;

    if (!gamepadActive) setGamepadActive(true);

    const pressed = (index: number): boolean => {
      const btn = gp.buttons[index];
      return btn !== undefined && btn.pressed;
    };

    const justPressed = (index: number): boolean => {
      const isPressed = pressed(index);
      const wasPressed = prevButtons.current.get(index) ?? false;
      return isPressed && !wasPressed;
    };

    // D-pad buttons
    if (justPressed(DPAD_UP)) optionsRef.current.onNavigate?.("up");
    if (justPressed(DPAD_DOWN)) optionsRef.current.onNavigate?.("down");
    if (justPressed(DPAD_LEFT)) optionsRef.current.onNavigate?.("left");
    if (justPressed(DPAD_RIGHT)) optionsRef.current.onNavigate?.("right");
    if (justPressed(BUTTON_A)) optionsRef.current.onConfirm?.();
    if (justPressed(BUTTON_B)) optionsRef.current.onBack?.();

    // Left analog stick navigation
    const axisX = gp.axes[AXIS_LEFT_X] ?? 0;
    const axisY = gp.axes[AXIS_LEFT_Y] ?? 0;
    const now = performance.now();

    let stickDirX = "";
    let stickDirY = "";
    if (axisX < -STICK_DEADZONE) stickDirX = "left";
    else if (axisX > STICK_DEADZONE) stickDirX = "right";
    if (axisY < -STICK_DEADZONE) stickDirY = "up";
    else if (axisY > STICK_DEADZONE) stickDirY = "down";

    const canRepeat = now - lastStickNav.current > STICK_REPEAT_MS;

    // Fire on direction change OR after repeat delay
    if (stickDirY && (stickDirY !== prevStickDir.current.y || canRepeat)) {
      optionsRef.current.onNavigate?.(stickDirY as "up" | "down");
      lastStickNav.current = now;
    }
    if (stickDirX && (stickDirX !== prevStickDir.current.x || canRepeat)) {
      optionsRef.current.onNavigate?.(stickDirX as "left" | "right");
      lastStickNav.current = now;
    }

    prevStickDir.current = { x: stickDirX, y: stickDirY };

    // Update previous button state
    const newPrev = new Map<number, boolean>();
    for (const idx of [BUTTON_A, BUTTON_B, DPAD_UP, DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT]) {
      newPrev.set(idx, pressed(idx));
    }
    prevButtons.current = newPrev;
  }, [gamepadActive]);

  // Gamepad polling loop
  useEffect(() => {
    let rafId: number;

    const loop = () => {
      poll();
      rafId = requestAnimationFrame(loop);
    };

    rafId = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(rafId);
  }, [poll]);

  // On gamepadconnected: immediately poll so the activating button press is captured.
  // Also listen for touch/click — Chrome's Gamepad API requires a user gesture
  // before exposing gamepads. On Steam Deck, tapping the touchscreen counts.
  useEffect(() => {
    const onConnect = () => {
      setGamepadActive(true);
      poll();
    };

    const onDisconnect = () => {
      setGamepadActive(false);
    };

    // After any user gesture (tap, click), check if a gamepad appeared
    const onUserGesture = () => {
      if (findGamepad()) {
        setGamepadActive(true);
      }
    };

    window.addEventListener("gamepadconnected", onConnect);
    window.addEventListener("gamepaddisconnected", onDisconnect);
    window.addEventListener("click", onUserGesture);
    window.addEventListener("touchstart", onUserGesture);
    return () => {
      window.removeEventListener("gamepadconnected", onConnect);
      window.removeEventListener("gamepaddisconnected", onDisconnect);
      window.removeEventListener("click", onUserGesture);
      window.removeEventListener("touchstart", onUserGesture);
    };
  }, [poll]);

  // Keyboard fallback (Steam Input may map controls as arrow keys)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      const inTextField = tag === "INPUT" || tag === "TEXTAREA";

      // In text fields: up/down arrows escape the field, other keys stay
      if (inTextField) {
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          (e.target as HTMLElement).blur();
          // Fall through to handle navigation below
        } else {
          return; // Let the input handle typing normally
        }
      }

      switch (e.key) {
        case "ArrowUp":
          e.preventDefault();
          optionsRef.current.onNavigate?.("up");
          break;
        case "ArrowDown":
          e.preventDefault();
          optionsRef.current.onNavigate?.("down");
          break;
        case "ArrowLeft":
          e.preventDefault();
          optionsRef.current.onNavigate?.("left");
          break;
        case "ArrowRight":
          e.preventDefault();
          optionsRef.current.onNavigate?.("right");
          break;
        case "Enter":
          // If a button already has native focus, its onClick fires automatically.
          // Only call onConfirm when focus is NOT on a button (avoids double-trigger).
          if ((e.target as HTMLElement).tagName !== "BUTTON") {
            e.preventDefault();
            optionsRef.current.onConfirm?.();
          }
          break;
        case "Escape":
        case "Backspace":
          e.preventDefault();
          optionsRef.current.onBack?.();
          break;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [gamepadActive]);

  return { gamepadActive };
}
