// src/store/licenseStore.ts
import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { LicenseState } from "@/types";
import { hasFeature } from "@/types";

interface LicenseStore {
  license:    LicenseState;
  setLicense: (l: LicenseState) => void;
  can:        (flag: number) => boolean;
}

const defaultLicense: LicenseState = {
  is_valid:   false,
  tier:       "",
  features:   0,
  expires_at: null,
};

export const useLicenseStore = create<LicenseStore>()(
  persist(
    (set, get) => ({
      license: defaultLicense,

      setLicense: (l) => set({ license: l }),

      /** Quick helper for feature-gating React renders */
      can: (flag) => hasFeature(get().license, flag),
    }),
    {
      name:    "superpos-license",
      storage: createJSONStorage(() => localStorage),
    },
  ),
);