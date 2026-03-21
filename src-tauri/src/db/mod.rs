// src-tauri/src/db/mod.rs
//! Database layer — connection, pragma configuration, and migration runner.
//!
//! Migration history:
//!   V1 (schema_v1) — baseline: products, inventory_batches, customers,
//!                     transactions/items, dain_entries, settings
//!   V2 (schema_v2) — Phase 3: suppliers, purchase_orders, purchase_items,
//!                     stock_adjustments, cashier_sessions, sale_payments,
//!                     price_history, tax_rates, views, full trigger suite

mod schema_v2;

use rusqlite::{Connection, Result as SqlResult, params};
use std::path::Path;

/// Thread-safe wrapper around `rusqlite::Connection`.
/// Access is serialized by the `Mutex<DbConnection>` in `AppState`.
pub struct DbConnection(pub Connection);
unsafe impl Send for DbConnection {}

// ─── Public API ───────────────────────────────────────────────────────────────

pub fn open_and_migrate(path: &Path) -> SqlResult<DbConnection> {
    let conn = Connection::open(path)?;
    configure_pragmas(&conn)?;

    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    run_migrations(&conn, version)?;

    Ok(DbConnection(conn))
}

// ─── Pragma configuration ─────────────────────────────────────────────────────

fn configure_pragmas(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
        PRAGMA journal_mode = WAL;          -- concurrent reads during reports
        PRAGMA foreign_keys = ON;           -- enforce all FK constraints
        PRAGMA synchronous   = NORMAL;      -- safe + fast (not paranoid-safe)
        PRAGMA temp_store    = MEMORY;      -- temp tables stay in RAM
        PRAGMA cache_size    = -16000;      -- 16 MB page cache
        PRAGMA mmap_size     = 268435456;   -- 256 MB memory-mapped I/O
        PRAGMA auto_vacuum   = INCREMENTAL; -- reclaim space without full VACUUM
        PRAGMA busy_timeout  = 5000;        -- 5s retry on locked database
    ")?;
    Ok(())
}

// ─── Migration runner ─────────────────────────────────────────────────────────

fn run_migrations(conn: &Connection, from_version: i64) -> SqlResult<()> {
    // Every entry is (target_version, migration_fn).
    // Migrations are cumulative — a fresh DB runs all of them in order.
    let migrations: &[(i64, fn(&Connection) -> SqlResult<()>)] = &[
        (1, migrate_v1),
        (2, schema_v2::migrate_v2),
    ];

    for (version, migrate_fn) in migrations {
        if from_version < *version {
            migrate_fn(conn)?;
            // Bump version atomically after success.
            conn.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        }
    }
    Ok(())
}

// ─── V1 — baseline schema ─────────────────────────────────────────────────────

fn migrate_v1(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
    -- ── Measurement units ────────────────────────────────────────────────────
    CREATE TABLE IF NOT EXISTS units (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        label_fr    TEXT    NOT NULL,
        label_ar    TEXT    NOT NULL DEFAULT ''
    );

    -- ── Product categories ───────────────────────────────────────────────────
    CREATE TABLE IF NOT EXISTS categories (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        name_fr     TEXT    NOT NULL,
        name_ar     TEXT    NOT NULL DEFAULT '',
        created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    -- ── Products (static catalogue identity) ────────────────────────────────
    CREATE TABLE IF NOT EXISTS products (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        gtin            TEXT    UNIQUE,
        name_fr         TEXT    NOT NULL,
        name_ar         TEXT    NOT NULL DEFAULT '',
        category_id     INTEGER REFERENCES categories(id) ON DELETE SET NULL,
        unit_id         INTEGER REFERENCES units(id)      ON DELETE SET NULL,
        sell_price      REAL    NOT NULL CHECK(sell_price >= 0),
        buy_price       REAL    NOT NULL DEFAULT 0 CHECK(buy_price >= 0),
        vat_rate        REAL    NOT NULL DEFAULT 0.19,
        min_stock_alert INTEGER NOT NULL DEFAULT 5,
        is_active       INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0,1)),
        created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_product_gtin     ON products(gtin);
    CREATE INDEX IF NOT EXISTS idx_product_category ON products(category_id);
    CREATE INDEX IF NOT EXISTS idx_product_active   ON products(is_active);

    -- ── Inventory batches ────────────────────────────────────────────────────
    CREATE TABLE IF NOT EXISTS inventory_batches (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        product_id  INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
        quantity    REAL    NOT NULL DEFAULT 0 CHECK(quantity >= 0),
        expiry_date TEXT,
        cost_price  REAL,
        received_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_batch_product ON inventory_batches(product_id);
    CREATE INDEX IF NOT EXISTS idx_batch_expiry  ON inventory_batches(expiry_date);

    -- ── Customers ────────────────────────────────────────────────────────────
    CREATE TABLE IF NOT EXISTS customers (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        name        TEXT    NOT NULL,
        phone       TEXT    NOT NULL UNIQUE,
        address     TEXT,
        created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_customer_phone ON customers(phone);

    -- ── Transactions (immutable sales ledger) ────────────────────────────────
    -- STRICT RULE: No card-number fields. payment_method is an enum string.
    CREATE TABLE IF NOT EXISTS transactions (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        ref_number      TEXT    NOT NULL UNIQUE,
        customer_id     INTEGER REFERENCES customers(id) ON DELETE SET NULL,
        total_ttc       REAL    NOT NULL,
        total_ht        REAL    NOT NULL,
        discount_amount REAL    NOT NULL DEFAULT 0,
        payment_method  TEXT    NOT NULL DEFAULT 'cash'
                            CHECK(payment_method IN ('cash','cib','dahabia','dain','cheque')),
        amount_paid     REAL    NOT NULL DEFAULT 0,
        change_given    REAL    NOT NULL DEFAULT 0,
        cashier_name    TEXT    NOT NULL DEFAULT 'Admin',
        notes           TEXT,
        created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_txn_date     ON transactions(created_at);
    CREATE INDEX IF NOT EXISTS idx_txn_customer ON transactions(customer_id);
    CREATE INDEX IF NOT EXISTS idx_txn_method   ON transactions(payment_method);

    -- ── Transaction line items ───────────────────────────────────────────────
    CREATE TABLE IF NOT EXISTS transaction_items (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        transaction_id  INTEGER NOT NULL REFERENCES transactions(id)    ON DELETE CASCADE,
        product_id      INTEGER NOT NULL REFERENCES products(id)        ON DELETE RESTRICT,
        batch_id        INTEGER          REFERENCES inventory_batches(id) ON DELETE SET NULL,
        quantity        REAL    NOT NULL CHECK(quantity > 0),
        unit_price      REAL    NOT NULL CHECK(unit_price >= 0),
        vat_rate        REAL    NOT NULL,
        discount_pct    REAL    NOT NULL DEFAULT 0 CHECK(discount_pct BETWEEN 0 AND 100)
    );
    CREATE INDEX IF NOT EXISTS idx_item_txn     ON transaction_items(transaction_id);
    CREATE INDEX IF NOT EXISTS idx_item_product ON transaction_items(product_id);

    -- ── Dain entries (customer credit ledger) ────────────────────────────────
    CREATE TABLE IF NOT EXISTS dain_entries (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        customer_id     INTEGER NOT NULL REFERENCES customers(id)   ON DELETE CASCADE,
        transaction_id  INTEGER          REFERENCES transactions(id) ON DELETE SET NULL,
        entry_type      TEXT    NOT NULL CHECK(entry_type IN ('debt','repayment')),
        amount          REAL    NOT NULL CHECK(amount > 0),
        notes           TEXT,
        created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_dain_customer ON dain_entries(customer_id);
    CREATE INDEX IF NOT EXISTS idx_dain_txn      ON dain_entries(transaction_id);

    -- ── Settings (key-value store) ───────────────────────────────────────────
    CREATE TABLE IF NOT EXISTS settings (
        key         TEXT PRIMARY KEY,
        value       TEXT NOT NULL,
        updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    -- ── Triggers ─────────────────────────────────────────────────────────────

    -- Auto-update products.updated_at
    CREATE TRIGGER IF NOT EXISTS trg_products_updated
    AFTER UPDATE ON products
    BEGIN
        UPDATE products
        SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
        WHERE id = NEW.id;
    END;

    -- Decrement inventory when a sale item is recorded
    CREATE TRIGGER IF NOT EXISTS trg_deduct_inventory
    AFTER INSERT ON transaction_items
    WHEN NEW.batch_id IS NOT NULL
    BEGIN
        UPDATE inventory_batches
        SET quantity = quantity - NEW.quantity
        WHERE id = NEW.batch_id;
    END;
    ")?;

    // Seed mandatory reference data
    conn.execute_batch("
    INSERT OR IGNORE INTO units (id, label_fr, label_ar) VALUES
        (1, 'Pièce',  'قطعة'),
        (2, 'Kg',     'كلغ'),
        (3, 'Litre',  'لتر'),
        (4, 'Carton', 'كرتون'),
        (5, 'Gr',     'غرام'),
        (6, 'Paquet', 'علبة');

    INSERT OR IGNORE INTO categories (id, name_fr, name_ar) VALUES
        (1, 'Divers',            'متنوع'),
        (2, 'Épicerie',          'بقالة'),
        (3, 'Boulangerie',       'مخبزة'),
        (4, 'Boucherie / Deli',  'جزارة'),
        (5, 'Produits laitiers', 'منتجات الألبان'),
        (6, 'Boissons',          'مشروبات'),
        (7, 'Hygiène',           'نظافة'),
        (8, 'Surgelés',          'مجمدات'),
        (9, 'Légumes & Fruits',  'خضر وفواكه');

    INSERT OR IGNORE INTO settings (key, value) VALUES
        ('shop_name_fr',     'Mon Supermarché'),
        ('shop_name_ar',     'سوبرماركت'),
        ('shop_address',     'Alger, Algérie'),
        ('shop_phone',       ''),
        ('shop_nif',         ''),
        ('shop_nis',         ''),
        ('shop_rc',          ''),
        ('default_language', 'fr'),
        ('currency',         'DZD'),
        ('thermal_width',    '80'),
        ('vat_display',      '1'),
        ('expiry_warn_days', '30');
    ")?;

    Ok(())
}