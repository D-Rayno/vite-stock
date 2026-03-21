// src/store/cartStore.ts
// Multi-cart ("Attente") system using the new CartItem shape.
// CartItem stores a snapshot of product details at the time of scan so
// catalogue edits during a transaction don't corrupt the active sale.

import { create } from "zustand";
import type { Cart, CartItem, CartTotals, PaymentMethod, ProductLookupResult } from "@/types";

export type { CartTotals };

// ─── Helpers ─────────────────────────────────────────────────────────────────

function makeCart(label: string): Cart {
  return {
    id:              crypto.randomUUID(),
    label,
    items:           [],
    customer_id:     null,
    discount_amount: 0,
    payment_method:  "cash",
    amount_paid:     0,
  };
}

function calcLineTotal(
  qty:          number,
  unit_price:   number,
  vat_rate:     number,
  discount_pct: number,
): number {
  const base = qty * unit_price;
  const disc = base * (discount_pct / 100);
  return Math.round((base - disc) * (1 + vat_rate) * 100) / 100;
}

function recalc(item: CartItem): CartItem {
  return {
    ...item,
    line_total: calcLineTotal(item.quantity, item.unit_price, item.vat_rate, item.discount_pct),
  };
}

// ─── Totals ───────────────────────────────────────────────────────────────────

export function computeTotals(cart: Cart): CartTotals {
  const total_ht = cart.items.reduce((s, i) => {
    const base = i.quantity * i.unit_price;
    return s + base * (1 - i.discount_pct / 100);
  }, 0);

  const total_vat = cart.items.reduce((s, i) => {
    const base = i.quantity * i.unit_price * (1 - i.discount_pct / 100);
    return s + base * i.vat_rate;
  }, 0);

  const total_ttc = Math.max(0, Math.round((total_ht + total_vat - cart.discount_amount) * 100) / 100);

  return {
    total_ht:   Math.round(total_ht  * 100) / 100,
    total_vat:  Math.round(total_vat * 100) / 100,
    total_ttc,
    item_count: cart.items.reduce((s, i) => s + i.quantity, 0),
    change:     Math.max(0, Math.round((cart.amount_paid - total_ttc) * 100) / 100),
  };
}

// ─── Store ────────────────────────────────────────────────────────────────────

interface CartStore {
  carts:    Cart[];
  activeId: string;

  addCart:        () => string;
  removeCart:     (id: string) => void;
  setActive:      (id: string) => void;
  nextCart:       () => void;
  prevCart:       () => void;

  addFromLookup:   (result: ProductLookupResult) => void;
  removeItem:      (productId: number) => void;
  updateQty:       (productId: number, qty: number) => void;
  updatePrice:     (productId: number, price: number) => void;
  updateDiscount:  (productId: number, pct: number) => void;
  setCartDiscount: (amount: number) => void;
  setPayment:      (method: PaymentMethod, paid: number) => void;
  setCustomer:     (id: number | null) => void;
  clearActiveCart: () => void;

  activeCart: () => Cart;
  totals:     () => CartTotals;
}

const INITIAL = makeCart("Caisse 1");

export const useCartStore = create<CartStore>()((set, get) => ({
  carts:    [INITIAL],
  activeId: INITIAL.id,

  addCart: () => {
    const n     = get().carts.length + 1;
    const label = n === 1 ? "Caisse 1" : `Attente #${n}`;
    const cart  = makeCart(label);
    set((s) => ({ carts: [...s.carts, cart], activeId: cart.id }));
    return cart.id;
  },

  removeCart: (id) =>
    set((s) => {
      const rest = s.carts.filter((c) => c.id !== id);
      if (rest.length === 0) {
        const fresh = makeCart("Caisse 1");
        return { carts: [fresh], activeId: fresh.id };
      }
      return { carts: rest, activeId: s.activeId === id ? rest[0].id : s.activeId };
    }),

  setActive: (id) => set({ activeId: id }),

  nextCart: () => {
    const { carts, activeId } = get();
    const i = carts.findIndex((c) => c.id === activeId);
    set({ activeId: carts[(i + 1) % carts.length].id });
  },

  prevCart: () => {
    const { carts, activeId } = get();
    const i = carts.findIndex((c) => c.id === activeId);
    set({ activeId: carts[(i - 1 + carts.length) % carts.length].id });
  },

  addFromLookup: (result) =>
    set((s) => ({
      carts: s.carts.map((c) => {
        if (c.id !== s.activeId) return c;
        const existing = c.items.find((i) => i.product_id === result.id);
        if (existing) {
          return {
            ...c,
            items: c.items.map((i) =>
              i.product_id === result.id
                ? recalc({ ...i, quantity: i.quantity + 1 })
                : i,
            ),
          };
        }
        const newItem: CartItem = recalc({
          product_id:        result.id,
          product_name:      result.name_fr,
          product_gtin:      result.gtin,
          unit_label:        result.unit_label_fr,
          batch_id:          result.batch_id,
          expiry_date:       result.expiry_date,
          days_until_expiry: result.days_until_expiry,
          quantity:          1,
          unit_price:        result.sell_price,
          vat_rate:          result.vat_rate,
          discount_pct:      0,
          line_total:        0,
        });
        return { ...c, items: [...c.items, newItem] };
      }),
    })),

  removeItem: (productId) =>
    set((s) => ({
      carts: s.carts.map((c) =>
        c.id !== s.activeId ? c
          : { ...c, items: c.items.filter((i) => i.product_id !== productId) },
      ),
    })),

  updateQty: (productId, qty) => {
    if (qty <= 0) { get().removeItem(productId); return; }
    set((s) => ({
      carts: s.carts.map((c) =>
        c.id !== s.activeId ? c : {
          ...c,
          items: c.items.map((i) =>
            i.product_id === productId ? recalc({ ...i, quantity: qty }) : i,
          ),
        },
      ),
    }));
  },

  updatePrice: (productId, price) =>
    set((s) => ({
      carts: s.carts.map((c) =>
        c.id !== s.activeId ? c : {
          ...c,
          items: c.items.map((i) =>
            i.product_id === productId ? recalc({ ...i, unit_price: price }) : i,
          ),
        },
      ),
    })),

  updateDiscount: (productId, pct) =>
    set((s) => ({
      carts: s.carts.map((c) =>
        c.id !== s.activeId ? c : {
          ...c,
          items: c.items.map((i) =>
            i.product_id === productId ? recalc({ ...i, discount_pct: pct }) : i,
          ),
        },
      ),
    })),

  setCartDiscount: (amount) =>
    set((s) => ({
      carts: s.carts.map((c) =>
        c.id !== s.activeId ? c : { ...c, discount_amount: amount },
      ),
    })),

  setPayment: (method, paid) =>
    set((s) => ({
      carts: s.carts.map((c) =>
        c.id !== s.activeId ? c : { ...c, payment_method: method, amount_paid: paid },
      ),
    })),

  setCustomer: (id) =>
    set((s) => ({
      carts: s.carts.map((c) =>
        c.id !== s.activeId ? c : { ...c, customer_id: id },
      ),
    })),

  clearActiveCart: () =>
    set((s) => ({
      carts: s.carts.map((c) =>
        c.id !== s.activeId ? c
          : { ...c, items: [], discount_amount: 0, amount_paid: 0, customer_id: null },
      ),
    })),

  activeCart: () => {
    const { carts, activeId } = get();
    return carts.find((c) => c.id === activeId) ?? carts[0];
  },

  totals: () => computeTotals(get().activeCart()),
}));