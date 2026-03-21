// src/types/index.ts
// All types mirror the Rust command DTOs (serde-serialised).

// ─── License ────────────────────────────────────────────────────────────────

export interface LicenseState {
  is_valid:    boolean;
  tier:        "basic" | "professional" | "enterprise" | "";
  features:    number;   // bitmask
  expires_at:  string | null;
}

/** Feature bitmask constants — must match Rust `src/license/mod.rs` */
export const Features = {
  POS_BASIC:           1 << 0,
  INVENTORY_MGMT:      1 << 1,
  THERMAL_PRINT:       1 << 2,
  DAIN_LEDGER:         1 << 3,
  A4_REPORTS:          1 << 4,
  MULTI_CART:          1 << 5,
  ADVANCED_ANALYTICS:  1 << 6,
} as const;

export function hasFeature(license: LicenseState, flag: number): boolean {
  return license.is_valid && (license.features & flag) !== 0;
}

// ─── Products ────────────────────────────────────────────────────────────────

export interface Product {
  id:               number;
  gtin:             string | null;
  name_fr:          string;
  name_ar:          string;
  category_id:      number | null;
  category_name_fr: string | null;
  unit_id:          number | null;
  unit_label_fr:    string | null;
  sell_price:       number;
  buy_price:        number;
  vat_rate:         number;
  min_stock_alert:  number;
  is_active:        boolean;
  total_stock:      number;
  created_at:       string;
}

/**
 * Returned by `cmd_lookup_product` — includes the FEFO-selected batch.
 * This is what the POS uses when scanning a barcode.
 */
export interface ProductLookupResult {
  // Product fields
  id:            number;
  gtin:          string | null;
  name_fr:       string;
  name_ar:       string;
  sell_price:    number;
  vat_rate:      number;
  unit_label_fr: string | null;
  total_stock:   number;
  // FEFO batch (null if product has no batches with stock)
  batch_id:           number | null;
  batch_qty:          number | null;
  expiry_date:        string | null;
  days_until_expiry:  number | null;
}

export interface CreateProductInput {
  gtin:            string | null;
  name_fr:         string;
  name_ar:         string;
  category_id:     number | null;
  unit_id:         number | null;
  sell_price:      number;
  buy_price:       number;
  vat_rate:        number;
  min_stock_alert: number;
}

// ─── Scanner ──────────────────────────────────────────────────────────────────

export type ScanStatus = "idle" | "scanning" | "success" | "error";

export interface ScanEvent {
  barcode:   string;
  timestamp: number;
}

// ─── Inventory ───────────────────────────────────────────────────────────────

export interface InventoryBatch {
  id:                 number;
  product_id:         number;
  product_name:       string;
  quantity:           number;
  expiry_date:        string | null;
  supplier_ref:       string | null;
  cost_price:         number | null;
  received_at:        string;
  days_until_expiry:  number | null;
}

/** Returns the expiry status label for color-coding */
export type ExpiryStatus = "expired" | "critical" | "warning" | "ok" | "none";

export function getExpiryStatus(days: number | null): ExpiryStatus {
  if (days === null) return "none";
  if (days < 0)     return "expired";
  if (days <= 7)    return "critical";
  if (days <= 30)   return "warning";
  return "ok";
}

// ─── POS / Cart ──────────────────────────────────────────────────────────────

/**
 * A cart line. Uses ProductLookupResult so the FEFO batch is embedded.
 * `product` is kept as a minimal snapshot (id, names, prices) so we don't
 * store the full Product catalogue row in cart state.
 */
export interface CartItem {
  // Identity snapshot (won't change even if catalogue is edited mid-sale)
  product_id:   number;
  product_name: string;    // name_fr
  product_gtin: string | null;
  unit_label:   string | null;
  // Batch info (FEFO-selected)
  batch_id:          number | null;
  expiry_date:       string | null;
  days_until_expiry: number | null;
  // Sale line
  quantity:     number;
  unit_price:   number;   // may be overridden by cashier
  vat_rate:     number;
  discount_pct: number;
  line_total:   number;   // derived — always recalculated
}

export interface Cart {
  id:              string;
  label:           string;
  items:           CartItem[];
  customer_id:     number | null;
  discount_amount: number;
  payment_method:  PaymentMethod;
  amount_paid:     number;
}

export interface CartTotals {
  total_ht:   number;
  total_vat:  number;
  total_ttc:  number;
  item_count: number;
  change:     number;
}

export type PaymentMethod = "cash" | "cib" | "dahabia" | "dain";

// ─── Transactions ─────────────────────────────────────────────────────────────

export interface TransactionSummary {
  id:           number;
  ref_number:   string;
  total_ttc:    number;
  change_given: number;
}

export interface DailyReport {
  date:              string;
  total_sales:       number;
  total_ht:          number;
  transaction_count: number;
  cash_total:        number;
  cib_total:         number;
  dain_total:        number;
}

// ─── Dain ────────────────────────────────────────────────────────────────────

export interface CustomerDainSummary {
  customer_id: number;
  name:        string;
  phone:       string;
  balance:     number;
}

export interface DainEntry {
  id:         number;
  entry_type: "debt" | "repayment";
  amount:     number;
  notes:      string | null;
  created_at: string;
}

// ─── Settings ────────────────────────────────────────────────────────────────

export interface AppSettings {
  shop_name_fr:     string;
  shop_name_ar:     string;
  shop_address:     string;
  shop_phone:       string;
  shop_nif:         string;
  shop_nis:         string;
  shop_rc:          string;
  default_language: "fr" | "ar";
  currency:         string;
  thermal_width:    "58" | "80";
  vat_display:      "0" | "1";
  expiry_warn_days: string;
  [key: string]: string;
}

// ─── Suppliers ───────────────────────────────────────────────────────────────

export interface Supplier {
  id:                 number;
  code:               string | null;
  name:               string;
  name_ar:            string;
  contact_name:       string | null;
  phone:              string | null;
  email:              string | null;
  address:            string | null;
  wilaya:             string | null;
  nif:                string | null;
  nis:                string | null;
  rc:                 string | null;
  payment_terms_days: number;
  total_debt_dzd:     number;
  is_active:          boolean;
  created_at:         string;
}

export interface SupplierBalance {
  supplier_id:     number;
  name:            string;
  code:            string | null;
  total_purchases: number;
  total_paid:      number;
  balance:         number;
  order_count:     number;
  last_delivery:   string | null;
}

// ─── Purchase Orders ─────────────────────────────────────────────────────────

export interface PurchaseOrder {
  id:              number;
  ref_number:      string;
  supplier_id:     number;
  supplier_name:   string;
  status:          "draft" | "received" | "invoiced" | "paid" | "cancelled";
  invoice_number:  string | null;
  total_ht:        number;
  total_ttc:       number;
  discount_amount: number;
  amount_paid:     number;
  notes:           string | null;
  received_at:     string;
  item_count:      number;
}

// ─── Stock Adjustments ───────────────────────────────────────────────────────

export type AdjustmentType =
  | "correction" | "waste"    | "damage"     | "theft"
  | "return_cust"| "return_supp" | "opening" | "promo";

export interface StockAdjustmentRow {
  id:              number;
  batch_id:        number;
  product_name:    string;
  adjustment_type: AdjustmentType;
  quantity_delta:  number;
  quantity_before: number;
  quantity_after:  number;
  reason:          string | null;
  adjusted_by:     string;
  adjusted_at:     string;
}

// ─── Cashier Sessions ────────────────────────────────────────────────────────

export interface CashierSession {
  id:               number;
  cashier_name:     string;
  opening_float:    number;
  closing_declared: number | null;
  total_sales_ttc:  number;
  total_cash_sales: number;
  total_cib_sales:  number;
  total_dain_sales: number;
  expected_cash:    number;
  variance:         number | null;
  notes:            string | null;
  opened_at:        string;
  closed_at:        string | null;
  status:           "open" | "closed";
}

// ─── Tax Rates ───────────────────────────────────────────────────────────────

export interface TaxRate {
  id:          number;
  label:       string;
  rate:        number;
  description: string | null;
  is_active:   boolean;
}

// ─── V2 extended CartTotals ───────────────────────────────────────────────────

export interface CartTotals {
  total_ht:   number;
  total_vat:  number;
  total_ttc:  number;
  item_count: number;
  change:     number;
}