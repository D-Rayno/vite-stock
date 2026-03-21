# SuperPOS — Phase 3: Database Schema Reference

**SQLite (rusqlite, bundled) · V1 + V2 migrations · WAL mode**

---

## Design principles

1. **Hard separation of static vs dynamic data.** `products` is the catalogue identity (GTIN, names, prices). `inventory_batches` holds mutable stock — quantity, expiry, cost — so a price change never corrupts historical batch data.
2. **Immutable sales ledger.** Transactions are never deleted. Voided sales set `is_voided = 1` and keep the full row so audit trails and session totals remain correct.
3. **No card data, ever.** `payment_method` is a `CHECK` enum (`cash | cib | dahabia | dain | cheque`). `sale_payments.terminal_ref` is capped at 20 chars by constraint — long enough for a TPE slip number, short enough to make it physically impossible to store a 16-digit PAN.
4. **Denormalised running totals where O(1) queries matter.** `suppliers.total_debt_dzd` and `cashier_sessions.total_*_sales` are maintained by triggers so balance queries never need a full-table sum.
5. **Algerian compliance baked in.** Three TVA rates (0%, 9%, 19%) live in `tax_rates` — a Finance Law change is a single `UPDATE`, not a code deploy. Price changes are logged automatically to `price_history` for DGI audit (3-year retention requirement). `stock_adjustments` covers the Code du Commerce Article 30 requirement (10-year books).

---

## Table catalogue

### Catalogue layer

#### `products`
Static catalogue identity. One row per SKU.

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `gtin` | TEXT UNIQUE | EAN-8, EAN-13, ITF-14 — null for PLU/weight items |
| `internal_code` | TEXT UNIQUE | Shop's own reference |
| `plu_code` | TEXT | 5-digit code for `WEIGHT` barcode type |
| `barcode_type` | TEXT | `CHECK IN ('EAN13','EAN8','ITF14','PLU','WEIGHT','INTERNAL')` |
| `is_variable_weight` | INTEGER | 0/1 — triggers scanner hook to extract PLU + weight from barcode |
| `name_fr` | TEXT NOT NULL | French name (POS display primary) |
| `name_ar` | TEXT | Arabic name (RTL display) |
| `category_id` | INTEGER FK → categories | |
| `unit_id` | INTEGER FK → units | |
| `tax_rate_id` | INTEGER FK → tax_rates | |
| `sell_price` | REAL `>= 0` | Current retail price |
| `buy_price` | REAL `>= 0` | Last purchase cost (updated on each delivery) |
| `price_sell_wholesale` | REAL | Optional B2B price |
| `vat_rate` | REAL | Redundant with tax_rate_id — kept for fast POS queries |
| `min_stock_alert` | INTEGER | Low-stock notification threshold |
| `max_stock_level` | INTEGER | Over-stock ceiling (optional) |
| `shelf_life_days` | INTEGER | Default expiry offset for batch entry form auto-fill |
| `brand` | TEXT | |
| `origin_country` | TEXT DEFAULT 'DZ' | |
| `is_active` | INTEGER | Soft-delete: 0 = hidden from POS, preserved in history |

**Indexes:** `gtin`, `category_id`, `is_active`

**Trigger:** `trg_products_updated` — auto-stamps `updated_at`  
**Trigger:** `trg_v2_log_price_change` — writes to `price_history` on `sell_price` / `buy_price` change

---

#### `categories`
| Column | Notes |
|---|---|
| `id` | |
| `name_fr`, `name_ar` | Seeded: Épicerie, Boulangerie, Boucherie, Laitiers, Boissons, Hygiène, Surgelés, Légumes & Fruits, Divers |

#### `units`
| Column | Notes |
|---|---|
| `id` | |
| `label_fr`, `label_ar` | Seeded: Pièce, Kg, Litre, Carton, Gr, Paquet |

#### `tax_rates`
| Column | Notes |
|---|---|
| `id` | |
| `label` UNIQUE | `'TVA 19%'`, `'TVA 9%'`, `'HT / 0%'` |
| `rate` REAL | `CHECK(rate >= 0 AND rate <= 1)` |
| `description` | |
| `is_active` | Allows future rate additions without data loss |

Seeded to the three standard Algerian TCA rates (Finance Law 2024).

---

### Inventory layer

#### `inventory_batches`
Dynamic stock. One row per physical delivery lot.

| Column | Notes |
|---|---|
| `id` | |
| `product_id` FK → products | CASCADE delete |
| `supplier_id` FK → suppliers | SET NULL |
| `purchase_order_id` FK → purchase_orders | SET NULL |
| `quantity` REAL `>= 0` | Current available units |
| `expiry_date` TEXT | ISO-8601 date — nullable for non-perishables |
| `cost_price` REAL | Purchase cost at time of receipt |
| `batch_number` TEXT | Supplier lot/batch reference (traceability / recall) |
| `purchase_date` TEXT | Date goods left supplier (may differ from `received_at`) |
| `location` TEXT | Shelf/aisle reference |
| `is_active` INTEGER | 0 = archived (zeroed / returned) |

**FEFO enforcement:** The `v_fefo_batches` view uses `ROW_NUMBER() OVER (PARTITION BY product_id ORDER BY expiry_date ASC NULLS LAST)`. The POS `cmd_lookup_product` command selects `fefo_rank = 1` to ensure the soonest-expiring batch is always sold first.

**Trigger:** `trg_v2_no_negative_stock` — `BEFORE UPDATE` aborts any decrement that would make `quantity < 0`  
**Trigger:** `trg_deduct_inventory` — auto-decrements `quantity` when a `transaction_items` row is inserted

---

#### `stock_adjustments`
Audit log for every inventory movement that is not a sale or purchase receipt.

| `adjustment_type` values | |
|---|---|
| `correction` | Physical count reconciliation |
| `waste` | Expired product discarded |
| `damage` | Breakage / spillage |
| `theft` | Shrinkage |
| `return_cust` | Customer return |
| `return_supp` | Returned to supplier |
| `opening` | Initial stock entry (before first PO) |
| `promo` | Free samples / promotional giveaways |

`quantity_delta` is **signed** — positive = stock added, negative = stock removed.  
`quantity_before` and `quantity_after` are snapshotted at insert time.

**Trigger:** `trg_v2_apply_adjustment` — updates `inventory_batches.quantity = quantity_after`

---

### Supplier & purchasing layer

#### `suppliers`
| Column | Notes |
|---|---|
| `code` UNIQUE | Internal ref, e.g. `SUP-001` |
| `wilaya` | Algerian province |
| `nif`, `nis`, `rc` | Algerian fiscal identifiers |
| `payment_terms_days` | Default AP payment window |
| `total_debt_dzd` REAL `>= 0` | **Denormalised** running AP balance — maintained by triggers |

**Trigger:** `trg_v2_po_received_debt` — adds `purchase_orders.total_ttc` to `total_debt_dzd` when a PO transitions `received → invoiced`  
**Trigger:** `trg_v2_supplier_payment_debit` — subtracts from `total_debt_dzd` on each `supplier_payments` insert

#### `purchase_orders`
Status lifecycle: `draft → received → invoiced → paid`

`ref_number` format: `BL-YYYYMMDD-NNNN` (generated in Rust, daily sequence)

**Trigger:** `trg_v2_update_po_totals` — recalculates `total_ht` / `total_ttc` after each `purchase_items` insert

#### `purchase_items`
A single item line on a delivery note.

`cost_price_ttc` is a **generated (virtual) column**: `cost_price_ht * (1 + tax_rate.rate)` — always correct, never stored.

**Trigger:** `trg_v2_create_batch_on_purchase` — auto-creates an `inventory_batches` row and back-fills `purchase_items.batch_id` when `quantity_received > 0`. This is the single entry point for all inbound stock.

#### `supplier_payments`
Records money paid **to** suppliers. `payment_method CHECK IN ('cash','cheque','virement','autre')`.  
`cheque_number` is only populated when `payment_method = 'cheque'`. No card fields.

---

### Sales layer

#### `transactions`
Immutable sales ledger head record.

| Column | Notes |
|---|---|
| `ref_number` UNIQUE | `TXN-YYYYMMDD-NNNN` |
| `payment_method` | `CHECK IN ('cash','cib','dahabia','dain','cheque')` |
| `payment_status` | `CHECK IN ('paid','partial','credit')` |
| `is_voided` INTEGER | 0/1 — never deleted, only voided |
| `session_id` FK → cashier_sessions | SET NULL on close |

**Trigger:** `trg_v2_auto_dain_on_sale` — when `payment_method = 'dain'` and `customer_id IS NOT NULL`, automatically inserts a `dain_entries` debt row  
**Trigger:** `trg_v2_update_session_on_sale` — accumulates session totals (total TTC, by payment method) on each committed sale  
**Trigger:** `trg_v2_reverse_session_on_void` — reverses session totals when `is_voided` flips to 1

#### `transaction_items`
Line items. Immutable snapshot semantics:

- `product_name_fr`, `gtin_snapshot` are snapshotted at insert time by `trg_v2_snapshot_item`. If a product is renamed or its barcode corrected later, historical receipts remain correct.
- `weight_grams` — populated for `WEIGHT` barcode type (electronic scale reads weight from barcode).
- `batch_id` — links to the FEFO-selected batch. The `trg_deduct_inventory` trigger uses this to decrement exactly the right batch.

#### `sale_payments`
Supports split payment (e.g. 2 000 DZD cash + 1 000 DZD CIB for a 3 000 DZD sale).

`terminal_ref TEXT CHECK(length(terminal_ref) <= 20)` — the TPE slip number. The 20-char limit is a compliance guardrail against accidental card data storage.

---

### Customer credit (Dain) layer

#### `customers`
| Column | Notes |
|---|---|
| `phone` TEXT UNIQUE | Primary lookup key at POS |
| `credit_limit_dzd` | 0 = no limit enforced |
| `wilaya` | Province |

#### `dain_entries`
Double-entry ledger. `entry_type CHECK IN ('debt','repayment')`.

`balance_after` — running balance snapshot (O(1) per row vs O(n) `SUM` per balance query). Populated by the application layer on insert.

---

### Session layer

#### `cashier_sessions`
Till open/close per shift.

Virtual (generated) columns — never stored, always correct:
- `expected_cash = opening_float + total_cash_sales`
- `variance = closing_declared - expected_cash` (null until session is closed)

Status: `CHECK IN ('open','closed')`. Only one `open` session per cashier is enforced in the Rust command layer (`cmd_open_session`).

---

### Audit layer

#### `price_history`
Append-only log. Populated automatically by `trg_v2_log_price_change` — the application never writes to this table directly. Required for DGI Algeria 3-year fiscal audit.

#### `settings`
Flat key-value store. Keys seeded at V1 migration:

| Key | Default | Notes |
|---|---|---|
| `shop_name_fr` | `'Mon Supermarché'` | |
| `shop_name_ar` | `'سوبرماركت'` | |
| `shop_address` | `'Alger, Algérie'` | |
| `shop_phone`, `shop_nif`, `shop_nis`, `shop_rc` | `''` | Printed on receipts |
| `default_language` | `'fr'` | `fr` or `ar` |
| `currency` | `'DZD'` | Display only |
| `thermal_width` | `'80'` | `58` or `80` (mm) |
| `vat_display` | `'1'` | Show VAT breakdown on ticket |
| `expiry_warn_days` | `'30'` | `v_expiry_alerts` view threshold |
| `printer_port`, `printer_baud` | `''`, `'9600'` | V2 additions |
| `backup_auto`, `backup_interval_h` | `'1'`, `'24'` | V2 additions |

---

## Views

| View | Purpose |
|---|---|
| `v_product_stock` | Live stock per product with FEFO-nearest expiry, stock status (`ok/low_stock/out_of_stock`), joins to category, unit, tax rate |
| `v_customer_balance` | Net Dain balance per customer (total_debt − total_repaid) |
| `v_supplier_balance` | AP balance per supplier (total_purchases − total_paid) |
| `v_daily_summary` | Pre-aggregated daily sales totals, filtered `is_voided = 0` |
| `v_fefo_batches` | FEFO rank per product using `ROW_NUMBER() OVER (PARTITION BY product_id ORDER BY expiry_date ASC NULLS LAST)` |
| `v_expiry_alerts` | Batches expiring within `settings.expiry_warn_days` — joined to supplier name |

---

## Trigger map

| Trigger | Table | Event | Action |
|---|---|---|---|
| `trg_products_updated` | products | AFTER UPDATE | Stamp `updated_at` |
| `trg_v2_log_price_change` | products | AFTER UPDATE OF sell_price, buy_price | Insert into `price_history` |
| `trg_deduct_inventory` | transaction_items | AFTER INSERT | Decrement `inventory_batches.quantity` |
| `trg_v2_snapshot_item` | transaction_items | AFTER INSERT | Backfill `product_name_fr`, `gtin_snapshot` |
| `trg_v2_no_negative_stock` | inventory_batches | BEFORE UPDATE OF quantity | `RAISE(ABORT)` if quantity < 0 |
| `trg_v2_create_batch_on_purchase` | purchase_items | AFTER INSERT | Create `inventory_batches` row, backfill `batch_id` |
| `trg_v2_update_po_totals` | purchase_items | AFTER INSERT | Recalculate `purchase_orders.total_ht/ttc` |
| `trg_v2_auto_dain_on_sale` | transactions | AFTER INSERT | Create `dain_entries` debt when `payment_method = 'dain'` |
| `trg_v2_update_session_on_sale` | transactions | AFTER INSERT | Accumulate session cash/CIB/Dain totals |
| `trg_v2_reverse_session_on_void` | transactions | AFTER UPDATE OF is_voided | Reverse session totals |
| `trg_v2_po_received_debt` | purchase_orders | AFTER UPDATE OF status | Add to `suppliers.total_debt_dzd` on `invoiced` |
| `trg_v2_supplier_payment_debit` | supplier_payments | AFTER INSERT | Subtract from `suppliers.total_debt_dzd` |
| `trg_v2_apply_adjustment` | stock_adjustments | AFTER INSERT | Set `inventory_batches.quantity = quantity_after` |
| `trg_v2_suppliers_updated` | suppliers | AFTER UPDATE | Stamp `updated_at` |
| `trg_v2_customers_updated` | customers | AFTER UPDATE | Stamp `updated_at` |

---

## PRAGMA configuration

Applied on every connection open (`configure_pragmas`):

```sql
PRAGMA journal_mode = WAL;          -- concurrent reads during report queries
PRAGMA foreign_keys = ON;           -- all FK constraints enforced
PRAGMA synchronous   = NORMAL;      -- safe + fast (not paranoid-safe)
PRAGMA temp_store    = MEMORY;      -- temp tables in RAM
PRAGMA cache_size    = -16000;      -- 16 MB page cache
PRAGMA mmap_size     = 268435456;   -- 256 MB memory-mapped I/O
PRAGMA auto_vacuum   = INCREMENTAL; -- reclaim space without full VACUUM
PRAGMA busy_timeout  = 5000;        -- 5s retry on locked database
```

---

## Migration strategy

Versions are tracked via `PRAGMA user_version`. The runner in `db/mod.rs` applies each migration function exactly once, in order:

| Version | File | Coverage |
|---|---|---|
| V1 | `db/mod.rs::migrate_v1` | Baseline: products, inventory_batches, customers, transactions/items, dain_entries, settings, core triggers |
| V2 | `db/schema_v2.rs::migrate_v2` | Suppliers, purchase orders, stock adjustments, cashier sessions, sale payments, price history, tax rates, 6 views, full trigger suite |

All V2 `ALTER TABLE` statements use `safe_alters()` which silently skips `duplicate column name` errors — migrations are idempotent and safe to replay on an existing database.

---

## Strict prohibitions (enforced by schema)

- No column named `card_number`, `pan`, `cvv`, `expiry_month`, `expiry_year`, or `cardholder_name` exists anywhere.
- `sale_payments.terminal_ref` max 20 chars (`CHECK(length(...) <= 20)`).
- `transactions.payment_method CHECK IN ('cash','cib','dahabia','dain','cheque')` — no free-text payment type.
- `inventory_batches.quantity CHECK(quantity >= 0)` + `trg_v2_no_negative_stock` — double guard against negative stock.
- Voided transactions are never deleted — `is_voided = 1` is the only path.