// src/store/settingsStore.ts
import { create } from "zustand";
import type { AppSettings } from "@/types";

interface SettingsStore {
  settings:    AppSettings | null;
  setSettings: (s: AppSettings) => void;
  getSetting:  (key: keyof AppSettings) => string;
}

export const useSettingsStore = create<SettingsStore>()((set, get) => ({
  settings: null,

  setSettings: (s) => set({ settings: s }),

  getSetting: (key) => {
    const s = get().settings;
    return s ? (s[key] ?? "") : "";
  },
}));