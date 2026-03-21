// src/pages/Products/ProductFormModal.tsx
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Product, CreateProductInput } from "@/types";

interface Props {
  product: Product | null;   // null = create mode
  onSave:  (input: CreateProductInput, id?: number) => Promise<void>;
  onClose: () => void;
}

const CATEGORY_OPTIONS = [
  { id: 1, name: "Divers" },
  { id: 2, name: "Épicerie" },
  { id: 3, name: "Boulangerie" },
  { id: 4, name: "Boucherie" },
  { id: 5, name: "Produits laitiers" },
  { id: 6, name: "Hygiène" },
  { id: 7, name: "Boissons" },
];

const UNIT_OPTIONS = [
  { id: 1, label: "Pièce" },
  { id: 2, label: "Kg" },
  { id: 3, label: "Litre" },
  { id: 4, label: "Carton" },
];

const BLANK: CreateProductInput = {
  gtin:            null,
  name_fr:         "",
  name_ar:         "",
  category_id:     1,
  unit_id:         1,
  sell_price:      0,
  buy_price:       0,
  vat_rate:        0.19,
  min_stock_alert: 5,
};

export function ProductFormModal({ product, onSave, onClose }: Props) {
  const { t } = useTranslation();
  const [form,    setForm]    = useState<CreateProductInput>(BLANK);
  const [saving,  setSaving]  = useState(false);
  const [errors,  setErrors]  = useState<Partial<Record<keyof CreateProductInput, string>>>({});

  useEffect(() => {
    if (product) {
      setForm({
        gtin:            product.gtin,
        name_fr:         product.name_fr,
        name_ar:         product.name_ar,
        category_id:     product.category_id,
        unit_id:         product.unit_id,
        sell_price:      product.sell_price,
        buy_price:       product.buy_price,
        vat_rate:        product.vat_rate,
        min_stock_alert: product.min_stock_alert,
      });
    } else {
      setForm(BLANK);
    }
    setErrors({});
  }, [product]);

  const set = <K extends keyof CreateProductInput>(key: K, val: CreateProductInput[K]) =>
    setForm((f) => ({ ...f, [key]: val }));

  const validate = (): boolean => {
    const e: typeof errors = {};
    if (!form.name_fr.trim()) e.name_fr = "Nom (FR) requis";
    if (form.sell_price < 0)  e.sell_price = "Prix invalide";
    if (form.buy_price < 0)   e.buy_price  = "Prix d'achat invalide";
    setErrors(e);
    return Object.keys(e).length === 0;
  };

  const handleSubmit = async () => {
    if (!validate() || saving) return;
    setSaving(true);
    try {
      await onSave(form, product?.id);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
      <div className="card w-full max-w-2xl shadow-2xl max-h-[90vh] flex flex-col">

        {/* Header */}
        <div className="flex items-center justify-between mb-5">
          <h2 className="text-xl font-bold">
            {product ? t("products.edit") : t("products.add")}
          </h2>
          <button onClick={onClose} className="text-2xl text-[var(--color-text-muted)] hover:text-[var(--color-text)] leading-none">×</button>
        </div>

        {/* Scrollable body */}
        <div className="overflow-y-auto flex-1 space-y-4 pe-1">

          {/* Barcode */}
          <div>
            <label className="text-sm font-medium block mb-1">{t("products.gtin")}</label>
            <input
              className="input font-mono"
              value={form.gtin ?? ""}
              onChange={(e) => set("gtin", e.target.value || null)}
              placeholder="Ex: 6191234567890"
            />
          </div>

          {/* Names (two columns) */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-sm font-medium block mb-1">
                {t("products.name_fr")} <span className="text-[var(--color-danger-600)]">*</span>
              </label>
              <input
                className={`input ${errors.name_fr ? "border-red-400" : ""}`}
                value={form.name_fr}
                onChange={(e) => set("name_fr", e.target.value)}
                placeholder="Nom en français"
                autoFocus
              />
              {errors.name_fr && <p className="text-xs text-[var(--color-danger-600)] mt-1">{errors.name_fr}</p>}
            </div>

            <div>
              <label className="text-sm font-medium block mb-1 text-right" dir="rtl">
                {t("products.name_ar")}
              </label>
              <input
                className="input text-right"
                dir="rtl"
                value={form.name_ar}
                onChange={(e) => set("name_ar", e.target.value)}
                placeholder="الاسم بالعربية"
              />
            </div>
          </div>

          {/* Category + Unit */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-sm font-medium block mb-1">{t("products.category")}</label>
              <select
                className="input"
                value={form.category_id ?? 1}
                onChange={(e) => set("category_id", parseInt(e.target.value))}
              >
                {CATEGORY_OPTIONS.map((c) => (
                  <option key={c.id} value={c.id}>{c.name}</option>
                ))}
              </select>
            </div>

            <div>
              <label className="text-sm font-medium block mb-1">{t("products.unit")}</label>
              <select
                className="input"
                value={form.unit_id ?? 1}
                onChange={(e) => set("unit_id", parseInt(e.target.value))}
              >
                {UNIT_OPTIONS.map((u) => (
                  <option key={u.id} value={u.id}>{u.label}</option>
                ))}
              </select>
            </div>
          </div>

          {/* Prices + VAT */}
          <div className="grid grid-cols-3 gap-3">
            <div>
              <label className="text-sm font-medium block mb-1">
                {t("products.sell_price")} <span className="text-[var(--color-danger-600)]">*</span>
              </label>
              <input
                type="number"
                min="0"
                step="0.01"
                className={`input ${errors.sell_price ? "border-red-400" : ""}`}
                value={form.sell_price}
                onChange={(e) => set("sell_price", parseFloat(e.target.value) || 0)}
              />
            </div>

            <div>
              <label className="text-sm font-medium block mb-1">{t("products.buy_price")}</label>
              <input
                type="number"
                min="0"
                step="0.01"
                className="input"
                value={form.buy_price}
                onChange={(e) => set("buy_price", parseFloat(e.target.value) || 0)}
              />
            </div>

            <div>
              <label className="text-sm font-medium block mb-1">{t("products.vat_rate")}</label>
              <select
                className="input"
                value={form.vat_rate}
                onChange={(e) => set("vat_rate", parseFloat(e.target.value))}
              >
                <option value={0}>0%</option>
                <option value={0.09}>9%</option>
                <option value={0.19}>19%</option>
              </select>
            </div>
          </div>

          {/* Margin display */}
          {form.sell_price > 0 && form.buy_price > 0 && (
            <div className="bg-[var(--color-surface-dim)] rounded-lg px-4 py-2 text-sm flex gap-6">
              <span className="text-[var(--color-text-muted)]">Marge brute :</span>
              <span className="font-semibold text-[var(--color-ok-600)]">
                {((form.sell_price - form.buy_price)).toFixed(2)} DZD
                ({form.buy_price > 0
                  ? (((form.sell_price - form.buy_price) / form.buy_price) * 100).toFixed(1)
                  : "—"}%)
              </span>
            </div>
          )}

          {/* Min stock alert */}
          <div>
            <label className="text-sm font-medium block mb-1">{t("products.min_stock")}</label>
            <input
              type="number"
              min="0"
              step="1"
              className="input w-32"
              value={form.min_stock_alert}
              onChange={(e) => set("min_stock_alert", parseInt(e.target.value) || 0)}
            />
            <p className="text-xs text-[var(--color-text-muted)] mt-1">
              Une alerte apparaît quand le stock descend sous ce seuil.
            </p>
          </div>
        </div>

        {/* Footer */}
        <div className="flex gap-3 justify-end mt-5 pt-4 border-t border-[var(--color-border)]">
          <button className="btn-ghost" onClick={onClose}>{t("products.cancel")}</button>
          <button className="btn-primary" onClick={handleSubmit} disabled={saving}>
            {saving ? "⏳ Enregistrement…" : t("products.save")}
          </button>
        </div>
      </div>
    </div>
  );
}