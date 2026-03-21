import { notifications } from "@mantine/notifications";
// src/pages/Dain/index.tsx
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { clsx } from "clsx";
import { FeatureGate } from "@/components/ui/FeatureGate";

import { Features, type CustomerDainSummary, type DainEntry } from "@/types";
import * as cmd from "@/lib/commands";

export default function DainPage() {
  const { t } = useTranslation();

  const [phone,    setPhone]    = useState("");
  const [customer, setCustomer] = useState<CustomerDainSummary | null>(null);
  const [history,  setHistory]  = useState<DainEntry[]>([]);
  const [loading,  setLoading]  = useState(false);
  const [notFound, setNotFound] = useState(false);

  // Entry form state
  const [mode,     setMode]     = useState<"debt" | "repayment" | null>(null);
  const [amount,   setAmount]   = useState("");
  const [notes,    setNotes]    = useState("");
  const [saving,   setSaving]   = useState(false);

  const search = async () => {
    if (!phone.trim()) return;
    setLoading(true);
    setNotFound(false);
    try {
      const c = await cmd.getCustomerDain(phone.trim());
      setCustomer(c);
      const h = await cmd.getDainHistory(c.customer_id);
      setHistory(h);
    } catch {
      setCustomer(null);
      setHistory([]);
      setNotFound(true);
    } finally {
      setLoading(false);
    }
  };

  const handleEntry = async () => {
    if (!customer || !mode || !amount || saving) return;
    setSaving(true);
    try {
      if (mode === "debt") {
        await cmd.addDainEntry(customer.customer_id, null, parseFloat(amount), notes || null);
      } else {
        await cmd.repayDain(customer.customer_id, parseFloat(amount), notes || null);
      }
      notifications.show({ color: "green", message: mode === "debt" ? "Dette enregistrée." : "Remboursement enregistré." });
      setMode(null);
      setAmount("");
      setNotes("");
      // Refresh
      const [c, h] = await Promise.all([
        cmd.getCustomerDain(phone.trim()),
        cmd.getDainHistory(customer.customer_id),
      ]);
      setCustomer(c);
      setHistory(h);
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <FeatureGate flag={Features.DAIN_LEDGER}>
      <div className="p-6 max-w-3xl mx-auto flex flex-col gap-6">

        <h1 className="text-2xl font-bold">{t("dain.title")}</h1>

        {/* Search */}
        <div className="flex gap-3">
          <input
            type="tel"
            className="input flex-1"
            placeholder={t("dain.search_phone")}
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && search()}
            autoFocus
          />
          <button className="btn-primary" onClick={search} disabled={loading}>
            {loading ? "⏳" : "🔍 Rechercher"}
          </button>
        </div>

        {notFound && (
          <p className="text-[var(--color-danger-600)] text-sm">
            ⚠ {t("dain.no_customer")}
          </p>
        )}

        {customer && (
          <>
            {/* Customer card */}
            <div className="card">
              <div className="flex items-start justify-between">
                <div>
                  <h2 className="text-lg font-bold">{customer.name}</h2>
                  <p className="text-[var(--color-text-muted)] text-sm">📞 {customer.phone}</p>
                </div>
                <div className={clsx(
                  "text-right rounded-xl px-4 py-2",
                  customer.balance > 0
                    ? "bg-[var(--color-danger-100)] text-[var(--color-danger-600)]"
                    : "bg-[var(--color-ok-100)] text-[var(--color-ok-600)]",
                )}>
                  <p className="text-xs font-medium">{t("dain.balance")}</p>
                  <p className="text-2xl font-bold">{customer.balance.toFixed(2)} DZD</p>
                </div>
              </div>

              {/* Action buttons */}
              <div className="flex gap-3 mt-4 pt-4 border-t border-[var(--color-border)]">
                <button
                  className="btn-danger flex-1 py-2"
                  onClick={() => setMode(mode === "debt" ? null : "debt")}
                >
                  + {t("dain.add_debt")}
                </button>
                <button
                  className="btn-primary flex-1 py-2"
                  style={{ background: "var(--color-ok-600)" }}
                  onClick={() => setMode(mode === "repayment" ? null : "repayment")}
                >
                  ✓ {t("dain.add_repay")}
                </button>
              </div>

              {/* Entry form */}
              {mode && (
                <div className="mt-4 pt-4 border-t border-[var(--color-border)] space-y-3">
                  <p className="font-semibold text-sm">
                    {mode === "debt" ? "Nouvelle dette" : "Remboursement"}
                  </p>
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="text-xs font-medium block mb-1">{t("dain.amount")}</label>
                      <input
                        type="number"
                        min="0.01"
                        step="0.01"
                        className="input"
                        value={amount}
                        onChange={(e) => setAmount(e.target.value)}
                        autoFocus
                      />
                    </div>
                    <div>
                      <label className="text-xs font-medium block mb-1">{t("dain.notes")}</label>
                      <input
                        type="text"
                        className="input"
                        value={notes}
                        onChange={(e) => setNotes(e.target.value)}
                        placeholder="Optionnel"
                      />
                    </div>
                  </div>
                  <div className="flex gap-2 justify-end">
                    <button className="btn-ghost text-sm" onClick={() => { setMode(null); setAmount(""); setNotes(""); }}>
                      Annuler
                    </button>
                    <button className="btn-primary text-sm" onClick={handleEntry} disabled={!amount || saving}>
                      {saving ? "⏳" : "Confirmer"}
                    </button>
                  </div>
                </div>
              )}
            </div>

            {/* History */}
            <div className="card">
              <h3 className="font-semibold mb-3">{t("dain.history")}</h3>
              {history.length === 0 ? (
                <p className="text-[var(--color-text-muted)] text-sm text-center py-6">Aucune transaction</p>
              ) : (
                <div className="space-y-2">
                  {history.map((e) => (
                    <div
                      key={e.id}
                      className={clsx(
                        "flex items-center justify-between rounded-lg px-3 py-2 text-sm",
                        e.entry_type === "debt"
                          ? "bg-[var(--color-danger-100)]"
                          : "bg-[var(--color-ok-100)]",
                      )}
                    >
                      <div>
                        <span className={clsx(
                          "font-medium",
                          e.entry_type === "debt"
                            ? "text-[var(--color-danger-600)]"
                            : "text-[var(--color-ok-600)]",
                        )}>
                          {e.entry_type === "debt" ? `+ ${t("dain.debt")}` : `− ${t("dain.repayment")}`}
                        </span>
                        {e.notes && <span className="ms-2 text-[var(--color-text-muted)]">— {e.notes}</span>}
                        <p className="text-xs text-[var(--color-text-muted)]">{e.created_at.slice(0, 16).replace("T", " ")}</p>
                      </div>
                      <span className="font-bold font-mono">
                        {e.entry_type === "debt" ? "+" : "−"}{e.amount.toFixed(2)} DZD
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </FeatureGate>
  );
}
