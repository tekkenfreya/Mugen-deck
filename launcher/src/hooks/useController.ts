import { useCallback, useEffect, useRef } from "react";

/** Gamepad button indices (standard mapping). */
const BUTTON_A = 0;
const BUTTON_B = 1;
const DPAD_UP = 12;
const DPAD_DOWN = 13;
const DPAD_LEFT = 14;
const DPAD_RIGHT = 15;

interface UseControllerOptions {
  onNavigate?: (direction: "up" | "down" | "left" | "right") => void;
  onConfirm?: () => void;
  onBack?: () => void;
}

/** Polls the Gamepad API each frame and fires navigation callbacks. */
export function useController(options: UseControllerOptions): void {
  const prevButtons = useRef<Map<number, boolean>>(new Map());
  const optionsRef = useRef(options);
  optionsRef.current = options;

  const poll = useCallback(() => {
    const gamepads = navigator.getGamepads();
    const gp = gamepads[0];
    if (!gp) return;

    const pressed = (index: number): boolean => {
      const btn = gp.buttons[index];
      return btn !== undefined && btn.pressed;
    };

    const justPressed = (index: number): boolean => {
      const isPressed = pressed(index);
      const wasPressed = prevButtons.current.get(index) ?? false;
      return isPressed && !wasPressed;
    };

    if (justPressed(DPAD_UP)) optionsRef.current.onNavigate?.("up");
    if (justPressed(DPAD_DOWN)) optionsRef.current.onNavigate?.("down");
    if (justPressed(DPAD_LEFT)) optionsRef.current.onNavigate?.("left");
    if (justPressed(DPAD_RIGHT)) optionsRef.current.onNavigate?.("right");
    if (justPressed(BUTTON_A)) optionsRef.current.onConfirm?.();
    if (justPressed(BUTTON_B)) optionsRef.current.onBack?.();

    // Update previous state
    const newPrev = new Map<number, boolean>();
    for (const idx of [BUTTON_A, BUTTON_B, DPAD_UP, DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT]) {
      newPrev.set(idx, pressed(idx));
    }
    prevButtons.current = newPrev;
  }, []);

  useEffect(() => {
    let rafId: number;

    const loop_ = () => {
      poll();
      rafId = requestAnimationFrame(loop_);
    };

    rafId = requestAnimationFrame(loop_);
    return () => cancelAnimationFrame(rafId);
  }, [poll]);
}
