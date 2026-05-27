//! Register-to-model decoder.
//!
//! Translates raw Modbus register values into the typed inverter
//! model structs, applying scaling factors and unit conversions.

use crate::modbus::client::BlockRead;
use crate::modbus::registers::{
    RegisterType, HR_BATTERY_CHARGE_RATE, HR_BATTERY_DISCHARGE_RATE, HR_BATTERY_MODE,
    HR_BATTERY_RESERVE_SOC, HR_CHARGE_SLOT_1_START_H, HR_CHARGE_SLOT_2_START_H,
    HR_CHARGE_SLOT_3_START_H, HR_DEVICE_TYPE, HR_DISCHARGE_SLOT_1_START_H,
    HR_DISCHARGE_SLOT_2_START_H, HR_TARGET_SOC, IR_BATTERY_CURRENT, IR_BATTERY_POWER,
    IR_BATTERY_SOC, IR_BATTERY_TEMPERATURE, IR_BATTERY_VOLTAGE, IR_GRID_FREQUENCY,
    IR_GRID_POWER, IR_GRID_VOLTAGE, IR_INVERTER_TEMPERATURE, IR_PV1_CURRENT, IR_PV1_POWER,
    IR_PV1_VOLTAGE, IR_PV2_CURRENT, IR_PV2_POWER, IR_PV2_VOLTAGE, IR_TODAY_CHARGE_ENERGY,
    IR_TODAY_CONSUMPTION, IR_TODAY_DISCHARGE_ENERGY, IR_TODAY_EXPORT_ENERGY,
    IR_TODAY_IMPORT_ENERGY, IR_TODAY_SOLAR_ENERGY,
};

use super::model::{BatteryMode, BatteryState, DeviceType, InverterSnapshot, ScheduleSlot};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Safely retrieve a register value by index, returning 0 if out of bounds.
fn get_reg(data: &[u16], index: usize) -> u16 {
    data.get(index).copied().unwrap_or(0)
}

/// Interpret a u16 register value as a signed i16, then widen to i32.
fn signed(raw: u16) -> i32 {
    raw as i16 as i32
}

// ---------------------------------------------------------------------------
// Block identification
// ---------------------------------------------------------------------------

/// Identifies which block we are processing based on its register type
/// and start address, returning a normalized string key.
fn block_key(block: &crate::modbus::registers::RegisterBlock) -> &'static str {
    match (block.register_type, block.start) {
        (RegisterType::Input, 0) => "input_0_59",
        (RegisterType::Input, 180) => "input_180_239",
        (RegisterType::Holding, 0) => "holding_0_59",
        (RegisterType::Holding, 60) => "holding_60_119",
        (RegisterType::Holding, 240) => "gen3_holding_240_359",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// Slot decoding
// ---------------------------------------------------------------------------

/// Decode a single schedule slot from 6 consecutive register values.
///
/// Layout: [start_h, start_m, end_h, end_m, target_soc, enable]
fn decode_slot(data: &[u16], base_index: usize) -> ScheduleSlot {
    ScheduleSlot {
        enabled: get_reg(data, base_index + 5) != 0,
        start_hour: get_reg(data, base_index) as u8,
        start_minute: get_reg(data, base_index + 1) as u8,
        end_hour: get_reg(data, base_index + 2) as u8,
        end_minute: get_reg(data, base_index + 3) as u8,
        target_soc: get_reg(data, base_index + 4) as u8,
    }
}

// ---------------------------------------------------------------------------
// Main decoder
// ---------------------------------------------------------------------------

/// Decode raw register blocks into an InverterSnapshot.
///
/// Each `BlockRead` contains a block descriptor and a `Vec<u16>` of raw
/// register values. The decoder identifies each block by its register type
/// and start address, extracts values using the register address constants,
/// applies scaling factors, and assembles a complete snapshot.
pub fn decode_snapshot(blocks: &[BlockRead]) -> InverterSnapshot {
    let mut snap = InverterSnapshot::default();
    snap.timestamp = chrono::Utc::now().timestamp();

    for br in blocks {
        let key = block_key(br.block);
        let data = &br.data;

        match key {
            "input_0_59" => decode_input_0_59(data, &mut snap),
            "input_180_239" => {
                // Currently no registers defined in this block; reserved for future use.
            }
            "holding_0_59" => decode_holding_0_59(data, &mut snap),
            "holding_60_119" => decode_holding_60_119(data, &mut snap),
            "gen3_holding_240_359" => {
                // Gen3 extended block — reserved for future use.
            }
            _ => {
                log::warn!("Unknown block '{}' in decode_snapshot", key);
            }
        }
    }

    // Compute home power: solar + battery + grid
    snap.home_power = snap.solar_power + snap.battery_power + snap.grid_power;

    snap
}

// ---------------------------------------------------------------------------
// Per-block decoders
// ---------------------------------------------------------------------------

/// Decode input registers 0–59 (telemetry).
fn decode_input_0_59(data: &[u16], snap: &mut InverterSnapshot) {
    // PV
    snap.pv1_power = get_reg(data, IR_PV1_POWER as usize) as i32;
    snap.pv2_power = get_reg(data, IR_PV2_POWER as usize) as i32;
    snap.solar_power = snap.pv1_power + snap.pv2_power;
    snap.pv1_voltage = get_reg(data, IR_PV1_VOLTAGE as usize) as f32 * 0.1;
    snap.pv2_voltage = get_reg(data, IR_PV2_VOLTAGE as usize) as f32 * 0.1;
    snap.pv1_current = get_reg(data, IR_PV1_CURRENT as usize) as f32 * 0.1;
    snap.pv2_current = get_reg(data, IR_PV2_CURRENT as usize) as f32 * 0.1;

    // Battery
    snap.battery_power = signed(get_reg(data, IR_BATTERY_POWER as usize));
    snap.soc = get_reg(data, IR_BATTERY_SOC as usize) as u8;
    snap.battery_voltage = get_reg(data, IR_BATTERY_VOLTAGE as usize) as f32 * 0.1;
    snap.battery_current = signed(get_reg(data, IR_BATTERY_CURRENT as usize)) as f32 * 0.1;
    snap.battery_state = BatteryState::from_power(snap.battery_power);
    snap.battery_temperature = get_reg(data, IR_BATTERY_TEMPERATURE as usize) as f32 * 0.1;

    // Grid
    snap.grid_power = signed(get_reg(data, IR_GRID_POWER as usize));
    snap.grid_voltage = get_reg(data, IR_GRID_VOLTAGE as usize) as f32 * 0.1;
    snap.grid_frequency = get_reg(data, IR_GRID_FREQUENCY as usize) as f32 * 0.01;

    // Inverter
    snap.inverter_temperature = get_reg(data, IR_INVERTER_TEMPERATURE as usize) as f32 * 0.1;

    // Energy totals (0.1 kWh units)
    snap.today_solar_kwh = get_reg(data, IR_TODAY_SOLAR_ENERGY as usize) as f32 * 0.1;
    snap.today_import_kwh = get_reg(data, IR_TODAY_IMPORT_ENERGY as usize) as f32 * 0.1;
    snap.today_export_kwh = get_reg(data, IR_TODAY_EXPORT_ENERGY as usize) as f32 * 0.1;
    snap.today_charge_kwh = get_reg(data, IR_TODAY_CHARGE_ENERGY as usize) as f32 * 0.1;
    snap.today_discharge_kwh = get_reg(data, IR_TODAY_DISCHARGE_ENERGY as usize) as f32 * 0.1;
    snap.today_consumption_kwh = get_reg(data, IR_TODAY_CONSUMPTION as usize) as f32 * 0.1;
}

/// Decode holding registers 0–59 (control state & schedules).
fn decode_holding_0_59(data: &[u16], snap: &mut InverterSnapshot) {
    snap.device_type = DeviceType::from_register(get_reg(data, HR_DEVICE_TYPE as usize));
    snap.battery_mode = BatteryMode::from_register(get_reg(data, HR_BATTERY_MODE as usize));
    snap.battery_reserve = get_reg(data, HR_BATTERY_RESERVE_SOC as usize) as u8;
    snap.charge_rate = get_reg(data, HR_BATTERY_CHARGE_RATE as usize);
    snap.discharge_rate = get_reg(data, HR_BATTERY_DISCHARGE_RATE as usize);

    // Charge slots (each is 6 registers)
    snap.charge_slots[0] = decode_slot(data, HR_CHARGE_SLOT_1_START_H as usize);
    snap.charge_slots[1] = decode_slot(data, HR_CHARGE_SLOT_2_START_H as usize);
    snap.charge_slots[2] = decode_slot(data, HR_CHARGE_SLOT_3_START_H as usize);

    // Discharge slots (each is 6 registers)
    snap.discharge_slots[0] = decode_slot(data, HR_DISCHARGE_SLOT_1_START_H as usize);
    snap.discharge_slots[1] = decode_slot(data, HR_DISCHARGE_SLOT_2_START_H as usize);
}

/// Decode holding registers 60–119 (additional configuration).
fn decode_holding_60_119(data: &[u16], snap: &mut InverterSnapshot) {
    // Register addresses in this block are offset by 60.
    // HR_TARGET_SOC = 60, so index = 60 - 60 = 0.
    snap.target_soc = get_reg(data, (HR_TARGET_SOC - 60) as usize) as u8;
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modbus::registers::{
        RegisterBlock, RegisterType, HR_CHARGE_SLOT_1_ENABLE, HR_CHARGE_SLOT_1_END_H,
        HR_CHARGE_SLOT_1_END_M, HR_CHARGE_SLOT_1_START_H, HR_CHARGE_SLOT_1_START_M,
        HR_CHARGE_SLOT_1_TARGET_SOC, HR_CHARGE_SLOT_2_ENABLE, HR_CHARGE_SLOT_3_ENABLE,
        HR_DISCHARGE_SLOT_1_ENABLE, HR_DISCHARGE_SLOT_1_END_H, HR_DISCHARGE_SLOT_1_END_M,
        HR_DISCHARGE_SLOT_1_START_M, HR_DISCHARGE_SLOT_1_TARGET_SOC, HR_DISCHARGE_SLOT_2_ENABLE,
    };

    /// Helper to build a `BlockRead` for testing.
    fn make_block(
        register_type: RegisterType,
        start: u16,
        count: u16,
        name: &'static str,
        data: Vec<u16>,
    ) -> BlockRead {
        // We need a static reference. In tests we leak a Box to get a &'static.
        // This is fine for tests only.
        let block = Box::leak(Box::new(RegisterBlock {
            start,
            count,
            register_type,
            name,
        }));
        BlockRead { block, data }
    }

    /// Build the standard set of blocks with known test data.
    fn test_blocks() -> Vec<BlockRead> {
        // Input registers 0–59
        let mut input_data = vec![0u16; 60];
        input_data[IR_PV1_POWER as usize] = 2500; // 2500 W
        input_data[IR_PV2_POWER as usize] = 1500; // 1500 W
        input_data[IR_PV1_VOLTAGE as usize] = 320; // 32.0 V  (320 * 0.1)
        input_data[IR_PV2_VOLTAGE as usize] = 315; // 31.5 V
        input_data[IR_PV1_CURRENT as usize] = 78; // 7.8 A  (78 * 0.1)
        input_data[IR_PV2_CURRENT as usize] = 48; // 4.8 A
        input_data[IR_BATTERY_POWER as usize] = 800; // 800 W charging
        input_data[IR_BATTERY_SOC as usize] = 75; // 75 %
        input_data[IR_BATTERY_VOLTAGE as usize] = 520; // 52.0 V
        input_data[IR_BATTERY_CURRENT as usize] = 15; // 1.5 A (positive = charging)
        input_data[IR_GRID_POWER as usize] = 100; // 100 W importing
        input_data[IR_GRID_VOLTAGE as usize] = 241; // 24.1 V... no, 241 * 0.1 = 241.0 ... actually let's use 2410 => 241.0 V
        input_data[IR_GRID_FREQUENCY as usize] = 5002; // 50.02 Hz (5002 * 0.01)
        input_data[IR_INVERTER_TEMPERATURE as usize] = 425; // 42.5 °C
        input_data[IR_BATTERY_TEMPERATURE as usize] = 310; // 31.0 °C
        input_data[IR_TODAY_SOLAR_ENERGY as usize] = 185; // 18.5 kWh
        input_data[IR_TODAY_IMPORT_ENERGY as usize] = 52; // 5.2 kWh
        input_data[IR_TODAY_EXPORT_ENERGY as usize] = 30; // 3.0 kWh
        input_data[IR_TODAY_CHARGE_ENERGY as usize] = 40; // 4.0 kWh
        input_data[IR_TODAY_DISCHARGE_ENERGY as usize] = 25; // 2.5 kWh
        input_data[IR_TODAY_CONSUMPTION as usize] = 120; // 12.0 kWh

        let input_block = make_block(
            RegisterType::Input,
            0,
            60,
            "input_0_59",
            input_data,
        );

        // Input registers 180–239 (empty)
        let input_180 = make_block(
            RegisterType::Input,
            180,
            60,
            "input_180_239",
            vec![0; 60],
        );

        // Holding registers 0–59
        let mut holding_data = vec![0u16; 60];
        holding_data[HR_DEVICE_TYPE as usize] = 2; // Gen3Hybrid
        holding_data[HR_BATTERY_MODE as usize] = 1; // Eco
        holding_data[HR_BATTERY_RESERVE_SOC as usize] = 10; // 10 %
        holding_data[HR_BATTERY_CHARGE_RATE as usize] = 2500;
        holding_data[HR_BATTERY_DISCHARGE_RATE as usize] = 2500;

        // Charge slot 1 (index 5-10): 06:00 to 10:00, SOC 100, enabled
        holding_data[HR_CHARGE_SLOT_1_START_H as usize] = 6;
        holding_data[HR_CHARGE_SLOT_1_START_M as usize] = 0;
        holding_data[HR_CHARGE_SLOT_1_END_H as usize] = 10;
        holding_data[HR_CHARGE_SLOT_1_END_M as usize] = 0;
        holding_data[HR_CHARGE_SLOT_1_TARGET_SOC as usize] = 100;
        holding_data[HR_CHARGE_SLOT_1_ENABLE as usize] = 1;

        // Charge slot 2 (index 11-16): disabled
        holding_data[HR_CHARGE_SLOT_2_ENABLE as usize] = 0;

        // Charge slot 3 (index 17-22): disabled
        holding_data[HR_CHARGE_SLOT_3_ENABLE as usize] = 0;

        // Discharge slot 1 (index 23-28): 16:00 to 19:30, SOC 20, enabled
        holding_data[HR_DISCHARGE_SLOT_1_START_H as usize] = 16;
        holding_data[HR_DISCHARGE_SLOT_1_START_M as usize] = 0;
        holding_data[HR_DISCHARGE_SLOT_1_END_H as usize] = 19;
        holding_data[HR_DISCHARGE_SLOT_1_END_M as usize] = 30;
        holding_data[HR_DISCHARGE_SLOT_1_TARGET_SOC as usize] = 20;
        holding_data[HR_DISCHARGE_SLOT_1_ENABLE as usize] = 1;

        // Discharge slot 2 (index 29-34): disabled
        holding_data[HR_DISCHARGE_SLOT_2_ENABLE as usize] = 0;

        let holding_block = make_block(
            RegisterType::Holding,
            0,
            60,
            "holding_0_59",
            holding_data,
        );

        // Holding registers 60–119
        let mut holding_60_data = vec![0u16; 60];
        holding_60_data[(HR_TARGET_SOC - 60) as usize] = 100; // target SOC 100 %

        let holding_60 = make_block(
            RegisterType::Holding,
            60,
            60,
            "holding_60_119",
            holding_60_data,
        );

        vec![input_block, input_180, holding_block, holding_60]
    }

    // -----------------------------------------------------------------------
    // Full decode test
    // -----------------------------------------------------------------------

    #[test]
    fn decode_snapshot_full() {
        let blocks = test_blocks();
        let snap = decode_snapshot(&blocks);

        // PV
        assert_eq!(snap.pv1_power, 2500);
        assert_eq!(snap.pv2_power, 1500);
        assert_eq!(snap.solar_power, 4000);
        assert!((snap.pv1_voltage - 32.0).abs() < f32::EPSILON);
        assert!((snap.pv2_voltage - 31.5).abs() < f32::EPSILON);
        assert!((snap.pv1_current - 7.8).abs() < 0.01);
        assert!((snap.pv2_current - 4.8).abs() < 0.01);

        // Battery
        assert_eq!(snap.battery_power, 800);
        assert_eq!(snap.soc, 75);
        assert!((snap.battery_voltage - 52.0).abs() < f32::EPSILON);
        assert!((snap.battery_current - 1.5).abs() < 0.01);
        assert_eq!(snap.battery_state, BatteryState::Charging);
        assert!((snap.battery_temperature - 31.0).abs() < 0.01);

        // Grid
        assert_eq!(snap.grid_power, 100);
        assert!((snap.grid_voltage - 24.1).abs() < 0.01);
        assert!((snap.grid_frequency - 50.02).abs() < 0.001);

        // Inverter
        assert!((snap.inverter_temperature - 42.5).abs() < 0.01);

        // Energy totals
        assert!((snap.today_solar_kwh - 18.5).abs() < 0.01);
        assert!((snap.today_import_kwh - 5.2).abs() < 0.01);
        assert!((snap.today_export_kwh - 3.0).abs() < f32::EPSILON);
        assert!((snap.today_charge_kwh - 4.0).abs() < f32::EPSILON);
        assert!((snap.today_discharge_kwh - 2.5).abs() < 0.01);
        assert!((snap.today_consumption_kwh - 12.0).abs() < 0.01);

        // Holding registers
        assert_eq!(snap.battery_mode, BatteryMode::Eco);
        assert_eq!(snap.device_type, DeviceType::Gen3Hybrid);
        assert_eq!(snap.battery_reserve, 10);
        assert_eq!(snap.charge_rate, 2500);
        assert_eq!(snap.discharge_rate, 2500);
        assert_eq!(snap.target_soc, 100);

        // Charge slot 1
        assert!(snap.charge_slots[0].enabled);
        assert_eq!(snap.charge_slots[0].start_hour, 6);
        assert_eq!(snap.charge_slots[0].start_minute, 0);
        assert_eq!(snap.charge_slots[0].end_hour, 10);
        assert_eq!(snap.charge_slots[0].end_minute, 0);
        assert_eq!(snap.charge_slots[0].target_soc, 100);

        // Charge slots 2 & 3 disabled
        assert!(!snap.charge_slots[1].enabled);
        assert!(!snap.charge_slots[2].enabled);

        // Discharge slot 1
        assert!(snap.discharge_slots[0].enabled);
        assert_eq!(snap.discharge_slots[0].start_hour, 16);
        assert_eq!(snap.discharge_slots[0].start_minute, 0);
        assert_eq!(snap.discharge_slots[0].end_hour, 19);
        assert_eq!(snap.discharge_slots[0].end_minute, 30);
        assert_eq!(snap.discharge_slots[0].target_soc, 20);

        // Discharge slot 2 disabled
        assert!(!snap.discharge_slots[1].enabled);

        // Home power = solar + battery + grid = 4000 + 800 + 100 = 4900
        assert_eq!(snap.home_power, 4900);

        // Timestamp should be set (not 0)
        assert!(snap.timestamp > 0);
    }

    // -----------------------------------------------------------------------
    // Signed value interpretation
    // -----------------------------------------------------------------------

    #[test]
    fn signed_battery_power_negative() {
        // 0xFF38 = 65336 as u16, which is -200 as i16
        let raw: u16 = -200i16 as u16;
        let mut input_data = vec![0u16; 60];
        input_data[IR_BATTERY_POWER as usize] = raw;

        let blocks = vec![make_block(
            RegisterType::Input,
            0,
            60,
            "input_0_59",
            input_data,
        )];
        let snap = decode_snapshot(&blocks);

        assert_eq!(snap.battery_power, -200);
        assert_eq!(snap.battery_state, BatteryState::Discharging);
    }

    #[test]
    fn signed_grid_power_negative() {
        // -500 W = exporting to grid
        let raw: u16 = -500i16 as u16;
        let mut input_data = vec![0u16; 60];
        input_data[IR_GRID_POWER as usize] = raw;

        let blocks = vec![make_block(
            RegisterType::Input,
            0,
            60,
            "input_0_59",
            input_data,
        )];
        let snap = decode_snapshot(&blocks);

        assert_eq!(snap.grid_power, -500);
    }

    #[test]
    fn signed_battery_current_negative() {
        let raw: u16 = -15i16 as u16; // -1.5 A
        let mut input_data = vec![0u16; 60];
        input_data[IR_BATTERY_CURRENT as usize] = raw;

        let blocks = vec![make_block(
            RegisterType::Input,
            0,
            60,
            "input_0_59",
            input_data,
        )];
        let snap = decode_snapshot(&blocks);

        assert!((snap.battery_current - (-1.5)).abs() < 0.01);
    }

    // -----------------------------------------------------------------------
    // Home power computation
    // -----------------------------------------------------------------------

    #[test]
    fn home_power_computed_correctly() {
        let mut input_data = vec![0u16; 60];
        input_data[IR_PV1_POWER as usize] = 3000; // PV1 = 3000 W
        input_data[IR_PV2_POWER as usize] = 1000; // PV2 = 1000 W => solar = 4000
        input_data[IR_BATTERY_POWER as usize] = -1500i16 as u16; // battery discharging -1500 W
        input_data[IR_GRID_POWER as usize] = 200; // grid importing 200 W

        let blocks = vec![make_block(
            RegisterType::Input,
            0,
            60,
            "input_0_59",
            input_data,
        )];
        let snap = decode_snapshot(&blocks);

        // home = solar(4000) + battery(-1500) + grid(200) = 2700
        assert_eq!(snap.home_power, 2700);
    }

    // -----------------------------------------------------------------------
    // Battery state derivation
    // -----------------------------------------------------------------------

    #[test]
    fn battery_state_charging_when_positive() {
        let mut input_data = vec![0u16; 60];
        input_data[IR_BATTERY_POWER as usize] = 1; // positive

        let blocks = vec![make_block(
            RegisterType::Input,
            0,
            60,
            "input_0_59",
            input_data,
        )];
        let snap = decode_snapshot(&blocks);

        assert_eq!(snap.battery_state, BatteryState::Charging);
    }

    #[test]
    fn battery_state_idle_when_zero() {
        let mut input_data = vec![0u16; 60];
        input_data[IR_BATTERY_POWER as usize] = 0;

        let blocks = vec![make_block(
            RegisterType::Input,
            0,
            60,
            "input_0_59",
            input_data,
        )];
        let snap = decode_snapshot(&blocks);

        assert_eq!(snap.battery_state, BatteryState::Idle);
    }

    #[test]
    fn battery_state_discharging_when_negative() {
        let mut input_data = vec![0u16; 60];
        input_data[IR_BATTERY_POWER as usize] = -1i16 as u16;

        let blocks = vec![make_block(
            RegisterType::Input,
            0,
            60,
            "input_0_59",
            input_data,
        )];
        let snap = decode_snapshot(&blocks);

        assert_eq!(snap.battery_state, BatteryState::Discharging);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn decode_snapshot_with_empty_blocks() {
        let snap = decode_snapshot(&[]);
        // Should still get a valid default snapshot with timestamp set
        assert!(snap.timestamp > 0);
        assert_eq!(snap.solar_power, 0);
        assert_eq!(snap.home_power, 0);
    }

    #[test]
    fn decode_snapshot_with_short_data() {
        // If the data vector is shorter than expected, get_reg returns 0
        let blocks = vec![make_block(
            RegisterType::Input,
            0,
            60,
            "input_0_59",
            vec![2500], // only 1 register instead of 60
        )];
        let snap = decode_snapshot(&blocks);

        assert_eq!(snap.pv1_power, 2500); // index 0 is available
        assert_eq!(snap.pv2_power, 0); // index 1 out of bounds -> 0
        assert_eq!(snap.home_power, 2500); // solar=2500 + battery=0 + grid=0
    }

    #[test]
    fn decode_snapshot_all_zero_input() {
        let blocks = vec![make_block(
            RegisterType::Input,
            0,
            60,
            "input_0_59",
            vec![0; 60],
        )];
        let snap = decode_snapshot(&blocks);

        assert_eq!(snap.solar_power, 0);
        assert_eq!(snap.battery_power, 0);
        assert_eq!(snap.grid_power, 0);
        assert_eq!(snap.home_power, 0);
        assert_eq!(snap.battery_state, BatteryState::Idle);
    }
}
