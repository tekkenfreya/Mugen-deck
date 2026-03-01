import { useController } from "@/hooks/useController";

interface ControllerNavProps {
  onBack?: () => void;
}

/** Global controller navigation wrapper. Handles B-button for back. */
export function ControllerNav({ onBack }: ControllerNavProps) {
  useController({
    onBack,
  });

  return null;
}
