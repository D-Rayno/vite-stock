// src/lib/commands.ts
import { invoke } from "@tauri-apps/api/core";
import type {
  LicenseState, Product, ProductLookupResult, CreateProductInput,
  InventoryBatch, TransactionSummary, DailyReport,
  CustomerDainSummary, DainEntry, AppSettings,
} from "@/types";

// ─── License ─────────────────────────────────────────────────────────────────

export const verifyLicense     = (key: string):  Promise<LicenseState> =>
  invoke<LicenseState>("cmd_verify_license", { key });

export const getLicenseState   = ():             Promise<LicenseState> =>
  invoke<LicenseState>("cmd_get_license_state");

export const reloadLicense     = ():             Promise<LicenseState> =>
  invoke<LicenseState>("cmd_reload_license");

export const getHwid           = ():             Promise<string> =>
  invoke<string>("cmd_get_hwid");

export interface HwidComponents {
  hwid:        string;
  machine_uid: string;
  cpu_brand:   string;
  platform_id: string;
}
export const getHwidComponents = (): Promise<HwidComponents> =>
  invoke("cmd_get_hwid_components");

// ─── Products ────────────────────────────────────────────────────────────────

export const getProducts    = ():                       Promise<Product[]>              => invoke("cmd_get_products");
export const searchProducts = (query: string):          Promise<Product[]>              => invoke("cmd_search_products", { query });
export const lookupProduct  = (gtin: string):           Promise<ProductLookupResult | null> => invoke("cmd_lookup_product", { gtin });
export const createProduct  = (input: CreateProductInput): Promise<number>             => invoke("cmd_create_product", { input });
export const updateProduct  = (input: Product):         Promise<void>                  => invoke("cmd_update_product", { input });
export const deleteProduct  = (id: number):             Promise<void>                  => invoke("cmd_delete_product", { id });

// ─── Inventory ───────────────────────────────────────────────────────────────

export interface AddBatchInput {
  product_id: number; quantity: number;
  expiry_date: string | null; supplier_ref: string | null; cost_price: number | null;
}
export const getInventoryBatches = ():                        Promise<InventoryBatch[]> => invoke("cmd_get_inventory_batches");
export const addInventoryBatch   = (input: AddBatchInput):    Promise<number>           => invoke("cmd_add_inventory_batch", { input });
export const getExpiryAlerts     = (warnDays?: number):       Promise<InventoryBatch[]> => invoke("cmd_get_expiry_alerts", { warnDays: warnDays ?? null });

// ─── Transactions ─────────────────────────────────────────────────────────────

export interface TransactionItemInput {
  product_id: number; batch_id: number | null;
  quantity: number; unit_price: number; vat_rate: number; discount_pct: number;
}
export interface CreateTransactionInput {
  customer_id: number | null; items: TransactionItemInput[];
  discount_amount: number; payment_method: string;
  amount_paid: number; cashier_name: string; notes: string | null;
}
export const createTransaction = (input: CreateTransactionInput): Promise<TransactionSummary> => invoke("cmd_create_transaction", { input });
export const getTransaction    = (id: number):                    Promise<unknown>             => invoke("cmd_get_transaction", { id });
export const getDailyReport    = (date: string):                  Promise<DailyReport>         => invoke("cmd_get_daily_report", { date });

// ─── Dain ────────────────────────────────────────────────────────────────────

export const getCustomerDain = (phone: string):                                    Promise<CustomerDainSummary> => invoke("cmd_get_customer", { phone });
export const addDainEntry    = (customerId: number, transactionId: number | null, amount: number, notes: string | null): Promise<number> => invoke("cmd_add_dain_entry", { customerId, transactionId, amount, notes });
export const repayDain       = (customerId: number, amount: number, notes: string | null): Promise<number> => invoke("cmd_repay_dain", { customerId, amount, notes });
export const getDainHistory  = (customerId: number):                               Promise<DainEntry[]>          => invoke("cmd_get_dain_history", { customerId });

// ─── Printing ────────────────────────────────────────────────────────────────

export interface ReceiptData {
  shop_name: string; shop_address: string; shop_phone: string;
  ref_number: string; cashier: string; date: string;
  items: { name: string; qty: number; unit_price: number; total: number }[];
  total_ht: number; total_ttc: number; vat_amount: number;
  discount_amount: number; payment_method: string;
  amount_paid: number; change_given: number; width_mm: number;
}
export interface PrinterInfo { name: string; port: string; }
export const listPrinters        = ():                          Promise<PrinterInfo[]> => invoke("cmd_list_printers");
export const printThermalReceipt = (data: ReceiptData, port: string): Promise<void>   => invoke("cmd_print_thermal_receipt", { data, port });

// ─── Settings ────────────────────────────────────────────────────────────────
//
// NOTE on Rust serialisation:
//   `AppSettings(pub HashMap<String,String>)` is a serde newtype wrapper.
//   Serde serialises newtypes as the inner type, so the JSON is a plain
//   object: { "shop_name_fr": "...", "shop_address": "...", ... }.
//   There is NO outer { "0": {...} } wrapper — that was a previous bug.

export const getSettings    = ():                            Promise<AppSettings> =>
  invoke<AppSettings>("cmd_get_settings");

export const updateSettings = (updates: Partial<AppSettings>): Promise<void> =>
  invoke("cmd_update_settings", { updates });