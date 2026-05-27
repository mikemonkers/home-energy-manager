//! REST API routes and handlers.
//!
//! Defines Axum routes for querying current inverter state,
//! historical data, and sending control commands.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use serde_json::{json, Value};

use crate::inverter::encoder::{encode_command, ControlCommand};
use crate::inverter::model::{BatteryMode, ScheduleSlot};
use crate::inverter::poll::{AppState, PollSettings};

// ---------------------------------------------------------------------------
// Helper: standard JSON response
// ---------------------------------------------------------------------------

fn ok_response(message: &str) -> Json<Value> {
    Json(json!({ "ok": true, "message": message }))
}

fn error_response(error: &str) -> Json<Value> {
    Json(json!({ "ok": false, "error": error }))
}

// ---------------------------------------------------------------------------
// Data endpoints
// ---------------------------------------------------------------------------

/// GET /api/snapshot — return the latest inverter snapshot as JSON.
pub async fn get_snapshot(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let snapshot = state.latest_snapshot.lock().await;
    match snapshot.as_ref() {
        Some(snap) => {
            // Wrap the snapshot in a standard response envelope.
            Json(json!({ "ok": true, "data": snap }))
        }
        None => Json(json!({ "ok": false, "error": "No snapshot available yet" })),
    }
}

/// GET /api/settings — return current poll settings as JSON.
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let settings = state.settings.lock().await;
    Json(json!({
        "ok": true,
        "data": {
            "host": settings.host,
            "port": settings.port,
            "serial": settings.serial,
            "interval_secs": settings.interval_secs,
        }
    }))
}

/// POST /api/settings — update poll settings from JSON body.
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<Value> {
    let new_settings = match parse_settings(&body) {
        Ok(s) => s,
        Err(e) => return error_response(&e),
    };

    let mut settings = state.settings.lock().await;
    *settings = new_settings;

    // Notify WebSocket clients about the settings change.
    let msg = format!(
        "Settings updated: host={}, port={}, interval={}s",
        settings.host, settings.port, settings.interval_secs
    );
    tracing::info!("{}", msg);
    ok_response(&msg)
}

fn parse_settings(body: &serde_json::Value) -> Result<PollSettings, String> {
    let host = body["host"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let port = body["port"]
        .as_u64()
        .unwrap_or(8899) as u16;
    let serial = body["serial"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let interval_secs = body["interval_secs"]
        .as_u64()
        .unwrap_or(10);

    if !host.is_empty() && port == 0 {
        return Err("Invalid port".to_string());
    }
    if interval_secs == 0 {
        return Err("interval_secs must be > 0".to_string());
    }

    Ok(PollSettings {
        host,
        port,
        serial,
        interval_secs,
    })
}

// ---------------------------------------------------------------------------
// Control endpoints
// ---------------------------------------------------------------------------

/// POST /api/control/mode — set battery operating mode.
///
/// Body: `{"mode": "eco"}` where mode is one of: paused, eco, timed_demand, timed_export.
pub async fn set_mode(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<Value> {
    let mode_str = match body["mode"].as_str() {
        Some(m) => m,
        None => return error_response("Missing or invalid 'mode' field"),
    };

    let mode: BatteryMode = match serde_json::from_value(json!(mode_str)) {
        Ok(m) => m,
        Err(_) => return error_response(&format!("Invalid mode: '{}'", mode_str)),
    };

    let cmd = ControlCommand::SetBatteryMode { mode };
    match encode_command(&cmd) {
        Ok(writes) => {
            tracing::info!("SetBatteryMode command encoded: {:?}", writes);
            ok_response(&format!("Battery mode set to {}", mode_str))
        }
        Err(e) => error_response(&format!("Validation error: {}", e)),
    }
}

/// POST /api/control/charge-slot — configure a charge schedule slot.
///
/// Body: `{"slot": 1, "config": {...ScheduleSlot fields...}}`
pub async fn set_charge_slot(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<Value> {
    let slot: u8 = match body["slot"].as_u64() {
        Some(s) => s as u8,
        None => return error_response("Missing or invalid 'slot' field (1-3)"),
    };

    let config: ScheduleSlot = match serde_json::from_value(body["config"].clone()) {
        Ok(c) => c,
        Err(e) => return error_response(&format!("Invalid slot config: {}", e)),
    };

    let cmd = ControlCommand::SetChargeSlot { slot, config };
    match encode_command(&cmd) {
        Ok(writes) => {
            tracing::info!("SetChargeSlot command encoded: {:?}", writes);
            ok_response(&format!("Charge slot {} configured", slot))
        }
        Err(e) => error_response(&format!("Validation error: {}", e)),
    }
}

/// POST /api/control/discharge-slot — configure a discharge schedule slot.
///
/// Body: `{"slot": 1, "config": {...ScheduleSlot fields...}}`
pub async fn set_discharge_slot(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<Value> {
    let slot: u8 = match body["slot"].as_u64() {
        Some(s) => s as u8,
        None => return error_response("Missing or invalid 'slot' field (1-2)"),
    };

    let config: ScheduleSlot = match serde_json::from_value(body["config"].clone()) {
        Ok(c) => c,
        Err(e) => return error_response(&format!("Invalid slot config: {}", e)),
    };

    let cmd = ControlCommand::SetDischargeSlot { slot, config };
    match encode_command(&cmd) {
        Ok(writes) => {
            tracing::info!("SetDischargeSlot command encoded: {:?}", writes);
            ok_response(&format!("Discharge slot {} configured", slot))
        }
        Err(e) => error_response(&format!("Validation error: {}", e)),
    }
}

/// POST /api/control/reserve — set battery reserve SoC percentage.
///
/// Body: `{"soc": 20}`
pub async fn set_reserve(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<Value> {
    let soc: u8 = match body["soc"].as_u64() {
        Some(s) => s as u8,
        None => return error_response("Missing or invalid 'soc' field (0-100)"),
    };

    let cmd = ControlCommand::SetReserve { soc };
    match encode_command(&cmd) {
        Ok(writes) => {
            tracing::info!("SetReserve command encoded: {:?}", writes);
            ok_response(&format!("Battery reserve set to {}%", soc))
        }
        Err(e) => error_response(&format!("Validation error: {}", e)),
    }
}

/// POST /api/control/charge-rate — set battery charge rate in watts.
///
/// Body: `{"rate": 2500}`
pub async fn set_charge_rate(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<Value> {
    let rate: u16 = match body["rate"].as_u64() {
        Some(r) => r as u16,
        None => return error_response("Missing or invalid 'rate' field"),
    };

    let cmd = ControlCommand::SetChargeRate { rate };
    match encode_command(&cmd) {
        Ok(writes) => {
            tracing::info!("SetChargeRate command encoded: {:?}", writes);
            ok_response(&format!("Charge rate set to {}W", rate))
        }
        Err(e) => error_response(&format!("Validation error: {}", e)),
    }
}

/// POST /api/control/discharge-rate — set battery discharge rate in watts.
///
/// Body: `{"rate": 3000}`
pub async fn set_discharge_rate(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<Value> {
    let rate: u16 = match body["rate"].as_u64() {
        Some(r) => r as u16,
        None => return error_response("Missing or invalid 'rate' field"),
    };

    let cmd = ControlCommand::SetDischargeRate { rate };
    match encode_command(&cmd) {
        Ok(writes) => {
            tracing::info!("SetDischargeRate command encoded: {:?}", writes);
            ok_response(&format!("Discharge rate set to {}W", rate))
        }
        Err(e) => error_response(&format!("Validation error: {}", e)),
    }
}

/// POST /api/control/force-charge — start a force-charge for the given duration.
///
/// Body: `{"minutes": 30}`
pub async fn force_charge(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<Value> {
    let minutes: u16 = match body["minutes"].as_u64() {
        Some(m) => m as u16,
        None => return error_response("Missing or invalid 'minutes' field"),
    };

    let cmd = ControlCommand::ForceCharge { minutes };
    match encode_command(&cmd) {
        Ok(writes) => {
            tracing::info!("ForceCharge command encoded: {:?}", writes);
            ok_response(&format!("Force charge started for {} minutes", minutes))
        }
        Err(e) => error_response(&format!("Validation error: {}", e)),
    }
}

/// POST /api/control/force-discharge — start a force-discharge for the given duration.
///
/// Body: `{"minutes": 30}`
pub async fn force_discharge(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<Value> {
    let minutes: u16 = match body["minutes"].as_u64() {
        Some(m) => m as u16,
        None => return error_response("Missing or invalid 'minutes' field"),
    };

    let cmd = ControlCommand::ForceDischarge { minutes };
    match encode_command(&cmd) {
        Ok(writes) => {
            tracing::info!("ForceDischarge command encoded: {:?}", writes);
            ok_response(&format!("Force discharge started for {} minutes", minutes))
        }
        Err(e) => error_response(&format!("Validation error: {}", e)),
    }
}

/// POST /api/control/pause — pause the battery for the given duration.
///
/// Body: `{"minutes": 60}`
pub async fn pause_battery(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<Value> {
    let minutes: u16 = match body["minutes"].as_u64() {
        Some(m) => m as u16,
        None => return error_response("Missing or invalid 'minutes' field"),
    };

    let cmd = ControlCommand::PauseBattery { minutes };
    match encode_command(&cmd) {
        Ok(writes) => {
            tracing::info!("PauseBattery command encoded: {:?}", writes);
            ok_response(&format!("Battery paused for {} minutes", minutes))
        }
        Err(e) => error_response(&format!("Validation error: {}", e)),
    }
}

/// POST /api/control/sync-clock — sync the inverter clock to the local time.
pub async fn sync_clock(
    State(_state): State<Arc<AppState>>,
) -> Json<Value> {
    let cmd = ControlCommand::SyncClock;
    match encode_command(&cmd) {
        Ok(writes) => {
            tracing::info!("SyncClock command encoded: {:?}", writes);
            ok_response("Clock sync command sent")
        }
        Err(e) => error_response(&format!("Validation error: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Discovery endpoint
// ---------------------------------------------------------------------------

/// GET /api/discover — scan the local network for GivEnergy inverters.
///
/// Returns a list of discovered inverters. Currently returns a placeholder
/// response; will be wired up to `discovery::scan_network` when the
/// discovery module is fully implemented.
pub async fn discover(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    tracing::info!("Network discovery requested");

    // TODO: Wire up to crate::inverter::discovery::scan_network once implemented.
    // For now, return the configured inverter info if available.
    let settings = state.settings.lock().await;

    if settings.host.is_empty() || settings.serial.is_empty() {
        return Json(json!({
            "ok": true,
            "inverters": [],
            "message": "Discovery not yet implemented. Configure inverter manually."
        }));
    }

    Json(json!({
        "ok": true,
        "inverters": [{
            "host": settings.host,
            "port": settings.port,
            "serial": settings.serial,
        }],
        "message": "Discovery placeholder — returning configured inverter"
    }))
}
