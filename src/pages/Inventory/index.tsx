import { notifications } from "@mantine/notifications";
// src/pages/Inventory/index.tsx
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { clsx } from "clsx";
import { FeatureGate } from "@/components/ui/FeatureGate";

import { Features, type InventoryBatch, getExpiryStatus } from "@/types";
import type { AddBatchInput } from "@/lib/commands";
import * as cmd from "@/lib/commands";
import { AddBatchModal } from "./AddBatchModal";

const EXPIRY_STYLE: Record<ReturnType<typeof getExpiryStatus>, string> = {
  expired:  "bg-[var(--color-danger-100)] text-[var(--color-danger-600)] font-bold",
  critical: "bg-orange-100 text-orange-700 font-bold",
  warning:  "bg-[var(--color-warn-100)] text-[var(--color-warn-600)]",
  ok:       "bg-[var(--color-ok-100)] text-[var(--color-ok-600)]",
  none:     "bg-gray-100 text-gray-500",
};

const EXPIRY_LABEL: Record<ReturnType<typeof getExpiryStatus>, string> = {
  expired:  "⛔ Expiré",
  critical: "🔴 Critique",
  warning:  "🟡 Attention",
  ok:       "🟢 OK",
  none:     "—",
};

export default function InventoryPage() {
  const { t } = useTranslation();
  

  const [batches,    setBatches]   = useState<InventoryBatch[]>([]);
  const [alerts,     setAlerts]    = useState<InventoryBatch[]>([]);
  const [loading,    setLoading]   = useState(true);
  const [tab,        setTab]       = useState<"all" | "alerts">("all");
  const [addModal,   setAddModal]  = useState(false);
  const [search,     setSearch]    = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [all, warn] = await Promise.all([
        cmd.getInventoryBatches(),
        cmd.getExpiryAlerts(30),
      ]);
      setBatches(all);
      setAlerts(warn);
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => { load(); }, [load]);

  const displayed = (tab === "alerts" ? alerts : batches).filter((b) =>
    !search || b.product_name.toLowerCase().includes(search.toLowerCase()),
  );

  const handleAddBatch = async (input: AddBatchInput) => {
    try {
      await cmd.addInventoryBatch(input);
      notifications.show({ color: "green", message: "Stock ajouté avec succès." });
      setAddModal(false);
      load();
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    }
  };

  return (
    <FeatureGate flag={Features.INVENTORY_MGMT}>
      <div className="flex flex-col h-full p-6 gap-4">

        {/* Header */}
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-bold">{t("inventory.title")}</h1>
          <button className="btn-primary" onClick={() => setAddModal(true)}>
            + {t("inventory.add_batch")}
          </button>
        </div>

        {/* Alert summary strip */}
        {alerts.length > 0 && (
          <div
            className="flex items-center gap-3 px-4 py-3 rounded-xl bg-[var(--color-warn-100)] border border-[var(--color-warn-600)] cursor-pointer"
            onClick={() => setTab("alerts")}
          >
            <span className="text-2xl">⚠️</span>
            <div>
              <p className="font-semibold text-[var(--color-warn-600)] text-sm">
                {alerts.filter((b) => getExpiryStatus(b.days_until_expiry) === "expired").length} batch(s) expirés ·{" "}
                {alerts.filter((b) => ["critical","warning"].includes(getExpiryStatus(b.days_until_expiry))).length} expirant dans les 30 jours
              </p>
              <p className="text-xs text-[var(--color-warn-600)] opacity-70">Cliquez pour voir les alertes</p>
            </div>
          </div>
        )}

        {/* Tabs + search */}
        <div className="flex items-center gap-4">
          <div className="flex rounded-lg border border-[var(--color-border)] overflow-hidden">
            {(["all","alerts"] as const).map((t_) => (
              <button
                key={t_}
                onClick={() => setTab(t_)}
                className={clsx(
                  "px-4 py-2 text-sm font-medium transition-colors",
                  tab === t_
                    ? "bg-[var(--color-brand-600)] text-white"
                    : "bg-white text-[var(--color-text-muted)] hover:bg-[var(--color-surface-dim)]",
                )}
              >
                {t_ === "all" ? `Tous les stocks (${batches.length})` : `Alertes (${alerts.length})`}
              </button>
            ))}
          </div>
          <input
            type="text"
            className="input max-w-xs"
            placeholder="Filtrer par produit…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>

        {/* Table */}
        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <span className="animate-pulse text-[var(--color-text-muted)]">Chargement…</span>
          </div>
        ) : (
          <div className="card overflow-hidden p-0 flex-1 overflow-y-auto">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-[var(--color-surface-dim)] border-b border-[var(--color-border)]">
                <tr className="text-xs uppercase tracking-wide text-[var(--color-text-muted)]">
                  <th className="text-start px-4 py-3">Produit</th>
                  <th className="text-center px-3 py-3">{t("inventory.qty")}</th>
                  <th className="text-center px-3 py-3">{t("inventory.expiry")}</th>
                  <th className="text-center px-3 py-3">Statut</th>
                  <th className="text-start px-3 py-3">{t("inventory.supplier")}</th>
                  <th className="text-start px-3 py-3">{t("inventory.received")}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--color-border)]">
                {displayed.length === 0 ? (
                  <tr>
                    <td colSpan={6} className="text-center py-16 text-[var(--color-text-muted)]">
                      Aucun résultat
                    </td>
                  </tr>
                ) : (
                  displayed.map((b) => {
                    const status = getExpiryStatus(b.days_until_expiry);
                    return (
                      <tr
                        key={b.id}
                        className={clsx(
                          "hover:bg-[var(--color-surface-dim)] transition-colors",
                          (status === "expired" || status === "critical") && "bg-red-50/50",
                        )}
                      >
                        <td className="px-4 py-3 font-medium">{b.product_name}</td>
                        <td className="px-3 py-3 text-center font-mono font-semibold">
                          {b.quantity % 1 === 0 ? b.quantity.toFixed(0) : b.quantity.toFixed(2)}
                        </td>
                        <td className="px-3 py-3 text-center">
                          {b.expiry_date ? (
                            <span className="font-mono text-xs">{b.expiry_date}</span>
                          ) : (
                            <span className="text-[var(--color-text-muted)]">—</span>
                          )}
                          {b.days_until_expiry !== null && (
                            <p className="text-xs mt-0.5 text-[var(--color-text-muted)]">
                              {b.days_until_expiry >= 0
                                ? `dans ${b.days_until_expiry}j`
                                : `il y a ${Math.abs(b.days_until_expiry)}j`}
                            </p>
                          )}
                        </td>
                        <td className="px-3 py-3 text-center">
                          <span className={clsx("text-xs px-2 py-0.5 rounded-full", EXPIRY_STYLE[status])}>
                            {EXPIRY_LABEL[status]}
                          </span>
                        </td>
                        <td className="px-3 py-3 text-[var(--color-text-muted)]">
                          {b.supplier_ref ?? "—"}
                        </td>
                        <td className="px-3 py-3 text-xs text-[var(--color-text-muted)]">
                          {b.received_at.slice(0, 10)}
                        </td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>
        )}

        {addModal && (
          <AddBatchModal onSave={handleAddBatch} onClose={() => setAddModal(false)} />
        )}
      </div>
    </FeatureGate>
  );
}