// src/components/ui/FeatureGate.tsx
// Renders children only if the active license includes the required feature flag.
// Shows a professional "upgrade required" placeholder otherwise.

// import { useTranslation } from "react-i18next";
import { useLicenseStore } from "@/store/licenseStore";

interface FeatureGateProps {
  flag:       number;
  children:   React.ReactNode;
  fallback?:  React.ReactNode;
}

export function FeatureGate({ flag, children, fallback }: FeatureGateProps) {
  const can = useLicenseStore((s) => s.can);

  if (can(flag)) return <>{children}</>;

  if (fallback) return <>{fallback}</>;

  return <LockedFeaturePlaceholder />;
}

function LockedFeaturePlaceholder() {
//   const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center justify-center h-64 gap-4 opacity-60">
      <div className="text-5xl">🔒</div>
      <p className="font-semibold text-lg text-brand-700">Fonctionnalité Premium</p>
      <p className="text-sm text-muted text-center max-w-xs">
        Cette fonctionnalité n'est pas incluse dans votre licence actuelle.
        Contactez votre revendeur pour mettre à niveau.
      </p>
    </div>
  );
}