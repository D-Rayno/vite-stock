import { notifications } from "@mantine/notifications";
// src/pages/Products/index.tsx
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { clsx } from "clsx";

import type { Product, CreateProductInput } from "@/types";
import * as cmd from "@/lib/commands";
import { ProductFormModal } from "./ProductFormModal";

export default function ProductsPage() {
  const { t } = useTranslation();

  const [products, setProducts]   = useState<Product[]>([]);
  const [filtered, setFiltered]   = useState<Product[]>([]);
  const [search,   setSearch]     = useState("");
  const [loading,  setLoading]    = useState(true);
  const [modal,    setModal]      = useState<"create" | "edit" | null>(null);
  const [editing,  setEditing]    = useState<Product | null>(null);
  const [deleting, setDeleting]   = useState<number | null>(null);

  // ── Load ───────────────────────────────────────────────────────────────────

  // `notifications` from Mantine is a stable module-level singleton —
  // it does not need to appear in the useCallback dependency array.
  const loadProducts = useCallback(async () => {
    setLoading(true);
    try {
      const data = await cmd.getProducts();
      setProducts(data);
      setFiltered(data);
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally {
      setLoading(false);
    }
  }, []); // ← stable: no external reactive deps

  useEffect(() => { loadProducts(); }, [loadProducts]);

  // ── Search ─────────────────────────────────────────────────────────────────

  useEffect(() => {
    const q = search.toLowerCase();
    if (!q) { setFiltered(products); return; }
    setFiltered(
      products.filter(
        (p) =>
          p.name_fr.toLowerCase().includes(q) ||
          p.name_ar.includes(q) ||
          (p.gtin ?? "").includes(q) ||
          (p.category_name_fr ?? "").toLowerCase().includes(q),
      ),
    );
  }, [search, products]);

  // ── CRUD ───────────────────────────────────────────────────────────────────

  const handleSave = async (input: CreateProductInput, id?: number) => {
    try {
      if (id) {
        await cmd.updateProduct({ ...input, id, is_active: true, total_stock: 0, created_at: "" });
        notifications.show({ color: "green", message: "Produit mis à jour." });
      } else {
        await cmd.createProduct(input);
        notifications.show({ color: "green", message: "Produit créé." });
      }
      setModal(null);
      setEditing(null);
      loadProducts();
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    }
  };

  const handleDelete = async (id: number) => {
    try {
      await cmd.deleteProduct(id);
      notifications.show({ color: "green", message: "Produit désactivé." });
      setDeleting(null);
      loadProducts();
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    }
  };

  // ── Stock status ───────────────────────────────────────────────────────────

  const stockBadge = (p: Product) => {
    if (p.total_stock <= 0)                return <span className="badge-danger">Rupture</span>;
    if (p.total_stock <= p.min_stock_alert) return <span className="badge-warn">Bas</span>;
    return <span className="badge-ok">OK</span>;
  };

  return (
    <div className="flex flex-col h-full p-6 gap-4">
      {/* ── Header ─────────────────────────────────────────────────────────── */}
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("products.title")}</h1>
        <button
          className="btn-primary gap-2"
          onClick={() => { setEditing(null); setModal("create"); }}
        >
          <span>+</span> {t("products.add")}
        </button>
      </div>

      {/* ── Search bar ─────────────────────────────────────────────────────── */}
      <input
        type="text"
        className="input max-w-sm"
        placeholder={t("products.search")}
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        autoFocus
      />

      {/* ── Table ──────────────────────────────────────────────────────────── */}
      {loading ? (
        <div className="flex-1 flex items-center justify-center text-[var(--color-text-muted)]">
          <span className="animate-pulse">Chargement…</span>
        </div>
      ) : (
        <div className="card overflow-hidden p-0 flex-1 overflow-y-auto">
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-[var(--color-surface-dim)] border-b border-[var(--color-border)]">
              <tr className="text-xs uppercase tracking-wide text-[var(--color-text-muted)]">
                <th className="text-start px-4 py-3">Produit</th>
                <th className="text-start px-3 py-3">{t("products.gtin")}</th>
                <th className="text-start px-3 py-3">{t("products.category")}</th>
                <th className="text-end   px-3 py-3">{t("products.sell_price")}</th>
                <th className="text-end   px-3 py-3">{t("products.buy_price")}</th>
                <th className="text-center px-3 py-3">{t("products.stock")}</th>
                <th className="text-center px-3 py-3">Statut</th>
                <th className="px-3 py-3" />
              </tr>
            </thead>
            <tbody className="divide-y divide-[var(--color-border)]">
              {filtered.length === 0 ? (
                <tr>
                  <td colSpan={8} className="text-center py-16 text-[var(--color-text-muted)]">
                    Aucun produit trouvé
                  </td>
                </tr>
              ) : (
                filtered.map((p) => (
                  <tr
                    key={p.id}
                    className={clsx(
                      "hover:bg-[var(--color-surface-dim)] group transition-colors",
                      !p.is_active && "opacity-50",
                    )}
                  >
                    <td className="px-4 py-3">
                      <p className="font-medium">{p.name_fr}</p>
                      {p.name_ar && (
                        <p className="text-xs text-[var(--color-text-muted)] text-right" dir="rtl">
                          {p.name_ar}
                        </p>
                      )}
                    </td>
                    <td className="px-3 py-3 font-mono text-xs">
                      {p.gtin ?? <span className="text-[var(--color-text-muted)]">—</span>}
                    </td>
                    <td className="px-3 py-3 text-[var(--color-text-muted)]">
                      {p.category_name_fr ?? "—"}
                    </td>
                    <td className="px-3 py-3 text-end font-semibold">
                      {p.sell_price.toFixed(2)}
                      <span className="text-xs font-normal ms-1 text-[var(--color-text-muted)]">DZD</span>
                    </td>
                    <td className="px-3 py-3 text-end text-[var(--color-text-muted)]">
                      {p.buy_price.toFixed(2)}
                    </td>
                    <td className="px-3 py-3 text-center font-mono">
                      {p.total_stock % 1 === 0
                        ? p.total_stock.toFixed(0)
                        : p.total_stock.toFixed(2)}
                      {p.unit_label_fr && (
                        <span className="ms-1 text-xs text-[var(--color-text-muted)]">
                          {p.unit_label_fr}
                        </span>
                      )}
                    </td>
                    <td className="px-3 py-3 text-center">{stockBadge(p)}</td>
                    <td className="px-3 py-3">
                      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity justify-end">
                        <button
                          className="btn-ghost text-xs py-1 px-2"
                          onClick={() => { setEditing(p); setModal("edit"); }}
                        >
                          ✏️ {t("products.edit")}
                        </button>
                        <button
                          className="btn-ghost text-xs py-1 px-2 text-[var(--color-danger-600)]"
                          onClick={() => setDeleting(p.id)}
                        >
                          🗑
                        </button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      )}

      {/* ── Stats bar ──────────────────────────────────────────────────────── */}
      <div className="text-xs text-[var(--color-text-muted)] flex gap-6">
        <span>{products.length} produits au total</span>
        <span className="text-[var(--color-warn-600)]">
          {products.filter((p) => p.total_stock > 0 && p.total_stock <= p.min_stock_alert).length} en alerte stock
        </span>
        <span className="text-[var(--color-danger-600)]">
          {products.filter((p) => p.total_stock <= 0 && p.is_active).length} en rupture
        </span>
      </div>

      {/* ── Modals ─────────────────────────────────────────────────────────── */}
      {modal && (
        <ProductFormModal
          product={editing}
          onSave={handleSave}
          onClose={() => { setModal(null); setEditing(null); }}
        />
      )}

      {deleting !== null && (
        <ConfirmDeleteModal
          onConfirm={() => handleDelete(deleting)}
          onCancel={() => setDeleting(null)}
        />
      )}
    </div>
  );
}

// ─── Confirm Delete Modal ──────────────────────────────────────────────────────

function ConfirmDeleteModal({
  onConfirm,
  onCancel,
}: {
  onConfirm: () => void;
  onCancel:  () => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="card w-full max-w-sm shadow-2xl">
        <h3 className="font-bold text-lg mb-2">Désactiver ce produit ?</h3>
        <p className="text-sm text-[var(--color-text-muted)] mb-5">
          Le produit sera masqué de la caisse mais conservé dans l'historique des ventes.
        </p>
        <div className="flex gap-3 justify-end">
          <button className="btn-ghost" onClick={onCancel}>Annuler</button>
          <button className="btn-danger" onClick={onConfirm}>Désactiver</button>
        </div>
      </div>
    </div>
  );
}