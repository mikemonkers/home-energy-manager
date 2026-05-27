pub mod inverter;
pub mod modbus;
pub mod server;
pub mod settings;

use std::sync::Arc;
use inverter::poll::{AppState, run_poll_loop};
use server::start_server;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      // Create shared app state
      let state = Arc::new(AppState::new());

      // Spawn the HTTP server on LAN interface, port 7337
      let server_state = state.clone();
      tokio::spawn(async move {
        start_server(server_state, "0.0.0.0", 7337).await;
      });

      // Spawn the Modbus polling loop
      let poll_state = state.clone();
      tokio::spawn(async move {
        run_poll_loop(poll_state).await;
      });

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
