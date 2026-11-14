import logo from "../../src-tauri/icons/icon.svg";

export function Brand({ size = 28 }: { size?: number }) {
  return <img src={logo} alt="" aria-hidden width={size} height={size} draggable={false}
    className="pointer-events-none shrink-0" />;
}
