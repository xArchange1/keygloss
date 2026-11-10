import { create } from "zustand";
import * as ipc from "./lib/ipc";
import type { EngineStatus, Settings } from "./lib/ipc";

interface Store {
  settings: Settings | null;
  status: EngineStatus | null;
  /** Optimistic shallow merge; the backend's answer (or `settings-changed`) reconciles. */
  update: (patch: Partial<Settings>) => void;
}

export const useStore = create<Store>((set) => ({
  settings: null,
  status: null,
  update: (patch) => {
    set((s) => ({ settings: s.settings && { ...s.settings, ...patch } }));
    ipc
      .updateSettings(patch)
      .then((settings) => set({ settings }))
      .catch((e) => {
        console.error("update_settings failed", e);
        ipc.getSettings().then((settings) => set({ settings }));
      });
  },
}));

/** Loads settings and status once and keeps them in sync with backend events. */
export function initStore() {
  ipc.getSettings().then((settings) => useStore.setState({ settings }));
  ipc.getEngineStatus().then((status) => useStore.setState({ status }));
  ipc.on("settings-changed", (settings) => useStore.setState({ settings }));
  ipc.on("engine-status", (status) => useStore.setState({ status }));
}

/** Non-null settings for components rendered after the initial load. */
export const useSettings = () => useStore((s) => s.settings!);
export const useUpdate = () => useStore((s) => s.update);
