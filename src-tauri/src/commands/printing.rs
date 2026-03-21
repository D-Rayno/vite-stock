// src-tauri/src/commands/printing.rs
//! Thermal printer commands — both USB/Serial (COM port) and Network (TCP/IP).
//!
//! ## Transport selection
//! Many Algerian shops use Xprinter / Gprinter models with RJ-45 network cards.
//! `PrintTarget` selects the transport:
//!   - `Serial { port, baud }` — USB or RS-232 COM port
//!   - `Network { host, port }` — TCP/IP (default port 9100)
//!
//! The ESC/POS byte stream is identical for both — only the transport differs.
//!
//! ## Feature gate
//! All commands require `features::THERMAL_PRINT` in the active license.
//! The gate is checked in Rust before any hardware I/O.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;
use tauri::{command, State};

use crate::{
    license::features,
    utils::escpos::{self, EscPosBuilder, ReceiptData},
    AppState,
};

// ─── Transport types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "transport")]
pub enum PrintTarget {
    /// USB / RS-232 serial port.
    Serial {
        port: String,
        baud: Option<u32>,
    },
    /// Network printer via TCP (port 9100 is the ESC/POS standard).
    Network {
        host:    String,
        port:    Option<u16>,
    },
}

impl PrintTarget {
    fn send(&self, bytes: &[u8]) -> Result<(), String> {
        match self {
            PrintTarget::Serial { port, baud } => {
                write_to_serial(port, baud.unwrap_or(9600), bytes)
            }
            PrintTarget::Network { host, port } => {
                write_to_network(host, port.unwrap_or(9100), bytes)
            }
        }
    }
}

// ─── Printer discovery ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct PrinterPort {
    pub port:           String,
    pub description:    String,
    pub likely_thermal: bool,
    pub transport:      String,   // "serial" | "network"
}

/// List available serial ports (USB / COM).
/// Network printers must be configured manually via IP.
#[command]
pub async fn cmd_list_printers(state: State<'_, AppState>) -> Result<Vec<PrinterPort>, String> {
    {
        let lic = state.license.lock().unwrap();
        if !lic.has_feature(features::THERMAL_PRINT) {
            return Err("Impression thermique non activée sur cette licence.".into());
        }
    }

    let ports = serialport::available_ports()
        .map_err(|e| format!("Impossible de lister les ports: {e}"))?;

    Ok(ports.into_iter().map(|p| {
        let description = match &p.port_type {
            serialport::SerialPortType::UsbPort(usb) => {
                format!(
                    "{} {}",
                    usb.manufacturer.clone().unwrap_or_default(),
                    usb.product.clone().unwrap_or_default()
                ).trim().to_string()
            }
            _ => format!("{:?}", p.port_type),
        };
        let likely_thermal = matches!(p.port_type, serialport::SerialPortType::UsbPort(_))
            || p.port_name.contains("COM")
            || p.port_name.contains("USB");
        PrinterPort {
            port:           p.port_name,
            description,
            likely_thermal,
            transport:      "serial".into(),
        }
    }).collect())
}

/// Test whether a network printer is reachable (TCP connect with short timeout).
#[command]
pub async fn cmd_test_network_printer(
    state: State<'_, AppState>,
    host:  String,
    port:  Option<u16>,
) -> Result<bool, String> {
    {
        let lic = state.license.lock().unwrap();
        if !lic.has_feature(features::THERMAL_PRINT) {
            return Err("Licence insuffisante.".into());
        }
    }
    let addr = format!("{}:{}", host, port.unwrap_or(9100));
    let reachable = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("Adresse invalide: {e}"))?,
        Duration::from_secs(3),
    ).is_ok();
    Ok(reachable)
}

// ─── Print receipt ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PrintReceiptRequest {
    pub data:   ReceiptData,
    pub target: PrintTarget,
}

/// Print a sale receipt to the configured thermal printer.
#[command]
pub async fn cmd_print_thermal_receipt(
    state:   State<'_, AppState>,
    request: PrintReceiptRequest,
) -> Result<(), String> {
    {
        let lic = state.license.lock().unwrap();
        if !lic.has_feature(features::THERMAL_PRINT) {
            return Err("Impression thermique non activée sur cette licence.".into());
        }
    }
    let bytes = escpos::build_receipt(&request.data);
    request.target.send(&bytes)
        .map_err(|e| format!("Erreur d'impression: {e}"))
}

// ─── Dain thermal statement ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DainStatementRequest {
    pub customer_id: i64,
    pub target:      PrintTarget,
}

/// Print a compact customer Dain ledger statement on the thermal printer.
/// Useful for handing a paper copy to the customer at the counter.
#[command]
pub async fn cmd_print_thermal_dain_statement(
    state:   State<'_, AppState>,
    request: DainStatementRequest,
) -> Result<(), String> {
    {
        let lic = state.license.lock().unwrap();
        if !lic.has_feature(features::THERMAL_PRINT) {
            return Err("Impression thermique non activée sur cette licence.".into());
        }
        if !lic.has_feature(features::DAIN_LEDGER) {
            return Err("Fonctionnalité Dain non activée sur cette licence.".into());
        }
    }

    // Gather data under a short mutex lock
    let (shop_name, shop_address, shop_phone, thermal_width, cust_name, cust_phone, balance, entries) = {
        use rusqlite::params;
        let db   = state.db.lock().unwrap();
        let conn = &db.0;

        let get_setting = |k: &str| -> String {
            conn.query_row(
                "SELECT value FROM settings WHERE key=?1",
                params![k],
                |r| r.get::<_, String>(0),
            ).unwrap_or_default()
        };

        let shop_name    = get_setting("shop_name_fr");
        let shop_address = get_setting("shop_address");
        let shop_phone   = get_setting("shop_phone");
        let width        = get_setting("thermal_width").parse::<u8>().unwrap_or(80);

        let (cust_name, cust_phone): (String, String) = conn.query_row(
            "SELECT name, phone FROM customers WHERE id=?1",
            params![request.customer_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).map_err(|_| "Client introuvable.".to_string())?;

        let balance: f64 = conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN entry_type='debt' THEN amount ELSE -amount END),0)
             FROM dain_entries WHERE customer_id=?1",
            params![request.customer_id],
            |r| r.get(0),
        ).unwrap_or(0.0);

        let mut stmt = conn.prepare(
            "SELECT entry_type, amount, COALESCE(notes,''), created_at
             FROM dain_entries WHERE customer_id=?1
             ORDER BY created_at DESC LIMIT 15"
        ).map_err(|e| e.to_string())?;

        let entries: Vec<(String, f64, String, String)> = stmt.query_map(
            params![request.customer_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, String>(3)?)),
        ).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        (shop_name, shop_address, shop_phone, width, cust_name, cust_phone, balance, entries)
    };

    // Build ESC/POS byte stream
    let mut b = EscPosBuilder::new(thermal_width);
    let w = b.width();

    b.align_center();
    b.bold_on(); b.double_height_on();
    b.text(&shop_name);
    b.normal_size(); b.bold_off();
    b.text(&shop_address);
    if !shop_phone.is_empty() {
        b.text(&format!("Tel: {}", shop_phone));
    }
    b.align_left();
    b.rule_double();

    b.bold_on();
    b.text("RELEVE DAIN - CREDIT CLIENT");
    b.bold_off();
    b.text(&format!("Client : {}", cust_name));
    b.text(&format!("Tel    : {}", cust_phone));
    b.text(&format!("Date   : {}", chrono::Utc::now().format("%d/%m/%Y %H:%M")));
    b.rule();

    // Balance
    b.bold_on(); b.double_size_on();
    let balance_label = if balance > 0.0 { "SOLDE DU  :" } else { "CREDIT    :" };
    b.row(balance_label, &format!("{:.2} DZD", balance.abs()));
    b.normal_size(); b.bold_off();
    b.rule();

    // Last 15 entries
    b.bold_on();
    b.text("15 DERNIERES OPERATIONS :");
    b.bold_off();
    b.rule();

    for (etype, amount, notes, date) in &entries {
        let date_short = date.chars().take(16).collect::<String>().replace('T', " ");
        let sign = if etype == "debt" { "+" } else { "-" };
        b.row(&date_short, &format!("{sign}{amount:.2}"));
        if !notes.is_empty() {
            b.text(&format!("  {}", &notes.chars().take(w.saturating_sub(2)).collect::<String>()));
        }
    }

    b.rule_double();
    b.align_center();
    b.bold_on();
    b.text("Merci de votre confiance");
    b.bold_off();
    b.lf_n(3);
    b.cut();

    let bytes = b.finish();
    request.target.send(&bytes)
        .map_err(|e| format!("Erreur d'impression du relevé: {e}"))
}

// ─── Test page ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TestPageRequest {
    pub target: PrintTarget,
}

#[command]
pub async fn cmd_print_test_page(
    state:   State<'_, AppState>,
    request: TestPageRequest,
) -> Result<(), String> {
    {
        let lic = state.license.lock().unwrap();
        if !lic.has_feature(features::THERMAL_PRINT) {
            return Err("Licence insuffisante.".into());
        }
    }
    let bytes = build_test_bytes();
    request.target.send(&bytes)
        .map_err(|e| format!("Test d'impression échoué: {e}"))
}

fn build_test_bytes() -> Vec<u8> {
    let mut b = EscPosBuilder::new(80);
    b.align_center();
    b.double_size_on(); b.bold_on();
    b.text("SuperPOS");
    b.normal_size(); b.bold_off();
    b.text("PAGE DE TEST — Imprimante OK");
    b.rule();
    b.align_left();
    b.text("Caracteres FR: e a c u E A C");
    b.bold_on(); b.text("Gras fonctionne"); b.bold_off();
    b.text(&format!(
        "Transport: {:?}",
        chrono::Utc::now().format("%d/%m/%Y %H:%M:%S")
    ));
    b.lf_n(3); b.cut();
    b.finish()
}

// ─── I/O transports ───────────────────────────────────────────────────────────

fn write_to_serial(port_name: &str, baud: u32, bytes: &[u8]) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let port_path = if port_name.starts_with("COM") && !port_name.starts_with(r"\\") {
        format!(r"\\.\{port_name}")
    } else {
        port_name.to_string()
    };
    #[cfg(not(target_os = "windows"))]
    let port_path = port_name.to_string();

    let mut port = serialport::new(&port_path, baud)
        .timeout(Duration::from_secs(5))
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None)
        .open()
        .map_err(|e| format!("Impossible d'ouvrir {port_name}: {e}"))?;

    for chunk in bytes.chunks(512) {
        port.write_all(chunk).map_err(|e| format!("Erreur écriture série: {e}"))?;
    }
    port.flush().map_err(|e| format!("Flush série: {e}"))?;
    Ok(())
}

fn write_to_network(host: &str, port: u16, bytes: &[u8]) -> Result<(), String> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect_timeout(
        &addr.parse().map_err(|e| format!("Adresse invalide: {e}"))?,
        Duration::from_secs(5),
    ).map_err(|e| format!("Connexion impossible à {addr}: {e}"))?;

    stream.set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;

    for chunk in bytes.chunks(512) {
        stream.write_all(chunk).map_err(|e| format!("Erreur écriture réseau: {e}"))?;
    }
    stream.flush().map_err(|e| format!("Flush réseau: {e}"))?;
    Ok(())
}