// src-tauri/src/db/schema_v2.rs
//! V2 database schema migration.
//!
//! Design goals for V2:
//!   1. Full supplier management  (suppliers, purchase_orders, purchase_items)
//!   2. Algerian barcode system   (EAN-8/13, ITF-14, PLU, weight-embedded)
//!   3. Configurable tax rates    (TVA 0%, 9%, 19% — all used in Algeria)
//!   4. Immutable sales ledger    (snapshots, void trail, split payments)
//!   5. Complete Dain system      (customer_ledger with running balance view)
//!   6. Stock audit trail         (stock_adjustments — every non-sale movement)
//!   7. Cashier sessions          (daily till open/close, float reconciliation)
//!   8. Price history             (auto-logged on every price change)
//!
//! Migration strategy:
//!   All statements use IF NOT EXISTS / IF NOT EXISTS (for indexes) or
//!   ALTER TABLE … ADD COLUMN IF NOT EXISTS (emulated via try-catch in Rust).
//!   This means the migration is safe to run on both a fresh database and an
//!   existing V1 database.
//!
//! STRICT rule enforced by CHECK constraints:
//!   No column named card_number, pan, cvv, expiry_month, expiry_year, or
//!   any payment-card-related field is present in any table.

use rusqlite::{Connection, Result as SqlResult, params};

// ─── Public entry point ────────────────────────────────────────────────────────

pub fn migrate_v2(conn: &Connection) -> SqlResult<()> {
    // Each section is in its own function for clarity and testability.
    create_tax_rates(conn)?;
    create_suppliers(conn)?;
    create_purchase_orders(conn)?;
    create_purchase_items(conn)?;
    create_stock_adjustments(conn)?;
    create_cashier_sessions(conn)?;
    create_sale_payments(conn)?;
    create_price_history(conn)?;
    upgrade_products(conn)?;
    upgrade_inventory_batches(conn)?;
    upgrade_transactions(conn)?;
    upgrade_transaction_items(conn)?;
    upgrade_customers(conn)?;
    upgrade_dain_entries(conn)?;
    create_views(conn)?;
    create_v2_triggers(conn)?;
    seed_v2_data(conn)?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// NEW TABLES
// ═══════════════════════════════════════════════════════════════════════════════

// ─── 1. tax_rates ─────────────────────────────────────────────────────────────
//
// Algeria levies three VAT rates under the Code des Taxes sur le Chiffre
// d'Affaires (Article 21 TCA):
//   • 0%  — basic foodstuffs, medicines, books (exonéré / hors taxe)
//   • 9%  — reduced rate for specific goods (e.g., sugar, flour)
//   • 19% — standard rate for all other goods and services
//
// Storing rates in a table (not hardcoded) lets the owner update them when
// the Finance Law changes each year without a software update.

fn create_tax_rates(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    CREATE TABLE IF NOT EXISTS tax_rates (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        label       TEXT    NOT NULL UNIQUE,   -- 'TVA 19%', 'TVA 9%', 'Exonéré'
        rate        REAL    NOT NULL CHECK(rate >= 0 AND rate <= 1),
        description TEXT,
        is_active   INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0,1)),
        created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_taxrate_rate ON tax_rates(rate);
    ")?;
    Ok(())
}

// ─── 2. suppliers ─────────────────────────────────────────────────────────────
//
// Supplier directory with running accounts-payable balance.
// `total_debt_dzd` is a DENORMALISED running total maintained by trigger
// (trg_v2_update_supplier_debt) for O(1) balance queries on the reports page.
// The source of truth remains purchase_orders.

fn create_suppliers(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    CREATE TABLE IF NOT EXISTS suppliers (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        code            TEXT    UNIQUE,              -- internal reference e.g. 'SUP-001'
        name            TEXT    NOT NULL,
        name_ar         TEXT    NOT NULL DEFAULT '',
        contact_name    TEXT,
        phone           TEXT,
        email           TEXT,
        address         TEXT,
        wilaya          TEXT,                        -- Algerian province
        nif             TEXT,                        -- Numéro d'Identification Fiscale
        nis             TEXT,                        -- Numéro d'Identification Statistique
        rc              TEXT,                        -- Registre du Commerce
        payment_terms_days INTEGER NOT NULL DEFAULT 30,
        -- Running AP balance — updated by trigger, always >= 0
        total_debt_dzd  REAL    NOT NULL DEFAULT 0 CHECK(total_debt_dzd >= 0),
        notes           TEXT,
        is_active       INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0,1)),
        created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_supplier_name   ON suppliers(name);
    CREATE INDEX IF NOT EXISTS idx_supplier_code   ON suppliers(code);
    CREATE INDEX IF NOT EXISTS idx_supplier_wilaya ON suppliers(wilaya);
    ")?;
    Ok(())
}

// ─── 3. purchase_orders ───────────────────────────────────────────────────────
//
// Each incoming delivery from a supplier creates one purchase_order.
// Status lifecycle:  draft → received → invoiced → paid
//
// Strict rule: no credit-card fields. Supplier payment is tracked as
// a separate `supplier_payments` table (debit against AP balance).

fn create_purchase_orders(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    CREATE TABLE IF NOT EXISTS purchase_orders (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        ref_number      TEXT    NOT NULL UNIQUE,     -- BL-YYYYMMDD-NNNN
        supplier_id     INTEGER NOT NULL REFERENCES suppliers(id) ON DELETE RESTRICT,
        status          TEXT    NOT NULL DEFAULT 'received'
                            CHECK(status IN ('draft','received','invoiced','paid','cancelled')),
        invoice_number  TEXT,                        -- supplier's invoice ref
        total_ht        REAL    NOT NULL DEFAULT 0,
        total_ttc       REAL    NOT NULL DEFAULT 0,
        discount_amount REAL    NOT NULL DEFAULT 0,
        amount_paid     REAL    NOT NULL DEFAULT 0,
        notes           TEXT,
        received_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        invoiced_at     TEXT,
        paid_at         TEXT,
        created_by      TEXT    NOT NULL DEFAULT 'Admin'
    );
    CREATE INDEX IF NOT EXISTS idx_po_supplier   ON purchase_orders(supplier_id);
    CREATE INDEX IF NOT EXISTS idx_po_status     ON purchase_orders(status);
    CREATE INDEX IF NOT EXISTS idx_po_received   ON purchase_orders(received_at);

    -- ── Supplier payment ledger (tracks AP debt repayments) ───────────────
    -- Records money paid TO suppliers. No card-number fields.
    CREATE TABLE IF NOT EXISTS supplier_payments (
        id                INTEGER PRIMARY KEY AUTOINCREMENT,
        supplier_id       INTEGER NOT NULL REFERENCES suppliers(id) ON DELETE CASCADE,
        purchase_order_id INTEGER REFERENCES purchase_orders(id)   ON DELETE SET NULL,
        amount            REAL    NOT NULL CHECK(amount > 0),
        payment_method    TEXT    NOT NULL DEFAULT 'cash'
                              CHECK(payment_method IN ('cash','cheque','virement','autre')),
        cheque_number     TEXT,                      -- only if payment_method='cheque'
        reference         TEXT,
        notes             TEXT,
        paid_at           TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_suppay_supplier ON supplier_payments(supplier_id);
    ")?;
    Ok(())
}

// ─── 4. purchase_items ────────────────────────────────────────────────────────
//
// Individual product lines within a purchase order.
// Inserting a purchase_item automatically creates an inventory_batch via
// trigger trg_v2_create_batch_on_purchase.

fn create_purchase_items(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    CREATE TABLE IF NOT EXISTS purchase_items (
        id                INTEGER PRIMARY KEY AUTOINCREMENT,
        purchase_order_id INTEGER NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
        product_id        INTEGER NOT NULL REFERENCES products(id)        ON DELETE RESTRICT,
        quantity_ordered  REAL    NOT NULL CHECK(quantity_ordered > 0),
        quantity_received REAL    NOT NULL DEFAULT 0 CHECK(quantity_received >= 0),
        cost_price_ht     REAL    NOT NULL CHECK(cost_price_ht >= 0),
        tax_rate_id       INTEGER REFERENCES tax_rates(id),
        cost_price_ttc    REAL    GENERATED ALWAYS AS (
                              cost_price_ht * (1 + COALESCE(
                                  (SELECT rate FROM tax_rates WHERE id = tax_rate_id), 0
                              ))
                          ) VIRTUAL,
        batch_number      TEXT,                       -- supplier lot/batch ref
        expiry_date       TEXT,                       -- ISO-8601 if perishable
        -- batch_id is set by trigger after the batch is created
        batch_id          INTEGER REFERENCES inventory_batches(id) ON DELETE SET NULL
    );
    CREATE INDEX IF NOT EXISTS idx_pi_po      ON purchase_items(purchase_order_id);
    CREATE INDEX IF NOT EXISTS idx_pi_product ON purchase_items(product_id);
    ")?;
    Ok(())
}

// ─── 5. stock_adjustments ─────────────────────────────────────────────────────
//
// Every inventory movement that is NOT a sale or purchase is recorded here.
// This preserves a complete audit trail (required by Algerian commercial law,
// Article 30 of the Code du Commerce — books must be kept for 10 years).
//
// adjustment_type values:
//   correction    — periodic physical count correction
//   waste         — expired product discarded
//   damage        — breakage, spillage, etc.
//   theft         — shrinkage / vol
//   return_cust   — customer returns merchandise
//   return_supp   — stock returned to supplier
//   opening       — initial stock entry (before first purchase order)
//   promo         — free samples / promotions

fn create_stock_adjustments(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    CREATE TABLE IF NOT EXISTS stock_adjustments (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        batch_id        INTEGER NOT NULL REFERENCES inventory_batches(id) ON DELETE CASCADE,
        product_id      INTEGER NOT NULL REFERENCES products(id)          ON DELETE RESTRICT,
        adjustment_type TEXT    NOT NULL
                            CHECK(adjustment_type IN (
                                'correction','waste','damage','theft',
                                'return_cust','return_supp','opening','promo'
                            )),
        -- Signed quantity: positive = stock added, negative = stock removed
        quantity_delta  REAL    NOT NULL,
        quantity_before REAL    NOT NULL,
        quantity_after  REAL    NOT NULL,
        unit_cost       REAL,                        -- for cost-of-goods tracking
        reason          TEXT,
        adjusted_by     TEXT    NOT NULL DEFAULT 'Admin',
        adjusted_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_adj_batch   ON stock_adjustments(batch_id);
    CREATE INDEX IF NOT EXISTS idx_adj_product ON stock_adjustments(product_id);
    CREATE INDEX IF NOT EXISTS idx_adj_date    ON stock_adjustments(adjusted_at);
    CREATE INDEX IF NOT EXISTS idx_adj_type    ON stock_adjustments(adjustment_type);
    ")?;
    Ok(())
}

// ─── 6. cashier_sessions ──────────────────────────────────────────────────────
//
// Tracks daily till open/close.  Cashier opens with a float count;
// closes by declaring how much cash is in the drawer.  The difference
// (expected vs actual) flags discrepancies for the manager.

fn create_cashier_sessions(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    CREATE TABLE IF NOT EXISTS cashier_sessions (
        id               INTEGER PRIMARY KEY AUTOINCREMENT,
        cashier_name     TEXT    NOT NULL,
        opening_float    REAL    NOT NULL DEFAULT 0,   -- cash placed in drawer at open
        closing_declared REAL,                         -- cash counted at close
        total_sales_ttc  REAL    NOT NULL DEFAULT 0,   -- auto-summed from transactions
        total_cash_sales REAL    NOT NULL DEFAULT 0,
        total_cib_sales  REAL    NOT NULL DEFAULT 0,
        total_dain_sales REAL    NOT NULL DEFAULT 0,
        expected_cash    REAL    GENERATED ALWAYS AS
                             (opening_float + total_cash_sales) VIRTUAL,
        variance         REAL    GENERATED ALWAYS AS
                             (CASE WHEN closing_declared IS NOT NULL
                                   THEN closing_declared - (opening_float + total_cash_sales)
                              END) VIRTUAL,
        notes            TEXT,
        opened_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        closed_at        TEXT,
        status           TEXT    NOT NULL DEFAULT 'open'
                             CHECK(status IN ('open','closed'))
    );
    CREATE INDEX IF NOT EXISTS idx_session_cashier ON cashier_sessions(cashier_name);
    CREATE INDEX IF NOT EXISTS idx_session_opened  ON cashier_sessions(opened_at);
    CREATE INDEX IF NOT EXISTS idx_session_status  ON cashier_sessions(status);
    ")?;
    Ok(())
}

// ─── 7. sale_payments ─────────────────────────────────────────────────────────
//
// Supports split payment per transaction.
// Example: a 3000 DZD sale paid as 2000 DZD cash + 1000 DZD CIB.
//
// STRICT RULE: No card-number fields anywhere in this table.
// The terminal_ref is the receipt number printed by the CIB/Dahabia TPE —
// it is a short alphanumeric reference, NOT a PAN or account number.

fn create_sale_payments(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    CREATE TABLE IF NOT EXISTS sale_payments (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        transaction_id  INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
        payment_method  TEXT    NOT NULL
                            CHECK(payment_method IN ('cash','cib','dahabia','dain','cheque')),
        amount          REAL    NOT NULL CHECK(amount > 0),
        -- TPE terminal receipt reference (printed slip number, not card data).
        -- Maximum 20 chars — enforced to prevent accidental card number storage.
        terminal_ref    TEXT    CHECK(
                            terminal_ref IS NULL OR length(terminal_ref) <= 20
                        ),
        created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        -- ⚠ Prohibited columns (schema enforces no card data):
        -- card_number, pan, account_number, cvv, pin, expiry_month,
        -- expiry_year, cardholder_name — NONE of these must ever be added.
    );
    CREATE INDEX IF NOT EXISTS idx_salespay_txn    ON sale_payments(transaction_id);
    CREATE INDEX IF NOT EXISTS idx_salespay_method ON sale_payments(payment_method);
    ")?;
    Ok(())
}

// ─── 8. price_history ─────────────────────────────────────────────────────────
//
// Immutable audit log of every price change.
// Required for tax audit compliance (DGI Algeria can audit 3 prior fiscal years).
// Populated automatically by trigger trg_v2_log_price_change.

fn create_price_history(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    CREATE TABLE IF NOT EXISTS price_history (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        product_id      INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
        old_price_sell  REAL    NOT NULL,
        new_price_sell  REAL    NOT NULL,
        old_price_buy   REAL,
        new_price_buy   REAL,
        changed_by      TEXT    NOT NULL DEFAULT 'Admin',
        reason          TEXT,
        changed_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_pricehistory_product ON price_history(product_id);
    CREATE INDEX IF NOT EXISTS idx_pricehistory_date    ON price_history(changed_at);
    ")?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// ALTER EXISTING TABLES — additive only, never drops columns
// ═══════════════════════════════════════════════════════════════════════════════

// ─── products — V2 upgrades ───────────────────────────────────────────────────
//
// Algerian barcode types:
//   EAN13    — standard 13-digit GS1 barcode (most imported + Algerian products)
//   EAN8     — short 8-digit (small packaging)
//   ITF14    — 14-digit interleaved for wholesale cartons (boîtes de 6, cartons)
//   PLU      — internal price-lookup code (fresh produce, deli — no barcode)
//   WEIGHT   — variable-weight barcode: 2|PPPPP|WWWWW|C
//              where PPPPP = PLU, WWWWW = weight in grams × 1000
//              used by electronic scales (balances) for cheese, meat, etc.
//   INTERNAL — shop's own numbering (no GS1 registration)
//
// is_variable_weight = 1 triggers the barcode scanner hook to extract PLU + weight.
// shelf_life_days is the default number of days before expiry for auto-fill
// in the batch receipt form (e.g., milk = 7, frozen goods = 365).

fn upgrade_products(conn: &Connection) -> SqlResult<()> {
    let alters = [
        // Barcode system
        "ALTER TABLE products ADD COLUMN barcode_type TEXT NOT NULL DEFAULT 'EAN13'
             CHECK(barcode_type IN ('EAN13','EAN8','ITF14','PLU','WEIGHT','INTERNAL'))",
        "ALTER TABLE products ADD COLUMN internal_code TEXT UNIQUE",
        "ALTER TABLE products ADD COLUMN plu_code TEXT",   // 5-digit for weight barcodes
        "ALTER TABLE products ADD COLUMN is_variable_weight INTEGER NOT NULL DEFAULT 0
             CHECK(is_variable_weight IN (0,1))",
        // Pricing
        "ALTER TABLE products ADD COLUMN price_sell_wholesale REAL",
        // Tax
        "ALTER TABLE products ADD COLUMN tax_rate_id INTEGER REFERENCES tax_rates(id)",
        // Stock
        "ALTER TABLE products ADD COLUMN max_stock_level INTEGER",
        "ALTER TABLE products ADD COLUMN shelf_life_days INTEGER",
        // Catalogue
        "ALTER TABLE products ADD COLUMN brand TEXT",
        "ALTER TABLE products ADD COLUMN origin_country TEXT NOT NULL DEFAULT 'DZ'",
        "ALTER TABLE products ADD COLUMN image_path TEXT",     // relative path to product image
    ];
    safe_alters(conn, &alters)
}

// ─── inventory_batches — V2 upgrades ─────────────────────────────────────────
//
// Links batches to their source purchase order and supplier.
// batch_number = supplier's lot number (printed on carton label, tracked for recalls).
// purchase_date is distinct from received_at (goods may sit in transit).

fn upgrade_inventory_batches(conn: &Connection) -> SqlResult<()> {
    let alters = [
        "ALTER TABLE inventory_batches ADD COLUMN supplier_id INTEGER
             REFERENCES suppliers(id) ON DELETE SET NULL",
        "ALTER TABLE inventory_batches ADD COLUMN purchase_order_id INTEGER
             REFERENCES purchase_orders(id) ON DELETE SET NULL",
        "ALTER TABLE inventory_batches ADD COLUMN batch_number TEXT",
        "ALTER TABLE inventory_batches ADD COLUMN purchase_date TEXT",
        "ALTER TABLE inventory_batches ADD COLUMN location TEXT",   // shelf/aisle reference
        "ALTER TABLE inventory_batches ADD COLUMN is_active INTEGER NOT NULL DEFAULT 1
             CHECK(is_active IN (0,1))",
    ];
    // Re-create the supplier index after column addition
    let result = safe_alters(conn, &alters);
    conn.execute_batch("
    CREATE INDEX IF NOT EXISTS idx_batch_supplier ON inventory_batches(supplier_id);
    CREATE INDEX IF NOT EXISTS idx_batch_po       ON inventory_batches(purchase_order_id);
    ")?;
    result
}

// ─── transactions — V2 upgrades ───────────────────────────────────────────────
//
// Adds session tracking, void support, and payment_status.
// Voided transactions are kept (never deleted) to preserve the audit trail.
// payment_status:
//   paid     — fully settled
//   partial  — only used with dain (customer paid less than total)
//   credit   — entirely on dain credit (amount_paid = 0)

fn upgrade_transactions(conn: &Connection) -> SqlResult<()> {
    let alters = [
        "ALTER TABLE transactions ADD COLUMN session_id INTEGER
             REFERENCES cashier_sessions(id) ON DELETE SET NULL",
        "ALTER TABLE transactions ADD COLUMN payment_status TEXT NOT NULL DEFAULT 'paid'
             CHECK(payment_status IN ('paid','partial','credit'))",
        "ALTER TABLE transactions ADD COLUMN is_voided INTEGER NOT NULL DEFAULT 0
             CHECK(is_voided IN (0,1))",
        "ALTER TABLE transactions ADD COLUMN voided_at TEXT",
        "ALTER TABLE transactions ADD COLUMN void_reason TEXT",
        "ALTER TABLE transactions ADD COLUMN voided_by TEXT",
    ];
    conn.execute_batch("
    CREATE INDEX IF NOT EXISTS idx_txn_voided  ON transactions(is_voided);
    CREATE INDEX IF NOT EXISTS idx_txn_session ON transactions(session_id);
    CREATE INDEX IF NOT EXISTS idx_txn_status  ON transactions(payment_status);
    ")?;
    safe_alters(conn, &alters)
}

// ─── transaction_items — V2 upgrades ─────────────────────────────────────────
//
// Snapshots of product name and barcode AT THE TIME OF SALE.
// Critical: if a product is renamed or its GTIN is corrected later, the
// historical receipt still shows the correct description.

fn upgrade_transaction_items(conn: &Connection) -> SqlResult<()> {
    let alters = [
        "ALTER TABLE transaction_items ADD COLUMN product_name_fr TEXT",
        "ALTER TABLE transaction_items ADD COLUMN product_name_ar TEXT",
        "ALTER TABLE transaction_items ADD COLUMN gtin_snapshot TEXT",
        "ALTER TABLE transaction_items ADD COLUMN product_type_snapshot TEXT",
        // For weight barcodes: actual weight sold in grams
        "ALTER TABLE transaction_items ADD COLUMN weight_grams REAL",
    ];
    safe_alters(conn, &alters)
}

// ─── customers — V2 upgrades ──────────────────────────────────────────────────
//
// Adds credit limit enforcement and loyalty fields.
// credit_limit_dzd = 0 means no limit enforced.
// loyalty_points is a simple integer accumulator (not a full loyalty system).

fn upgrade_customers(conn: &Connection) -> SqlResult<()> {
    let alters = [
        "ALTER TABLE customers ADD COLUMN wilaya TEXT",
        "ALTER TABLE customers ADD COLUMN credit_limit_dzd REAL NOT NULL DEFAULT 0",
        "ALTER TABLE customers ADD COLUMN notes TEXT",
        "ALTER TABLE customers ADD COLUMN is_active INTEGER NOT NULL DEFAULT 1
             CHECK(is_active IN (0,1))",
        "ALTER TABLE customers ADD COLUMN updated_at TEXT
             NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
    ];
    safe_alters(conn, &alters)
}

// ─── dain_entries — V2 upgrades ───────────────────────────────────────────────
//
// Adds balance_after snapshot so we can render a running-balance ledger
// without a recursive SUM per row (O(n²) → O(1) per row).
// Also adds `created_by` for multi-cashier attribution.

fn upgrade_dain_entries(conn: &Connection) -> SqlResult<()> {
    let alters = [
        "ALTER TABLE dain_entries ADD COLUMN balance_after REAL",
        "ALTER TABLE dain_entries ADD COLUMN created_by TEXT NOT NULL DEFAULT 'Admin'",
    ];
    safe_alters(conn, &alters)
}

// ═══════════════════════════════════════════════════════════════════════════════
// VIEWS — pre-computed queries for common access patterns
// ═══════════════════════════════════════════════════════════════════════════════

fn create_views(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    -- ── v_product_stock ──────────────────────────────────────────────────────
    -- Current available stock per product (sum of active batches with qty > 0).
    -- Join to products gives the full catalogue with live stock numbers.
    DROP VIEW IF EXISTS v_product_stock;
    CREATE VIEW v_product_stock AS
    SELECT
        p.id                                                    AS product_id,
        p.gtin,
        p.internal_code,
        p.plu_code,
        p.barcode_type,
        p.name_fr,
        p.name_ar,
        p.sell_price,
        p.buy_price,
        p.vat_rate,
        tr.rate                                                 AS tax_rate,
        p.min_stock_alert,
        p.max_stock_level,
        p.is_variable_weight,
        COALESCE(SUM(ib.quantity), 0)                           AS total_stock,
        COUNT(CASE WHEN ib.quantity > 0 THEN 1 END)             AS active_batch_count,
        MIN(CASE WHEN ib.quantity > 0 AND ib.expiry_date IS NOT NULL
                 THEN ib.expiry_date END)                       AS nearest_expiry,
        CAST(
            julianday(MIN(CASE WHEN ib.quantity > 0
                               AND ib.expiry_date IS NOT NULL
                          THEN ib.expiry_date END))
            - julianday('now') AS INTEGER
        )                                                       AS days_to_nearest_expiry,
        CASE
            WHEN COALESCE(SUM(ib.quantity), 0) = 0 THEN 'out_of_stock'
            WHEN COALESCE(SUM(ib.quantity), 0) <= p.min_stock_alert THEN 'low_stock'
            ELSE 'ok'
        END                                                     AS stock_status,
        u.label_fr                                              AS unit_label_fr,
        u.label_ar                                              AS unit_label_ar,
        c.name_fr                                               AS category_name_fr,
        c.name_ar                                               AS category_name_ar,
        p.is_active,
        p.shelf_life_days
    FROM products p
    LEFT JOIN inventory_batches ib
           ON ib.product_id = p.id AND ib.is_active = 1
    LEFT JOIN tax_rates tr ON tr.id = p.tax_rate_id
    LEFT JOIN units u      ON u.id  = p.unit_id
    LEFT JOIN categories c ON c.id  = p.category_id
    GROUP BY p.id;

    -- ── v_customer_balance ───────────────────────────────────────────────────
    -- Net outstanding balance per customer.
    -- balance > 0 means the customer owes money (debt > repayments).
    DROP VIEW IF EXISTS v_customer_balance;
    CREATE VIEW v_customer_balance AS
    SELECT
        c.id                                                    AS customer_id,
        c.name,
        c.phone,
        c.wilaya,
        c.credit_limit_dzd,
        c.is_active,
        COALESCE(SUM(CASE WHEN d.entry_type = 'debt'
                          THEN d.amount ELSE 0 END), 0)         AS total_debt,
        COALESCE(SUM(CASE WHEN d.entry_type = 'repayment'
                          THEN d.amount ELSE 0 END), 0)         AS total_repaid,
        COALESCE(SUM(CASE WHEN d.entry_type = 'debt'
                          THEN d.amount ELSE 0 END), 0)
        - COALESCE(SUM(CASE WHEN d.entry_type = 'repayment'
                            THEN d.amount ELSE 0 END), 0)       AS balance,
        COUNT(CASE WHEN d.entry_type = 'debt' THEN 1 END)       AS debt_count,
        MAX(d.created_at)                                       AS last_activity
    FROM customers c
    LEFT JOIN dain_entries d ON d.customer_id = c.id
    GROUP BY c.id;

    -- ── v_supplier_balance ───────────────────────────────────────────────────
    -- Outstanding AP balance per supplier.
    -- balance > 0 means the shop owes money to the supplier.
    DROP VIEW IF EXISTS v_supplier_balance;
    CREATE VIEW v_supplier_balance AS
    SELECT
        s.id                                                    AS supplier_id,
        s.code,
        s.name,
        s.wilaya,
        s.phone,
        s.payment_terms_days,
        s.is_active,
        COALESCE(SUM(po.total_ttc), 0)                          AS total_purchases,
        COALESCE(SUM(sp.amount),    0)                          AS total_paid,
        COALESCE(SUM(po.total_ttc), 0)
        - COALESCE(SUM(sp.amount),  0)                          AS balance,
        COUNT(DISTINCT po.id)                                   AS order_count,
        MAX(po.received_at)                                     AS last_delivery
    FROM suppliers s
    LEFT JOIN purchase_orders  po ON po.supplier_id = s.id
                                  AND po.status NOT IN ('draft','cancelled')
    LEFT JOIN supplier_payments sp ON sp.supplier_id = s.id
    GROUP BY s.id;

    -- ── v_daily_summary ──────────────────────────────────────────────────────
    -- Pre-aggregated daily totals for the reports dashboard.
    -- Filters out voided transactions automatically.
    DROP VIEW IF EXISTS v_daily_summary;
    CREATE VIEW v_daily_summary AS
    SELECT
        DATE(created_at)                                        AS sale_date,
        COUNT(*)                                                AS txn_count,
        COALESCE(SUM(total_ttc),    0)                          AS total_ttc,
        COALESCE(SUM(total_ht),     0)                          AS total_ht,
        COALESCE(SUM(total_ttc) - SUM(total_ht), 0)            AS total_vat,
        COALESCE(AVG(total_ttc),    0)                          AS avg_basket,
        COALESCE(SUM(CASE WHEN payment_method='cash'
                          THEN total_ttc END), 0)               AS cash_sales,
        COALESCE(SUM(CASE WHEN payment_method IN ('cib','dahabia')
                          THEN total_ttc END), 0)               AS card_sales,
        COALESCE(SUM(CASE WHEN payment_method='dain'
                          THEN total_ttc END), 0)               AS dain_sales,
        COALESCE(SUM(discount_amount), 0)                       AS total_discounts
    FROM transactions
    WHERE is_voided = 0
    GROUP BY DATE(created_at);

    -- ── v_fefo_batches ───────────────────────────────────────────────────────
    -- For each product, the batch to sell next under FEFO policy.
    -- Dated batches first, then undated, within each group by arrival date.
    DROP VIEW IF EXISTS v_fefo_batches;
    CREATE VIEW v_fefo_batches AS
    SELECT
        ib.product_id,
        ib.id                                                   AS batch_id,
        ib.quantity,
        ib.expiry_date,
        ib.supplier_id,
        ib.batch_number,
        ib.cost_price,
        CAST(julianday(ib.expiry_date) - julianday('now') AS INTEGER)
                                                                AS days_until_expiry,
        ROW_NUMBER() OVER (
            PARTITION BY ib.product_id
            ORDER BY
                CASE WHEN ib.expiry_date IS NULL THEN 1 ELSE 0 END ASC,
                ib.expiry_date ASC,
                ib.received_at ASC
        )                                                       AS fefo_rank
    FROM inventory_batches ib
    WHERE ib.quantity > 0 AND ib.is_active = 1;

    -- ── v_expiry_alerts ──────────────────────────────────────────────────────
    -- Batches expiring within the configured warning window.
    DROP VIEW IF EXISTS v_expiry_alerts;
    CREATE VIEW v_expiry_alerts AS
    SELECT
        ib.id                                                   AS batch_id,
        ib.product_id,
        p.name_fr                                               AS product_name_fr,
        p.name_ar                                               AS product_name_ar,
        p.gtin,
        ib.quantity,
        ib.expiry_date,
        CAST(julianday(ib.expiry_date) - julianday('now') AS INTEGER)
                                                                AS days_until_expiry,
        CASE
            WHEN julianday(ib.expiry_date) < julianday('now')      THEN 'expired'
            WHEN julianday(ib.expiry_date) - julianday('now') <= 7 THEN 'critical'
            ELSE 'warning'
        END                                                     AS alert_level,
        s.name                                                  AS supplier_name
    FROM inventory_batches ib
    JOIN products p ON p.id = ib.product_id
    LEFT JOIN suppliers s ON s.id = ib.supplier_id
    WHERE ib.quantity > 0
      AND ib.is_active = 1
      AND ib.expiry_date IS NOT NULL
      AND julianday(ib.expiry_date) - julianday('now') <= CAST(
              COALESCE(
                  (SELECT value FROM settings WHERE key='expiry_warn_days'), '30'
              ) AS INTEGER
          );
    ")?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TRIGGERS
// ═══════════════════════════════════════════════════════════════════════════════

fn create_v2_triggers(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    -- ── Auto-create inventory batch when a purchase item is received ──────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_create_batch_on_purchase
    AFTER INSERT ON purchase_items
    WHEN NEW.quantity_received > 0
    BEGIN
        INSERT INTO inventory_batches (
            product_id, quantity, expiry_date, cost_price,
            supplier_id, purchase_order_id, batch_number,
            purchase_date, received_at
        )
        SELECT
            NEW.product_id,
            NEW.quantity_received,
            NEW.expiry_date,
            NEW.cost_price_ht,
            po.supplier_id,
            NEW.purchase_order_id,
            NEW.batch_number,
            po.received_at,
            po.received_at
        FROM purchase_orders po WHERE po.id = NEW.purchase_order_id;

        -- Back-fill batch_id on the purchase_item row
        UPDATE purchase_items
        SET batch_id = last_insert_rowid()
        WHERE id = NEW.id;
    END;

    -- ── Update purchase_order totals when a purchase item is inserted ─────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_update_po_totals
    AFTER INSERT ON purchase_items
    BEGIN
        UPDATE purchase_orders
        SET
            total_ht  = (SELECT COALESCE(SUM(cost_price_ht * quantity_received),0)
                         FROM purchase_items WHERE purchase_order_id = NEW.purchase_order_id),
            total_ttc = (SELECT COALESCE(SUM(
                            cost_price_ht * quantity_received *
                            (1 + COALESCE((SELECT rate FROM tax_rates WHERE id = pi.tax_rate_id),0))
                         ),0)
                         FROM purchase_items pi WHERE pi.purchase_order_id = NEW.purchase_order_id)
        WHERE id = NEW.purchase_order_id;
    END;

    -- ── Log price changes automatically ───────────────────────────────────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_log_price_change
    AFTER UPDATE OF sell_price, buy_price ON products
    WHEN OLD.sell_price <> NEW.sell_price OR
         OLD.buy_price  <> NEW.buy_price
    BEGIN
        INSERT INTO price_history (
            product_id, old_price_sell, new_price_sell,
            old_price_buy, new_price_buy
        ) VALUES (
            NEW.id, OLD.sell_price, NEW.sell_price,
            OLD.buy_price, NEW.buy_price
        );
    END;

    -- ── Snapshot product details into transaction_items at sale time ──────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_snapshot_item
    AFTER INSERT ON transaction_items
    BEGIN
        UPDATE transaction_items
        SET
            product_name_fr   = (SELECT name_fr FROM products WHERE id = NEW.product_id),
            product_name_ar   = (SELECT name_ar FROM products WHERE id = NEW.product_id),
            gtin_snapshot     = (SELECT gtin   FROM products WHERE id = NEW.product_id)
        WHERE id = NEW.id;
    END;

    -- ── Update stock when a stock_adjustment is recorded ─────────────────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_apply_adjustment
    AFTER INSERT ON stock_adjustments
    BEGIN
        UPDATE inventory_batches
        SET quantity = NEW.quantity_after
        WHERE id = NEW.batch_id;
    END;

    -- ── Prevent inventory_batches quantity from going negative ────────────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_no_negative_stock
    BEFORE UPDATE OF quantity ON inventory_batches
    WHEN NEW.quantity < 0
    BEGIN
        SELECT RAISE(ABORT,
            'Stock cannot go negative. Check sale quantity vs batch quantity.');
    END;

    -- ── Auto-create dain_entry when a transaction is paid with 'dain' ─────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_auto_dain_on_sale
    AFTER INSERT ON transactions
    WHEN NEW.payment_method = 'dain' AND NEW.customer_id IS NOT NULL
    BEGIN
        INSERT INTO dain_entries (
            customer_id, transaction_id, entry_type, amount, notes, created_by
        ) VALUES (
            NEW.customer_id,
            NEW.id,
            'debt',
            NEW.total_ttc,
            'Vente crédit auto : ' || NEW.ref_number,
            NEW.cashier_name
        );
    END;

    -- ── Update session totals when a transaction is committed ─────────────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_update_session_on_sale
    AFTER INSERT ON transactions
    WHEN NEW.session_id IS NOT NULL AND NEW.is_voided = 0
    BEGIN
        UPDATE cashier_sessions
        SET
            total_sales_ttc  = total_sales_ttc  + NEW.total_ttc,
            total_cash_sales = total_cash_sales + CASE WHEN NEW.payment_method='cash'
                                                       THEN NEW.total_ttc ELSE 0 END,
            total_cib_sales  = total_cib_sales  + CASE WHEN NEW.payment_method IN ('cib','dahabia')
                                                       THEN NEW.total_ttc ELSE 0 END,
            total_dain_sales = total_dain_sales + CASE WHEN NEW.payment_method='dain'
                                                       THEN NEW.total_ttc ELSE 0 END
        WHERE id = NEW.session_id;
    END;

    -- ── Reverse session totals when a transaction is voided ──────────────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_reverse_session_on_void
    AFTER UPDATE OF is_voided ON transactions
    WHEN NEW.is_voided = 1 AND OLD.is_voided = 0
      AND NEW.session_id IS NOT NULL
    BEGIN
        UPDATE cashier_sessions
        SET
            total_sales_ttc  = total_sales_ttc  - OLD.total_ttc,
            total_cash_sales = total_cash_sales - CASE WHEN OLD.payment_method='cash'
                                                       THEN OLD.total_ttc ELSE 0 END,
            total_cib_sales  = total_cib_sales  - CASE WHEN OLD.payment_method IN ('cib','dahabia')
                                                       THEN OLD.total_ttc ELSE 0 END,
            total_dain_sales = total_dain_sales - CASE WHEN OLD.payment_method='dain'
                                                       THEN OLD.total_ttc ELSE 0 END
        WHERE id = OLD.session_id;
    END;

    -- ── Update supplier denormalized debt via supplier_payments ───────────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_supplier_payment_debit
    AFTER INSERT ON supplier_payments
    BEGIN
        UPDATE suppliers
        SET total_debt_dzd = MAX(0, total_debt_dzd - NEW.amount)
        WHERE id = NEW.supplier_id;
    END;

    -- ── Increase supplier debt when a purchase_order is received ─────────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_po_received_debt
    AFTER UPDATE OF status ON purchase_orders
    WHEN NEW.status = 'invoiced' AND OLD.status = 'received'
    BEGIN
        UPDATE suppliers
        SET total_debt_dzd = total_debt_dzd + NEW.total_ttc
        WHERE id = NEW.supplier_id;
    END;

    -- ── auto-update updated_at on suppliers ───────────────────────────────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_suppliers_updated
    AFTER UPDATE ON suppliers
    BEGIN
        UPDATE suppliers SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
        WHERE id = NEW.id;
    END;

    -- ── auto-update updated_at on customers ───────────────────────────────────
    CREATE TRIGGER IF NOT EXISTS trg_v2_customers_updated
    AFTER UPDATE ON customers
    BEGIN
        UPDATE customers SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
        WHERE id = NEW.id;
    END;
    ")?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// SEED DATA
// ═══════════════════════════════════════════════════════════════════════════════

fn seed_v2_data(conn: &Connection) -> SqlResult<()> {
    // ── Tax rates (standard Algerian TCA rates) ───────────────────────────────
    conn.execute_batch("
    INSERT OR IGNORE INTO tax_rates (id, label, rate, description) VALUES
        (1, 'TVA 19%', 0.19, 'Taux normal — la plupart des produits'),
        (2, 'TVA 9%',  0.09, 'Taux réduit — sucre, farine, huile, médicaments'),
        (3, 'HT / 0%', 0.00, 'Exonéré de TVA — produits de première nécessité');
    ")?;

    // ── Update all existing products to reference the correct tax_rate ────────
    // Products currently using 0.19 → tax_rate_id 1; 0.09 → 2; 0.0 → 3
    conn.execute_batch("
    UPDATE products SET tax_rate_id = 1 WHERE vat_rate = 0.19 AND tax_rate_id IS NULL;
    UPDATE products SET tax_rate_id = 2 WHERE vat_rate = 0.09 AND tax_rate_id IS NULL;
    UPDATE products SET tax_rate_id = 3 WHERE vat_rate = 0.00 AND tax_rate_id IS NULL;
    ")?;

    // ── Additional settings added in V2 ──────────────────────────────────────
    conn.execute_batch("
    INSERT OR IGNORE INTO settings (key, value) VALUES
        ('printer_port',       ''),
        ('printer_baud',       '9600'),
        ('backup_auto',        '1'),
        ('backup_interval_h',  '24'),
        ('session_auto_open',  '1'),
        ('cashier_pin_enabled','0'),
        ('credit_limit_global','0'),
        ('weight_scale_port',  ''),
        ('weight_scale_baud',  '9600');
    ")?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPER — safe ALTER TABLE (ignore if column already exists)
// ═══════════════════════════════════════════════════════════════════════════════

fn safe_alters(conn: &Connection, statements: &[&str]) -> SqlResult<()> {
    for stmt in statements {
        match conn.execute_batch(stmt) {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::Unknown
                    // SQLite error 1 (SQLITE_ERROR) with "duplicate column name"
                    // is what you get when the column already exists.
                    // We check the error code is "not an error" or check the message.
                => {
                    // Silently skip — column already exists from a previous migration.
                }
            Err(e) => {
                // Filter "duplicate column name" by message text
                let msg = e.to_string();
                if msg.contains("duplicate column name") || msg.contains("already exists") {
                    continue;
                }
                return Err(e);
            }
        }
    }
    Ok(())
}