// src/components/LicenseGuard.tsx
//
// Wraps the entire app router. If `license.is_valid === false`, renders
// the ActivationPage exclusively — all other routes are inaccessible.
//
// This is the React enforcement layer. The Rust layer independently enforces
// feature flags inside every premium Tauri command, so bypassing this
// component in DevTools would still hit a Rust wall.

import { useLicenseStore } from "@/store/licenseStore";
import ActivationPage      from "@/pages/Activation";

interface Props {
  children: React.ReactNode;
}

export function LicenseGuard({ children }: Props) {
  const isValid = useLicenseStore((s) => s.license.is_valid);

  if (!isValid) {
    return <ActivationPage />;
  }

  return <>{children}</>;
}