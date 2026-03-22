// src-tauri/src/bin/activate-license.rs
use std::path::PathBuf;
use vite_stock_lib::license::{verify_and_build_state, save_to_disk};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let key = match args.get(1) {
        Some(k) => k,
        None => {
            eprintln!("Usage: activate-license <SUPERPOS-KEY>");
            std::process::exit(1);
        }
    };

    // Determine app data dir (Linux specific for now, as user is on Linux)
    let home = std::env::var("HOME").expect("HOME not set");
    let app_dir = PathBuf::from(home).join(".local/share/com.rayno.vite-stock");
    
    if !app_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&app_dir) {
             eprintln!("❌ Failed to create app data dir: {}", e);
             std::process::exit(1);
        }
    }

    println!("Validating license...");
    match verify_and_build_state(key) {
        Ok(state) => {
            println!("✅ License is VALID for this machine!");
            println!("  Tier: {}", state.tier);
            
            println!("Saving to {}...", app_dir.display());
            if let Err(e) = save_to_disk(&app_dir, key) {
                eprintln!("❌ Failed to save license: {}", e);
                std::process::exit(1);
            }
            
            println!("🚀 Activation SUCCESSFUL! Restart the app to apply.");
        }
        Err(e) => {
            eprintln!("❌ License validation FAILED: {}", e);
            std::process::exit(1);
        }
    }
}
