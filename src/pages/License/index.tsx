import { notifications } from "@mantine/notifications";
// src/pages/License/index.tsx
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useLicenseStore } from "@/store/licenseStore";

import * as cmd from "@/lib/commands";
import { clsx } from "clsx";

export default function LicensePage() {
  const { t } = useTranslation();
  
  const { license, setLicense } = useLicenseStore();

  const [hwid, setHwid]       = useState("Chargement…");
  const [blob, setBlob]       = useState("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    cmd.getHwid()
      .then(setHwid)
      .catch((e) => setHwid(`Erreur: ${e}`));
  }, []);

  const handleActivate = async () => {
    if (!blob.trim()) { notifications.show({ color: "red", message: "Entrez une clé de licence." }); return; }
    setLoading(true);
    try {
      const state = await cmd.verifyLicense(blob.trim());
      setLicense(state);
      notifications.show({ color: "green", message: "Licence activée avec succès !" });
      setBlob("");
    } catch (e) {
      notifications.show({ color: "red", message: `Activation échouée : ${e}` });
    } finally {
      setLoading(false);
    }
  };

  const copyHwid = () => {
    navigator.clipboard.writeText(hwid);
    notifications.show({ color: "green", message: "HWID copié dans le presse-papier !" });
  };

  const tierColor: Record<string, string> = {
    basic:        "badge-ok",
    professional: "badge-warn",
    enterprise:   "bg-[var(--color-brand-100)] text-[var(--color-brand-700)] px-2 py-0.5 rounded-full text-xs font-semibold",
  };

  return (
    <div className="p-8 max-w-2xl mx-auto">
      <h1 className="text-2xl font-bold mb-6">{t("license.title")}</h1>

      {/* Current status */}
      <div className={clsx("card mb-6", license.is_valid ? "border-green-200" : "border-red-200")}>
        <div className="flex items-center gap-3 mb-3">
          <span className="text-3xl">{license.is_valid ? "✅" : "⚠️"}</span>
          <div>
            <p className="font-semibold text-lg">
              {license.is_valid ? t("license.valid") : t("license.invalid")}
            </p>
            {license.is_valid && (
              <div className="flex items-center gap-2 mt-1">
                <span className={tierColor[license.tier] ?? "badge-ok"}>
                  {license.tier.toUpperCase()}
                </span>
                <span className="text-sm text-[var(--color-text-muted)]">
                  {license.expires_at
                    ? t("license.expires", { date: license.expires_at })
                    : t("license.perpetual")}
                </span>
              </div>
            )}
          </div>
        </div>

        {/* Feature flags */}
        {license.is_valid && (
          <div className="mt-3 pt-3 border-t border-[var(--color-border)] grid grid-cols-2 gap-1.5">
            {[
              [1,   "POS de base"],
              [2,   "Gestion des stocks"],
              [4,   "Impression thermique"],
              [8,   "Dain (crédit)"],
              [16,  "Rapports A4"],
              [32,  "Multi-caisse"],
              [64,  "Analytiques avancés"],
            ].map(([flag, label]) => {
              const active = (license.features & (flag as number)) !== 0;
              return (
                <div key={flag as number} className={clsx(
                  "flex items-center gap-2 text-sm px-3 py-1.5 rounded-md",
                  active ? "bg-green-50 text-green-800" : "bg-gray-50 text-gray-400",
                )}>
                  <span>{active ? "✓" : "○"}</span>
                  <span>{label as string}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* HWID */}
      <div className="card mb-6">
        <label className="text-sm font-semibold block mb-2">{t("license.hwid_label")}</label>
        <div className="flex items-center gap-2">
          <code className="flex-1 bg-[var(--color-surface-dim)] px-3 py-2 rounded-lg text-sm font-mono break-all border border-[var(--color-border)]">
            {hwid}
          </code>
          <button className="btn-ghost shrink-0" onClick={copyHwid}>
            📋 {t("license.copy_hwid")}
          </button>
        </div>
        <p className="text-xs text-[var(--color-text-muted)] mt-2">
          Communiquez cet identifiant à votre revendeur pour obtenir une clé de licence.
        </p>
      </div>

      {/* Activation */}
      <div className="card">
        <label className="text-sm font-semibold block mb-2">{t("license.enter_key")}</label>
        <textarea
          className="input font-mono text-sm h-28 resize-none"
          value={blob}
          onChange={(e) => setBlob(e.target.value)}
          placeholder="Collez votre clé de licence ici…"
          spellCheck={false}
        />
        <button
          className="btn-primary mt-3 w-full py-2.5"
          onClick={handleActivate}
          disabled={loading}
        >
          {loading ? "⏳ Activation…" : t("license.activate")}
        </button>
      </div>
    </div>
  );
}