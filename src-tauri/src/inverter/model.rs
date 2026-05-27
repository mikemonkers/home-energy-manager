//! Inverter data model.
//!
//! Defines the Rust structs representing inverter state —
//! battery, PV, grid, and system-level data — with Serde
//! serialization for the frontend and WebSocket clients.

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Battery charge / discharge state derived from the battery power reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryState {
    /// Battery is neither charging nor discharging.
    Idle,
    /// Battery is actively charging (power > 0).
    Charging,
    /// Battery is actively discharging (power < 0).
    Discharging,
    /// Battery is paused (manually or by schedule).
    Paused,
}

impl BatteryState {
    /// Derive the battery state from the signed battery power value.
    ///
    /// Positive power means charging, negative means discharging, zero is idle.
    pub fn from_power(power: i32) -> Self {
        if power > 0 {
            Self::Charging
        } else if power < 0 {
            Self::Discharging
        } else {
            Self::Idle
        }
    }
}

/// Battery operating mode, read from holding register `HR_BATTERY_MODE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryMode {
    /// Battery is paused.
    Paused,
    /// Eco mode – battery discharges to meet home demand.
    Eco,
    /// Timed demand – discharge schedule controls battery.
    TimedDemand,
    /// Timed export – discharge schedule forces export to grid.
    TimedExport,
}

impl BatteryMode {
    /// Map a raw holding-register value to a [`BatteryMode`].
    ///
    /// GivEnergy encoding: 0 = paused, 1 = eco, 2 = timed demand, 3 = timed export.
    pub fn from_register(val: u16) -> Self {
        match val {
            0 => Self::Paused,
            1 => Self::Eco,
            2 => Self::TimedDemand,
            3 => Self::TimedExport,
            _ => Self::Paused,
        }
    }

    /// Convert this mode back to the raw holding-register value.
    pub fn to_register(self) -> u16 {
        match self {
            Self::Paused => 0,
            Self::Eco => 1,
            Self::TimedDemand => 2,
            Self::TimedExport => 3,
        }
    }
}

/// Inverter hardware variant, read from holding register `HR_DEVICE_TYPE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DeviceType {
    Gen2Hybrid,
    Gen3Hybrid,
    ACCoupled,
    AllInOne,
    ThreePhase,
    Unknown(u16),
}

impl DeviceType {
    /// Map a raw holding-register value to a [`DeviceType`].
    ///
    /// Known GivEnergy device-type codes are mapped to named variants;
    /// any unrecognised value is wrapped in [`DeviceType::Unknown`].
    pub fn from_register(val: u16) -> Self {
        match val {
            1 => Self::Gen2Hybrid,
            2 => Self::Gen3Hybrid,
            3 => Self::ACCoupled,
            4 => Self::AllInOne,
            5 => Self::ThreePhase,
            other => Self::Unknown(other),
        }
    }
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// A single charge or discharge schedule slot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScheduleSlot {
    /// Whether the slot is active.
    pub enabled: bool,
    /// Start hour (0–23).
    pub start_hour: u8,
    /// Start minute (0–59).
    pub start_minute: u8,
    /// End hour (0–23).
    pub end_hour: u8,
    /// End minute (0–59).
    pub end_minute: u8,
    /// Target state-of-charge percentage for this slot (0–100).
    pub target_soc: u8,
}

impl Default for ScheduleSlot {
    fn default() -> Self {
        Self {
            enabled: false,
            start_hour: 0,
            start_minute: 0,
            end_hour: 0,
            end_minute: 0,
            target_soc: 0,
        }
    }
}

/// Per-module battery data (available on some inverter models).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BatteryModule {
    /// Module index (0-based).
    pub index: u8,
    /// Module state of charge (0–100 %).
    pub soc: u8,
    /// Module temperature in °C.
    pub temperature: f32,
    /// Module voltage in volts.
    pub voltage: f32,
    /// Module current in amps (signed – negative = discharging).
    pub current: f32,
}

/// Complete snapshot of inverter state from a single poll cycle.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InverterSnapshot {
    // -- Timestamp --
    /// Unix-epoch timestamp (seconds) when this snapshot was captured.
    pub timestamp: i64,

    // -- Solar / PV --
    /// Total solar power in watts.
    pub solar_power: i32,
    /// PV1 string power in watts.
    pub pv1_power: i32,
    /// PV2 string power in watts.
    pub pv2_power: i32,
    /// PV1 string voltage in volts.
    pub pv1_voltage: f32,
    /// PV2 string voltage in volts.
    pub pv2_voltage: f32,
    /// PV1 string current in amps.
    pub pv1_current: f32,
    /// PV2 string current in amps.
    pub pv2_current: f32,

    // -- Battery --
    /// Battery power in watts (positive = charging, negative = discharging).
    pub battery_power: i32,
    /// Battery state of charge (0–100 %).
    pub soc: u8,
    /// Battery voltage in volts.
    pub battery_voltage: f32,
    /// Battery current in amps (negative = discharging).
    pub battery_current: f32,
    /// Derived battery state.
    pub battery_state: BatteryState,
    /// Battery temperature in °C.
    pub battery_temperature: f32,
    /// Battery capacity in kWh.
    pub battery_capacity_kwh: f32,

    // -- Grid --
    /// Grid power in watts (positive = importing).
    pub grid_power: i32,
    /// Grid voltage in volts.
    pub grid_voltage: f32,
    /// Grid frequency in hertz.
    pub grid_frequency: f32,

    // -- Home --
    /// Computed home consumption in watts (solar + battery + grid).
    pub home_power: i32,

    // -- Inverter --
    /// Inverter internal temperature in °C.
    pub inverter_temperature: f32,

    // -- Today's energy totals --
    /// Solar energy generated today in kWh.
    pub today_solar_kwh: f32,
    /// Energy imported from the grid today in kWh.
    pub today_import_kwh: f32,
    /// Energy exported to the grid today in kWh.
    pub today_export_kwh: f32,
    /// Energy used to charge the battery today in kWh.
    pub today_charge_kwh: f32,
    /// Energy discharged from the battery today in kWh.
    pub today_discharge_kwh: f32,
    /// Total household consumption today in kWh.
    pub today_consumption_kwh: f32,

    // -- Battery modules --
    /// Per-module battery telemetry (empty if not available).
    pub battery_modules: Vec<BatteryModule>,

    // -- Control state (from holding registers) --
    /// Current battery operating mode.
    pub battery_mode: BatteryMode,
    /// Battery reserve SoC percentage (0–100).
    pub battery_reserve: u8,
    /// Battery charge rate in watts.
    pub charge_rate: u16,
    /// Battery discharge rate in watts.
    pub discharge_rate: u16,
    /// Target SoC for charging (0–100 %).
    pub target_soc: u8,

    // -- Charge / discharge schedule slots --
    /// Charge schedule slots (up to 3).
    pub charge_slots: [ScheduleSlot; 3],
    /// Discharge schedule slots (up to 2).
    pub discharge_slots: [ScheduleSlot; 2],

    // -- Device info --
    /// Inverter serial number.
    pub inverter_serial: String,
    /// Firmware version string.
    pub firmware_version: String,
    /// Detected device type.
    pub device_type: DeviceType,
}

impl Default for InverterSnapshot {
    fn default() -> Self {
        Self {
            timestamp: 0,
            solar_power: 0,
            pv1_power: 0,
            pv2_power: 0,
            pv1_voltage: 0.0,
            pv2_voltage: 0.0,
            pv1_current: 0.0,
            pv2_current: 0.0,
            battery_power: 0,
            soc: 0,
            battery_voltage: 0.0,
            battery_current: 0.0,
            battery_state: BatteryState::Idle,
            battery_temperature: 0.0,
            battery_capacity_kwh: 0.0,
            grid_power: 0,
            grid_voltage: 0.0,
            grid_frequency: 0.0,
            home_power: 0,
            inverter_temperature: 0.0,
            today_solar_kwh: 0.0,
            today_import_kwh: 0.0,
            today_export_kwh: 0.0,
            today_charge_kwh: 0.0,
            today_discharge_kwh: 0.0,
            today_consumption_kwh: 0.0,
            battery_modules: Vec::new(),
            battery_mode: BatteryMode::Paused,
            battery_reserve: 0,
            charge_rate: 0,
            discharge_rate: 0,
            target_soc: 0,
            charge_slots: [
                ScheduleSlot::default(),
                ScheduleSlot::default(),
                ScheduleSlot::default(),
            ],
            discharge_slots: [ScheduleSlot::default(), ScheduleSlot::default()],
            inverter_serial: String::new(),
            firmware_version: String::new(),
            device_type: DeviceType::Unknown(0),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- BatteryState::from_power -------------------------------------------

    #[test]
    fn battery_state_from_positive_power() {
        assert_eq!(BatteryState::from_power(1500), BatteryState::Charging);
    }

    #[test]
    fn battery_state_from_negative_power() {
        assert_eq!(BatteryState::from_power(-800), BatteryState::Discharging);
    }

    #[test]
    fn battery_state_from_zero_power() {
        assert_eq!(BatteryState::from_power(0), BatteryState::Idle);
    }

    // -- BatteryMode::from_register -----------------------------------------

    #[test]
    fn battery_mode_from_register_known_values() {
        assert_eq!(BatteryMode::from_register(0), BatteryMode::Paused);
        assert_eq!(BatteryMode::from_register(1), BatteryMode::Eco);
        assert_eq!(BatteryMode::from_register(2), BatteryMode::TimedDemand);
        assert_eq!(BatteryMode::from_register(3), BatteryMode::TimedExport);
    }

    #[test]
    fn battery_mode_from_register_unknown_falls_back_to_paused() {
        assert_eq!(BatteryMode::from_register(99), BatteryMode::Paused);
    }

    // -- DeviceType::from_register ------------------------------------------

    #[test]
    fn device_type_from_register_known_values() {
        assert_eq!(DeviceType::from_register(1), DeviceType::Gen2Hybrid);
        assert_eq!(DeviceType::from_register(2), DeviceType::Gen3Hybrid);
        assert_eq!(DeviceType::from_register(3), DeviceType::ACCoupled);
        assert_eq!(DeviceType::from_register(4), DeviceType::AllInOne);
        assert_eq!(DeviceType::from_register(5), DeviceType::ThreePhase);
    }

    #[test]
    fn device_type_from_register_unknown() {
        assert_eq!(DeviceType::from_register(42), DeviceType::Unknown(42));
    }

    // -- ScheduleSlot::default ----------------------------------------------

    #[test]
    fn schedule_slot_default_is_disabled_with_zeros() {
        let slot = ScheduleSlot::default();
        assert!(!slot.enabled);
        assert_eq!(slot.start_hour, 0);
        assert_eq!(slot.start_minute, 0);
        assert_eq!(slot.end_hour, 0);
        assert_eq!(slot.end_minute, 0);
        assert_eq!(slot.target_soc, 0);
    }

    // -- InverterSnapshot::default ------------------------------------------

    #[test]
    fn inverter_snapshot_default_has_idle_battery() {
        let snap = InverterSnapshot::default();
        assert_eq!(snap.timestamp, 0);
        assert_eq!(snap.battery_state, BatteryState::Idle);
        assert_eq!(snap.battery_mode, BatteryMode::Paused);
        assert_eq!(snap.solar_power, 0);
        assert_eq!(snap.pv1_voltage, 0.0);
        assert_eq!(snap.grid_frequency, 0.0);
        assert_eq!(snap.home_power, 0);
        assert!(snap.battery_modules.is_empty());
        assert_eq!(snap.charge_slots.len(), 3);
        assert_eq!(snap.discharge_slots.len(), 2);
        assert!(snap.inverter_serial.is_empty());
        assert!(snap.firmware_version.is_empty());
        assert_eq!(snap.device_type, DeviceType::Unknown(0));
    }

    #[test]
    fn inverter_snapshot_default_charge_slots_all_disabled() {
        let snap = InverterSnapshot::default();
        for slot in &snap.charge_slots {
            assert!(!slot.enabled);
        }
    }

    #[test]
    fn inverter_snapshot_default_discharge_slots_all_disabled() {
        let snap = InverterSnapshot::default();
        for slot in &snap.discharge_slots {
            assert!(!slot.enabled);
        }
    }

    // -- Serde round-trip ---------------------------------------------------

    #[test]
    fn battery_state_serde_roundtrip() {
        let states = [
            BatteryState::Idle,
            BatteryState::Charging,
            BatteryState::Discharging,
            BatteryState::Paused,
        ];
        for state in &states {
            let json = serde_json::to_string(state).unwrap();
            let deserialized: BatteryState = serde_json::from_str(&json).unwrap();
            assert_eq!(*state, deserialized, "roundtrip failed for {:?}", state);
        }
    }

    #[test]
    fn battery_mode_serde_roundtrip() {
        let modes = [
            BatteryMode::Paused,
            BatteryMode::Eco,
            BatteryMode::TimedDemand,
            BatteryMode::TimedExport,
        ];
        for mode in &modes {
            let json = serde_json::to_string(mode).unwrap();
            let deserialized: BatteryMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, deserialized, "roundtrip failed for {:?}", mode);
        }
    }

    #[test]
    fn device_type_serde_roundtrip() {
        let variants = [
            DeviceType::Gen2Hybrid,
            DeviceType::Gen3Hybrid,
            DeviceType::ACCoupled,
            DeviceType::AllInOne,
            DeviceType::ThreePhase,
            DeviceType::Unknown(99),
        ];
        for dt in &variants {
            let json = serde_json::to_string(dt).unwrap();
            let deserialized: DeviceType = serde_json::from_str(&json).unwrap();
            assert_eq!(*dt, deserialized, "roundtrip failed for {:?}", dt);
        }
    }

    #[test]
    fn inverter_snapshot_serde_roundtrip() {
        let snap = InverterSnapshot::default();
        let json = serde_json::to_string(&snap).unwrap();
        let deserialized: InverterSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap.timestamp, deserialized.timestamp);
        assert_eq!(snap.battery_state, deserialized.battery_state);
        assert_eq!(snap.battery_mode, deserialized.battery_mode);
        assert_eq!(snap.charge_slots.len(), deserialized.charge_slots.len());
        assert_eq!(
            snap.discharge_slots.len(),
            deserialized.discharge_slots.len()
        );
    }
}
