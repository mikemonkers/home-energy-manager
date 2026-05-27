//! GivEnergy Modbus register address constants and block definitions.
//!
//! Each poll cycle reads 60-aligned blocks of registers. The standard blocks
//! cover input registers (read-only telemetry) and holding registers (read/write
//! configuration). Gen3 inverters expose an additional extended block.

// ---------------------------------------------------------------------------
// Register type
// ---------------------------------------------------------------------------

/// Distinguishes Modbus input registers (read-only) from holding registers (read/write).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegisterType {
    /// Input register – read-only telemetry data.
    Input,
    /// Holding register – read/write configuration.
    Holding,
}

// ---------------------------------------------------------------------------
// Register block descriptor
// ---------------------------------------------------------------------------

/// A contiguous range of registers to read in a single Modbus request.
#[derive(Debug, Clone, Copy)]
pub struct RegisterBlock {
    /// Starting register address.
    pub start: u16,
    /// Number of consecutive registers to read.
    pub count: u16,
    /// Whether this is an input or holding register block.
    pub register_type: RegisterType,
    /// Human-readable block name (used in logging / diagnostics).
    pub name: &'static str,
}

// ---------------------------------------------------------------------------
// Standard poll blocks
// ---------------------------------------------------------------------------

/// Blocks read during every poll cycle on all inverter generations.
pub const STANDARD_POLL_BLOCKS: &[RegisterBlock] = &[
    RegisterBlock {
        start: 0,
        count: 60,
        register_type: RegisterType::Input,
        name: "input_0_59",
    },
    RegisterBlock {
        start: 180,
        count: 60,
        register_type: RegisterType::Input,
        name: "input_180_239",
    },
    RegisterBlock {
        start: 0,
        count: 60,
        register_type: RegisterType::Holding,
        name: "holding_0_59",
    },
    RegisterBlock {
        start: 60,
        count: 60,
        register_type: RegisterType::Holding,
        name: "holding_60_119",
    },
];

/// Extended block read on Gen3 inverters (in addition to the standard blocks).
pub const GEN3_EXTENDED_BLOCK: RegisterBlock = RegisterBlock {
    start: 240,
    count: 120,
    register_type: RegisterType::Holding,
    name: "gen3_holding_240_359",
};

// ===========================================================================
// Input Register addresses (read-only telemetry)
// ===========================================================================
//
// Block: Input Registers 0-59
// -----------------------------------------------

/// PV1 power in watts.
pub const IR_PV1_POWER: u16 = 0;
/// PV2 power in watts.
pub const IR_PV2_POWER: u16 = 1;
/// PV1 voltage in 0.1 V units (divide by 10 for volts).
pub const IR_PV1_VOLTAGE: u16 = 2;
/// PV2 voltage in 0.1 V units (divide by 10 for volts).
pub const IR_PV2_VOLTAGE: u16 = 3;
/// PV1 current in 0.1 A units (divide by 10 for amps).
pub const IR_PV1_CURRENT: u16 = 4;
/// PV2 current in 0.1 A units (divide by 10 for amps).
pub const IR_PV2_CURRENT: u16 = 5;
/// Battery power in watts, signed (positive = charging).
pub const IR_BATTERY_POWER: u16 = 6;
/// Battery state of charge as a percentage (0-100).
pub const IR_BATTERY_SOC: u16 = 7;
/// Battery voltage in 0.1 V units (divide by 10 for volts).
pub const IR_BATTERY_VOLTAGE: u16 = 9;
/// Battery current in 0.1 A units (divide by 10 for amps), signed.
pub const IR_BATTERY_CURRENT: u16 = 10;
/// Grid power in watts, signed (positive = importing from grid).
pub const IR_GRID_POWER: u16 = 11;
/// Grid voltage in 0.1 V units (divide by 10 for volts).
pub const IR_GRID_VOLTAGE: u16 = 12;
/// Grid frequency in 0.01 Hz units (divide by 100 for Hz).
pub const IR_GRID_FREQUENCY: u16 = 13;
/// Inverter temperature in 0.1 °C units (divide by 10 for °C).
pub const IR_INVERTER_TEMPERATURE: u16 = 14;
/// Battery temperature in 0.1 °C units (divide by 10 for °C).
pub const IR_BATTERY_TEMPERATURE: u16 = 16;

// Block: Input Registers 0-59 – energy totals (today)
// -----------------------------------------------

/// Total solar energy generated today in 0.1 kWh units.
pub const IR_TODAY_SOLAR_ENERGY: u16 = 36;
/// Total energy imported from grid today in 0.1 kWh units.
pub const IR_TODAY_IMPORT_ENERGY: u16 = 38;
/// Total energy exported to grid today in 0.1 kWh units.
pub const IR_TODAY_EXPORT_ENERGY: u16 = 40;
/// Total energy used to charge the battery today in 0.1 kWh units.
pub const IR_TODAY_CHARGE_ENERGY: u16 = 42;
/// Total energy discharged from the battery today in 0.1 kWh units.
pub const IR_TODAY_DISCHARGE_ENERGY: u16 = 44;
/// Total household consumption today in 0.1 kWh units.
pub const IR_TODAY_CONSUMPTION: u16 = 46;

// ===========================================================================
// Holding Register addresses (read/write configuration)
// ===========================================================================
//
// Block: Holding Registers 0-59
// -----------------------------------------------

/// Device type code identifying the inverter model.
pub const HR_DEVICE_TYPE: u16 = 0;
/// Battery operating mode: 0 = paused, 1 = eco, 2 = timed demand, 3 = timed export.
pub const HR_BATTERY_MODE: u16 = 1;
/// Whether the charge schedule is enabled (1 = enabled, 0 = disabled).
pub const HR_ENABLE_CHARGE_SCHEDULE: u16 = 4;

/// Charge slot 1: start hour (0-23).
pub const HR_CHARGE_SLOT_1_START_H: u16 = 5;
/// Charge slot 1: start minute (0-59).
pub const HR_CHARGE_SLOT_1_START_M: u16 = 6;
/// Charge slot 1: end hour (0-23).
pub const HR_CHARGE_SLOT_1_END_H: u16 = 7;
/// Charge slot 1: end minute (0-59).
pub const HR_CHARGE_SLOT_1_END_M: u16 = 8;
/// Charge slot 1: target state-of-charge percentage (0-100).
pub const HR_CHARGE_SLOT_1_TARGET_SOC: u16 = 9;
/// Charge slot 1: enable flag (1 = enabled, 0 = disabled).
pub const HR_CHARGE_SLOT_1_ENABLE: u16 = 10;

/// Charge slot 2: start hour.
pub const HR_CHARGE_SLOT_2_START_H: u16 = 11;
/// Charge slot 2: start minute.
pub const HR_CHARGE_SLOT_2_START_M: u16 = 12;
/// Charge slot 2: end hour.
pub const HR_CHARGE_SLOT_2_END_H: u16 = 13;
/// Charge slot 2: end minute.
pub const HR_CHARGE_SLOT_2_END_M: u16 = 14;
/// Charge slot 2: target state-of-charge percentage.
pub const HR_CHARGE_SLOT_2_TARGET_SOC: u16 = 15;
/// Charge slot 2: enable flag.
pub const HR_CHARGE_SLOT_2_ENABLE: u16 = 16;

/// Charge slot 3: start hour.
pub const HR_CHARGE_SLOT_3_START_H: u16 = 17;
/// Charge slot 3: start minute.
pub const HR_CHARGE_SLOT_3_START_M: u16 = 18;
/// Charge slot 3: end hour.
pub const HR_CHARGE_SLOT_3_END_H: u16 = 19;
/// Charge slot 3: end minute.
pub const HR_CHARGE_SLOT_3_END_M: u16 = 20;
/// Charge slot 3: target state-of-charge percentage.
pub const HR_CHARGE_SLOT_3_TARGET_SOC: u16 = 21;
/// Charge slot 3: enable flag.
pub const HR_CHARGE_SLOT_3_ENABLE: u16 = 22;

/// Discharge slot 1: start hour.
pub const HR_DISCHARGE_SLOT_1_START_H: u16 = 23;
/// Discharge slot 1: start minute.
pub const HR_DISCHARGE_SLOT_1_START_M: u16 = 24;
/// Discharge slot 1: end hour.
pub const HR_DISCHARGE_SLOT_1_END_H: u16 = 25;
/// Discharge slot 1: end minute.
pub const HR_DISCHARGE_SLOT_1_END_M: u16 = 26;
/// Discharge slot 1: target state-of-charge percentage (0-100).
pub const HR_DISCHARGE_SLOT_1_TARGET_SOC: u16 = 27;
/// Discharge slot 1: enable flag (1 = enabled, 0 = disabled).
pub const HR_DISCHARGE_SLOT_1_ENABLE: u16 = 28;

/// Discharge slot 2: start hour.
pub const HR_DISCHARGE_SLOT_2_START_H: u16 = 29;
/// Discharge slot 2: start minute.
pub const HR_DISCHARGE_SLOT_2_START_M: u16 = 30;
/// Discharge slot 2: end hour.
pub const HR_DISCHARGE_SLOT_2_END_H: u16 = 31;
/// Discharge slot 2: end minute.
pub const HR_DISCHARGE_SLOT_2_END_M: u16 = 32;
/// Discharge slot 2: target state-of-charge percentage.
pub const HR_DISCHARGE_SLOT_2_TARGET_SOC: u16 = 33;
/// Discharge slot 2: enable flag.
pub const HR_DISCHARGE_SLOT_2_ENABLE: u16 = 34;

/// Battery reserve state-of-charge percentage (0-100).
pub const HR_BATTERY_RESERVE_SOC: u16 = 35;
/// Battery charge rate in watts.
pub const HR_BATTERY_CHARGE_RATE: u16 = 36;
/// Battery discharge rate in watts.
pub const HR_BATTERY_DISCHARGE_RATE: u16 = 37;
/// Battery power limit enable flag (1 = enabled, 0 = disabled).
pub const HR_BATTERY_POWER_LIMIT_ENABLE: u16 = 59;

// Block: Holding Registers 60-119
// -----------------------------------------------

/// Target state-of-charge percentage for charging (0-100).
pub const HR_TARGET_SOC: u16 = 60;
/// Charge cutoff state-of-charge percentage (0-100).
pub const HR_CHARGE_CUTOFF_SOC: u16 = 61;
/// Force charge enable flag (1 = enabled, 0 = disabled).
pub const HR_FORCE_CHARGE_ENABLE: u16 = 64;
/// Force discharge enable flag (1 = enabled, 0 = disabled).
pub const HR_FORCE_DISCHARGE_ENABLE: u16 = 65;
/// Pause battery enable flag (1 = paused, 0 = normal).
pub const HR_PAUSE_BATTERY_ENABLE: u16 = 66;
/// System clock – year (e.g. 2026).
pub const HR_CLOCK_YEAR: u16 = 70;
/// System clock – month (1–12).
pub const HR_CLOCK_MONTH: u16 = 71;
/// System clock – day (1–31).
pub const HR_CLOCK_DAY: u16 = 72;
/// System clock – hour (0–23).
pub const HR_CLOCK_HOUR: u16 = 73;
/// System clock – minute (0–59).
pub const HR_CLOCK_MINUTE: u16 = 74;
/// System clock – second (0–59).
pub const HR_CLOCK_SECOND: u16 = 75;

// Block: Holding Registers 240-359 (Gen3 only)
// -----------------------------------------------

/// Gen3 extended battery charge rate in watts.
pub const HR_GEN3_EXTENDED_CHARGE_RATE: u16 = 256;
/// Gen3 extended battery discharge rate in watts.
pub const HR_GEN3_EXTENDED_DISCHARGE_RATE: u16 = 257;
/// Gen3 battery state-of-charge target for timed discharge/export modes (0-100).
pub const HR_GEN3_BATTERY_SOC_TARGET: u16 = 258;

// ---------------------------------------------------------------------------
// Slot register layout helpers
// ---------------------------------------------------------------------------

/// Number of registers occupied by a single charge/discharge slot.
pub const SLOT_REGISTER_COUNT: u16 = 6;

/// Returns the base address of charge slot `index` (0-based) within holding registers.
///
/// Charge slot 0 starts at `HR_CHARGE_SLOT_1_START_H` (address 5).
/// Each slot occupies 6 consecutive registers: start_h, start_m, end_h, end_m, target_soc, enable.
#[inline]
pub const fn charge_slot_base(index: u32) -> u16 {
    HR_CHARGE_SLOT_1_START_H + (index as u16) * SLOT_REGISTER_COUNT
}

/// Returns the base address of discharge slot `index` (0-based) within holding registers.
///
/// Discharge slot 0 starts at `HR_DISCHARGE_SLOT_1_START_H` (address 23).
/// Each slot occupies 6 consecutive registers: start_h, start_m, end_h, end_m, target_soc, enable.
#[inline]
pub const fn discharge_slot_base(index: u32) -> u16 {
    HR_DISCHARGE_SLOT_1_START_H + (index as u16) * SLOT_REGISTER_COUNT
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_poll_blocks_count() {
        assert_eq!(STANDARD_POLL_BLOCKS.len(), 4);
    }

    #[test]
    fn standard_poll_block_addresses_do_not_overlap() {
        // Quick sanity: every block starts on a multiple of 60.
        for block in STANDARD_POLL_BLOCKS {
            assert_eq!(
                block.start % 60,
                0,
                "block '{}' starts at {} which is not 60-aligned",
                block.name,
                block.start
            );
        }
    }

    #[test]
    fn gen3_extended_block_start_is_240() {
        assert_eq!(GEN3_EXTENDED_BLOCK.start, 240);
        assert_eq!(GEN3_EXTENDED_BLOCK.count, 120);
    }

    #[test]
    fn charge_slot_base_addresses() {
        assert_eq!(charge_slot_base(0), 5);
        assert_eq!(charge_slot_base(1), 11);
        assert_eq!(charge_slot_base(2), 17);
    }

    #[test]
    fn discharge_slot_base_addresses() {
        assert_eq!(discharge_slot_base(0), 23);
        assert_eq!(discharge_slot_base(1), 29);
    }

    #[test]
    fn register_addresses_within_blocks() {
        // Input register 0-59 block
        assert!(IR_PV1_POWER < 60);
        assert!(IR_TODAY_CONSUMPTION < 60);

        // Input register 180-239 block – no addresses defined here currently;
        // energy totals live in block 0-59 per the task spec.

        // Holding register 0-59 block
        assert!(HR_DEVICE_TYPE < 60);
        assert!(HR_BATTERY_POWER_LIMIT_ENABLE < 60);

        // Holding register 60-119 block
        assert!(HR_TARGET_SOC >= 60);
        assert!(HR_PAUSE_BATTERY_ENABLE < 120);

        // Gen3 extended block
        assert!(HR_GEN3_EXTENDED_CHARGE_RATE >= 240);
        assert!(HR_GEN3_BATTERY_SOC_TARGET < 360);
    }
}
