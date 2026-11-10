import { Keyboard } from "lucide-react";

/** Placeholder brand mark until Keygloss has its own icon. */
export function Brand({ size = 28 }: { size?: number }) {
  return (
    <span aria-hidden style={{ width: size, height: size, borderRadius: size * 0.28 }}
      className="grid shrink-0 place-items-center border border-fg/10 bg-gradient-to-b from-raised to-fg/[0.06] text-accent shadow-[0_1px_2px_rgb(0_0_0/0.12)]">
      <Keyboard size={size * 0.56} strokeWidth={1.9} />
    </span>
  );
}
