// src/pages/Inventory/AddBatchModal.tsx
import { useEffect, useState } from "react";
import type { AddBatchInput } from "@/lib/commands";
import type { Product } from "@/types";
import * as cmd from "@/lib/commands";

interface Props {
  onSave:  (input: AddBatchInput) => Promise<void>;
  onClose: () => void;
}

export function AddBatchModal({ onSave, onClose }: Props) {
  const [products,  setProducts]  = useState<Product[]>([]);
  const [productId, setProductId] = useState<number | "">("");
  const [qty,       setQty]       = useState<string>("1");
  const [expiry,    setExpiry]    = useState("");
  const [supplier,  setSupplier]  = useState("");
  const [cost,      setCost]      = useState<string>("");
  const [saving,    setSaving]    = useState(false);

  useEffect(() => {
    cmd.getProducts().then(setProducts).catch(console.error);
  }, []);

  const handleSave = async () => {
    if (!productId || !qty || saving) return;
    setSaving(true);
    try {
      await onSave({
        product_id:   productId as number,
        quantity:     parseFloat(qty),
        expiry_date:  expiry || null,
        supplier_ref: supplier || null,
        cost_price:   cost ? parseFloat(cost) : null,
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="card w-full max-w-lg shadow-2xl">
        <div className="flex justify-between items-center mb-5">
          <h2 className="text-xl font-bold">Entrée de stock</h2>
          <button onClick={onClose} className="text-2xl text-[var(--color-text-muted)] leading-none">×</button>
        </div>

        <div className="space-y-4">
          {/* Product selector */}
          <div>
            <label className="text-sm font-medium block mb-1">Produit *</label>
            <select
              className="input"
              value={productId}
              onChange={(e) => setProductId(e.target.value ? parseInt(e.target.value) : "")}
              autoFocus
            >
              <option value="">— Sélectionner un produit —</option>
              {products.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name_fr} {p.gtin ? `(${p.gtin})` : ""}
                </option>
              ))}
            </select>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-sm font-medium block mb-1">Quantité *</label>
              <input
                type="number"
                min="0.01"
                step="0.01"
                className="input"
                value={qty}
                onChange={(e) => setQty(e.target.value)}
              />
            </div>
            <div>
              <label className="text-sm font-medium block mb-1">Prix d'achat (DZD)</label>
              <input
                type="number"
                min="0"
                step="0.01"
                className="input"
                value={cost}
                onChange={(e) => setCost(e.target.value)}
                placeholder="Optionnel"
              />
            </div>
          </div>

          <div>
            <label className="text-sm font-medium block mb-1">Date d'expiration</label>
            <input
              type="date"
              className="input"
              value={expiry}
              onChange={(e) => setExpiry(e.target.value)}
              min={new Date().toISOString().slice(0, 10)}
            />
          </div>

          <div>
            <label className="text-sm font-medium block mb-1">Référence fournisseur</label>
            <input
              type="text"
              className="input"
              value={supplier}
              onChange={(e) => setSupplier(e.target.value)}
              placeholder="Optionnel"
            />
          </div>
        </div>

        <div className="flex gap-3 justify-end mt-5 pt-4 border-t border-[var(--color-border)]">
          <button className="btn-ghost" onClick={onClose}>Annuler</button>
          <button
            className="btn-primary"
            onClick={handleSave}
            disabled={!productId || !qty || saving}
          >
            {saving ? "⏳ Enregistrement…" : "Enregistrer l'entrée"}
          </button>
        </div>
      </div>
    </div>
  );
}