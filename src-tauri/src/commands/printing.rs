// src-tauri/src/commands/printing.rs
use serde::{Deserialize, Serialize};
use tauri::{command, State};
use crate::{license::features, utils::escpos::{self, ReceiptData}, AppState};

// ─── Printer discovery ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct PrinterPort {
    pub port:           String,
    pub description:    String,
    pub likely_thermal: bool,
}

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
        PrinterPort { port: p.port_name, description, likely_thermal }
    }).collect())
}

// ─── Print receipt ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PrintReceiptRequest {
    pub data: ReceiptData,
    pub port: String,
    pub baud: Option<u32>,
}

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
    write_to_port(&request.port, request.baud.unwrap_or(9600), &bytes)
        .map_err(|e| format!("Erreur d'impression sur {}: {e}", request.port))
}

#[command]
pub async fn cmd_print_test_page(
    state: State<'_, AppState>,
    port:  String,
    baud:  Option<u32>,
) -> Result<(), String> {
    {
        let lic = state.license.lock().unwrap();
        if !lic.has_feature(features::THERMAL_PRINT) {
            return Err("Licence insuffisante.".into());
        }
    }
    let bytes = build_test_bytes();
    write_to_port(&port, baud.unwrap_or(9600), &bytes)
        .map_err(|e| format!("Test d'impression échoué sur {port}: {e}"))
}

fn write_to_port(port_name: &str, baud: u32, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::time::Duration;

    #[cfg(target_os = "windows")]
    let port_path = if port_name.starts_with("COM") && !port_name.starts_with(r"\\") {
        format!(r"\\.\{port_name}")
    } else { port_name.to_string() };
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
        port.write_all(chunk).map_err(|e| format!("Erreur d'écriture: {e}"))?;
    }
    port.flush().map_err(|e| format!("Flush: {e}"))?;
    Ok(())
}

fn build_test_bytes() -> Vec<u8> {
    use crate::utils::escpos::EscPosBuilder;
    let mut b = EscPosBuilder::new(80);
    b.align_center();
    b.double_size_on(); b.bold_on();
    b.text("SuperPOS");
    b.normal_size(); b.bold_off();
    b.text("Page de test - Imprimante OK");
    b.rule();
    b.align_left();
    b.text("Caracteres FR: e a c u E A C");
    b.bold_on(); b.text("Gras fonctionne"); b.bold_off();
    b.lf_n(3); b.cut();
    b.finish()
}