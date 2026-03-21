// src/lib/commands.ts
// Phase 4 additions:
//   - PrintTarget (serial | network) replaces the old `port: string` approach
//   - cmd_test_network_printer
//   - cmd_print_thermal_dain_statement
//   - cmd_export_dain_pdf / cmd_export_stock_pdf / cmd_export_sales_pdf
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

// ─── Phase 4 A4 PDF exports ───────────────────────────────────────────────────

export interface ExportPdfResult {
  path: string;
}

/** Generate + open an A4 Dain statement for the customer (Enterprise). */
export const exportDainPdf = (customerId: number): Promise<ExportPdfResult> =>
  invoke("cmd_export_dain_pdf", { customerId });

/** Generate + open an A4 stock inventory/alerts report (Enterprise). */
export const exportStockPdf = (warnOnly?: boolean): Promise<ExportPdfResult> =>
  invoke("cmd_export_stock_pdf", { warnOnly: warnOnly ?? false });

/** Generate + open an A4 sales report for a date range (Enterprise). */
export const exportSalesPdf = (dateFrom: string, dateTo: string): Promise<ExportPdfResult> =>
  invoke("cmd_export_sales_pdf", { dateFrom, dateTo });

// ─── Phase 4 Thermal Printing (serial + network) ─────────────────────────────

/** Serial/USB printer target. */
export interface PrintTargetSerial {
  transport: "Serial";
  port:      string;
  baud?:     number;
}

/** Network TCP printer target (IP + port 9100). */
export interface PrintTargetNetwork {
  transport: "Network";
  host:      string;
  port?:     number;
}

export type PrintTarget = PrintTargetSerial | PrintTargetNetwork;

export interface PrinterPort {
  port:           string;
  description:    string;
  likely_thermal: boolean;
  transport:      "serial" | "network";
}

export interface ReceiptData {
  shop_name_fr:    string;
  shop_name_ar:    string;
  shop_address:    string;
  shop_phone:      string;
  shop_nif:        string;
  shop_nis:        string;
  ref_number:      string;
  cashier:         string;
  date:            string;
  items:           { name_fr: string; name_ar: string; qty: number; unit_price: number; total_ttc: number; }[];
  total_ht:        number;
  total_ttc:       number;
  vat_amount:      number;
  vat_rate:        number;
  discount_amount: number;
  payment_method:  string;
  amount_paid:     number;
  change_given:    number;
  width_mm:        number;
  show_vat:        boolean;
}

export const listPrinters = (): Promise<PrinterPort[]> =>
  invoke("cmd_list_printers");

/** Ping a network printer to check connectivity. */
export const testNetworkPrinter = (host: string, port?: number): Promise<boolean> =>
  invoke("cmd_test_network_printer", { host, port: port ?? null });

export const printThermalReceipt = (data: ReceiptData, target: PrintTarget): Promise<void> =>
  invoke("cmd_print_thermal_receipt", { request: { data, target } });

/** Print a compact Dain ledger statement on the thermal printer. */
export const printThermalDainStatement = (customerId: number, target: PrintTarget): Promise<void> =>
  invoke("cmd_print_thermal_dain_statement", { request: { customerId, target } });

export const printTestPage = (target: PrintTarget): Promise<void> =>
  invoke("cmd_print_test_page", { request: { target } });

// ─── Settings ────────────────────────────────────────────────────────────────

export const getSettings    = ():                            Promise<AppSettings> =>
  invoke<AppSettings>("cmd_get_settings");

export const updateSettings = (updates: Partial<AppSettings>): Promise<void> =>
  invoke("cmd_update_settings", { updates });

// ─── Backup ──────────────────────────────────────────────────────────────────

export interface BackupResult { path: string; size_kb: number; created_at: string; }
export const createBackup    = (): Promise<BackupResult>         => invoke("cmd_create_backup");
export const listBackups     = (): Promise<BackupResult[]>       => invoke("cmd_list_backups");
export const exportSalesExcel = (dateFrom: string, dateTo: string): Promise<{ path: string; rows: number }> =>
  invoke("cmd_export_sales_excel", { request: { date_from: dateFrom, date_to: dateTo } });