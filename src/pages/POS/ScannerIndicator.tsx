// src/pages/POS/ScannerIndicator.tsx
// Visible "Scanner Ready" indicator using Mantine Badge + animated pulse.

import { Badge, Tooltip } from "@mantine/core";
import {
  IconBarcode,
  IconBarcodeOff,
  IconCheck
} from "@tabler/icons-react";
import { useBarcodeContext } from "@/providers/BarcodeProvider";
import type { ScanStatus } from "@/types";

const CONFIG: Record<ScanStatus, {
  color:   string;
  label:   string;
  icon:    React.ReactNode;
  pulse:   boolean;
}> = {
  idle:     { color: "green",  label: "Scanner Prêt",     icon: <IconBarcode size={14} />,    pulse: true  },
  scanning: { color: "blue",   label: "Lecture en cours", icon: <IconBarcode size={14} />,    pulse: true  },
  success:  { color: "teal",   label: "Produit trouvé",   icon: <IconCheck size={14} />,      pulse: false },
  error:    { color: "red",    label: "Code inconnu",     icon: <IconBarcodeOff size={14} />, pulse: false },
};

export function ScannerIndicator() {
  const { scanStatus, lastBarcode } = useBarcodeContext();
  const cfg = CONFIG[scanStatus];

  return (
    <Tooltip
      label={lastBarcode ? `Dernier scan : ${lastBarcode}` : "En attente d'un scan…"}
      position="bottom"
      withArrow
    >
      <Badge
        color={cfg.color}
        variant={scanStatus === "idle" ? "light" : "filled"}
        size="lg"
        radius="sm"
        leftSection={cfg.icon}
        style={{
          cursor: "default",
          userSelect: "none",
          animation: cfg.pulse ? "scanner-pulse 2s ease-in-out infinite" : undefined,
        }}
      >
        {cfg.label}
      </Badge>
    </Tooltip>
  );
}

// ── Inject the @keyframes animation once globally (avoids a separate CSS file)
if (typeof document !== "undefined") {
  const styleId = "scanner-pulse-anim";
  if (!document.getElementById(styleId)) {
    const style = document.createElement("style");
    style.id = styleId;
    style.textContent = `
      @keyframes scanner-pulse {
        0%, 100% { opacity: 1; }
        50%       { opacity: 0.55; }
      }
    `;
    document.head.appendChild(style);
  }
}