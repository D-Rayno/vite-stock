// src-tauri/src/main.rs
//
// Thin binary entry point.
// All app setup lives in lib.rs → vite_stock_lib::run().
// Keeping this file minimal lets the mobile targets (iOS/Android) call
// vite_stock_lib::run() from their own platform entry points without any
// duplication.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    vite_stock_lib::run()
}