//! Model-to-register encoder.
//!
//! Converts high-level inverter commands (e.g. set charge rate,
//! enable discharge) into the raw Modbus holding-register values
//! that the inverter expects, with safety validation.

use crate::modbus::registers::{
    charge_slot_base, discharge_slot_base, HR_BATTERY_CHARGE_RATE, HR_BATTERY_DISCHARGE_RATE,
    HR_BATTERY_MODE, HR_BATTERY_RESERVE_SOC, HR_CLOCK_YEAR, HR_FORCE_CHARGE_ENABLE,
    HR_FORCE_DISCHARGE_ENABLE, HR_PAUSE_BATTERY_ENABLE, HR_TARGET_SOC,
};

use chrono::{Datelike, Timelike};

use super::model::{BatteryMode, ScheduleSlot};

// ---------------------------------------------------------------------------
// Constants / limits
// ---------------------------------------------------------------------------

/// Maximum charge / discharge rate in watts.
const MAX_RATE_W: u16 = 5000;
/// Maximum duration for force-charge / force-discharge / pause commands (minutes).
const MAX_DURATION_MINUTES: u16 = 1440; // 24 h

// ---------------------------------------------------------------------------
// Control command enum
// ---------------------------------------------------------------------------

/// Commands that can be sent to the inverter.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ControlCommand {
    SetBatteryMode { mode: BatteryMode },
    SetReserve { soc: u8 },
    SetChargeRate { rate: u16 },
    SetDischargeRate { rate: u16 },
    SetTargetSoc { soc: u8 },
    SetChargeSlot { slot: u8, config: ScheduleSlot },
    SetDischargeSlot { slot: u8, config: ScheduleSlot },
    ForceCharge { minutes: u16 },
    ForceDischarge { minutes: u16 },
    PauseBattery { minutes: u16 },
    SyncClock,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Safety validation error.
#[derive(Debug, thiserror::Error)]
pub enum EncoderError {
    #[error("Invalid slot index: {0}, must be 1-3 for charge or 1-2 for discharge")]
    InvalidSlot(u8),
    #[error("SOC out of range: {0}, must be 0-100")]
    SocOutOfRange(u8),
    #[error("Rate out of range: {0}W")]
    RateOutOfRange(u16),
    #[error("Duration out of range: {0} minutes")]
    DurationOutOfRange(u16),
    #[error("Invalid schedule time: {start_h}:{start_m:02} - {end_h}:{end_m:02}")]
    InvalidScheduleTime {
        start_h: u8,
        start_m: u8,
        end_h: u8,
        end_m: u8,
    },
}

// ---------------------------------------------------------------------------
// Encoded write result
// ---------------------------------------------------------------------------

/// Result of encoding a command – the holding-register address and values to
/// write via Modbus function code 0x10.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedWrite {
    /// Starting holding-register address.
    pub address: u16,
    /// Register values to write (one or more consecutive registers).
    pub values: Vec<u16>,
}

// ---------------------------------------------------------------------------
// Encoder implementation
// ---------------------------------------------------------------------------

/// Encode a control command into validated register writes.
///
/// Returns one or more [`EncodedWrite`] operations that the caller should
/// transmit to the inverter via Modbus write-multiple-registers (FC 0x10).
pub fn encode_command(cmd: &ControlCommand) -> Result<Vec<EncodedWrite>, EncoderError> {
    match cmd {
        ControlCommand::SetBatteryMode { mode } => {
            Ok(vec![EncodedWrite {
                address: HR_BATTERY_MODE,
                values: vec![mode.to_register()],
            }])
        }

        ControlCommand::SetReserve { soc } => {
            validate_soc(*soc)?;
            Ok(vec![EncodedWrite {
                address: HR_BATTERY_RESERVE_SOC,
                values: vec![*soc as u16],
            }])
        }

        ControlCommand::SetChargeRate { rate } => {
            validate_rate(*rate)?;
            Ok(vec![EncodedWrite {
                address: HR_BATTERY_CHARGE_RATE,
                values: vec![*rate],
            }])
        }

        ControlCommand::SetDischargeRate { rate } => {
            validate_rate(*rate)?;
            Ok(vec![EncodedWrite {
                address: HR_BATTERY_DISCHARGE_RATE,
                values: vec![*rate],
            }])
        }

        ControlCommand::SetTargetSoc { soc } => {
            validate_soc(*soc)?;
            Ok(vec![EncodedWrite {
                address: HR_TARGET_SOC,
                values: vec![*soc as u16],
            }])
        }

        ControlCommand::SetChargeSlot { slot, config } => {
            validate_slot(*slot, /* is_charge */ true)?;
            validate_schedule(config)?;
            let base = charge_slot_base((*slot - 1) as u32);
            Ok(vec![encode_slot(base, config)])
        }

        ControlCommand::SetDischargeSlot { slot, config } => {
            validate_slot(*slot, /* is_charge */ false)?;
            validate_schedule(config)?;
            let base = discharge_slot_base((*slot - 1) as u32);
            Ok(vec![encode_slot(base, config)])
        }

        ControlCommand::ForceCharge { minutes } => {
            validate_duration(*minutes)?;
            Ok(vec![EncodedWrite {
                address: HR_FORCE_CHARGE_ENABLE,
                values: vec![1, *minutes],
            }])
        }

        ControlCommand::ForceDischarge { minutes } => {
            validate_duration(*minutes)?;
            Ok(vec![EncodedWrite {
                address: HR_FORCE_DISCHARGE_ENABLE,
                values: vec![1, *minutes],
            }])
        }

        ControlCommand::PauseBattery { minutes } => {
            validate_duration(*minutes)?;
            Ok(vec![EncodedWrite {
                address: HR_PAUSE_BATTERY_ENABLE,
                values: vec![1, *minutes],
            }])
        }

        ControlCommand::SyncClock => {
            let now = chrono::Local::now();
            Ok(vec![EncodedWrite {
                address: HR_CLOCK_YEAR,
                values: vec![
                    (now.year() - 2000) as u16, // year offset from 2000
                    now.month() as u16,
                    now.day() as u16,
                    now.hour() as u16,
                    now.minute() as u16,
                    now.second() as u16,
                ],
            }])
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn validate_soc(soc: u8) -> Result<(), EncoderError> {
    if soc > 100 {
        Err(EncoderError::SocOutOfRange(soc))
    } else {
        Ok(())
    }
}

fn validate_rate(rate: u16) -> Result<(), EncoderError> {
    if rate > MAX_RATE_W {
        Err(EncoderError::RateOutOfRange(rate))
    } else {
        Ok(())
    }
}

fn validate_duration(minutes: u16) -> Result<(), EncoderError> {
    if minutes == 0 || minutes > MAX_DURATION_MINUTES {
        Err(EncoderError::DurationOutOfRange(minutes))
    } else {
        Ok(())
    }
}

fn validate_slot(slot: u8, is_charge: bool) -> Result<(), EncoderError> {
    let max = if is_charge { 3 } else { 2 };
    if slot < 1 || slot > max {
        Err(EncoderError::InvalidSlot(slot))
    } else {
        Ok(())
    }
}

fn validate_schedule(slot: &ScheduleSlot) -> Result<(), EncoderError> {
    if slot.start_hour > 23
        || slot.start_minute > 59
        || slot.end_hour > 23
        || slot.end_minute > 59
    {
        return Err(EncoderError::InvalidScheduleTime {
            start_h: slot.start_hour,
            start_m: slot.start_minute,
            end_h: slot.end_hour,
            end_m: slot.end_minute,
        });
    }
    validate_soc(slot.target_soc)?;
    Ok(())
}

fn encode_slot(base: u16, slot: &ScheduleSlot) -> EncodedWrite {
    EncodedWrite {
        address: base,
        values: vec![
            slot.start_hour as u16,
            slot.start_minute as u16,
            slot.end_hour as u16,
            slot.end_minute as u16,
            slot.target_soc as u16,
            u16::from(slot.enabled),
        ],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to build a schedule slot.
    fn make_slot(
        enabled: bool,
        start_h: u8,
        start_m: u8,
        end_h: u8,
        end_m: u8,
        target_soc: u8,
    ) -> ScheduleSlot {
        ScheduleSlot {
            enabled,
            start_hour: start_h,
            start_minute: start_m,
            end_hour: end_h,
            end_minute: end_m,
            target_soc,
        }
    }

    // -- SetBatteryMode -------------------------------------------------------

    #[test]
    fn encode_set_battery_mode_eco() {
        let cmd = ControlCommand::SetBatteryMode {
            mode: BatteryMode::Eco,
        };
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].address, HR_BATTERY_MODE);
        assert_eq!(writes[0].values, vec![1]);
    }

    #[test]
    fn encode_set_battery_mode_all_variants() {
        for (mode, expected) in [
            (BatteryMode::Paused, 0u16),
            (BatteryMode::Eco, 1),
            (BatteryMode::TimedDemand, 2),
            (BatteryMode::TimedExport, 3),
        ] {
            let cmd = ControlCommand::SetBatteryMode { mode };
            let writes = encode_command(&cmd).unwrap();
            assert_eq!(writes[0].values, vec![expected], "failed for {mode:?}");
        }
    }

    // -- SetReserve -----------------------------------------------------------

    #[test]
    fn encode_set_reserve_valid() {
        let cmd = ControlCommand::SetReserve { soc: 20 };
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes[0].address, HR_BATTERY_RESERVE_SOC);
        assert_eq!(writes[0].values, vec![20]);
    }

    #[test]
    fn encode_set_reserve_zero_is_valid() {
        let cmd = ControlCommand::SetReserve { soc: 0 };
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes[0].values, vec![0]);
    }

    #[test]
    fn encode_set_reserve_100_is_valid() {
        let cmd = ControlCommand::SetReserve { soc: 100 };
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes[0].values, vec![100]);
    }

    #[test]
    fn encode_set_reserve_rejects_101() {
        let cmd = ControlCommand::SetReserve { soc: 101 };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::SocOutOfRange(101)));
    }

    // -- SetChargeRate --------------------------------------------------------

    #[test]
    fn encode_set_charge_rate_valid() {
        let cmd = ControlCommand::SetChargeRate { rate: 2500 };
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes[0].address, HR_BATTERY_CHARGE_RATE);
        assert_eq!(writes[0].values, vec![2500]);
    }

    #[test]
    fn encode_set_charge_rate_max_boundary() {
        let cmd = ControlCommand::SetChargeRate { rate: 5000 };
        assert!(encode_command(&cmd).is_ok());
    }

    #[test]
    fn encode_set_charge_rate_rejects_over_max() {
        let cmd = ControlCommand::SetChargeRate { rate: 5001 };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::RateOutOfRange(5001)));
    }

    // -- SetDischargeRate -----------------------------------------------------

    #[test]
    fn encode_set_discharge_rate_valid() {
        let cmd = ControlCommand::SetDischargeRate { rate: 3000 };
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes[0].address, HR_BATTERY_DISCHARGE_RATE);
        assert_eq!(writes[0].values, vec![3000]);
    }

    #[test]
    fn encode_set_discharge_rate_rejects_over_max() {
        let cmd = ControlCommand::SetDischargeRate { rate: 6000 };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::RateOutOfRange(6000)));
    }

    // -- SetTargetSoc ---------------------------------------------------------

    #[test]
    fn encode_set_target_soc_valid() {
        let cmd = ControlCommand::SetTargetSoc { soc: 80 };
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes[0].address, HR_TARGET_SOC);
        assert_eq!(writes[0].values, vec![80]);
    }

    #[test]
    fn encode_set_target_soc_rejects_over_100() {
        let cmd = ControlCommand::SetTargetSoc { soc: 150 };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::SocOutOfRange(150)));
    }

    // -- SetChargeSlot --------------------------------------------------------

    #[test]
    fn encode_set_charge_slot_1() {
        let slot = make_slot(true, 0, 30, 6, 0, 100);
        let cmd = ControlCommand::SetChargeSlot { slot: 1, config: slot };
        let writes = encode_command(&cmd).unwrap();
        // Charge slot 1 base = charge_slot_base(0) = 5
        assert_eq!(writes[0].address, 5);
        assert_eq!(writes[0].values, vec![0, 30, 6, 0, 100, 1]);
    }

    #[test]
    fn encode_set_charge_slot_2() {
        let slot = make_slot(false, 12, 0, 14, 0, 50);
        let cmd = ControlCommand::SetChargeSlot { slot: 2, config: slot };
        let writes = encode_command(&cmd).unwrap();
        // Charge slot 2 base = charge_slot_base(1) = 11
        assert_eq!(writes[0].address, 11);
        assert_eq!(writes[0].values, vec![12, 0, 14, 0, 50, 0]);
    }

    #[test]
    fn encode_set_charge_slot_3() {
        let slot = make_slot(true, 23, 59, 23, 59, 0);
        let cmd = ControlCommand::SetChargeSlot { slot: 3, config: slot };
        let writes = encode_command(&cmd).unwrap();
        // Charge slot 3 base = charge_slot_base(2) = 17
        assert_eq!(writes[0].address, 17);
    }

    #[test]
    fn encode_set_charge_slot_rejects_slot_0() {
        let slot = make_slot(true, 0, 0, 6, 0, 100);
        let cmd = ControlCommand::SetChargeSlot { slot: 0, config: slot };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::InvalidSlot(0)));
    }

    #[test]
    fn encode_set_charge_slot_rejects_slot_4() {
        let slot = make_slot(true, 0, 0, 6, 0, 100);
        let cmd = ControlCommand::SetChargeSlot { slot: 4, config: slot };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::InvalidSlot(4)));
    }

    #[test]
    fn encode_set_charge_slot_rejects_bad_hour() {
        let slot = make_slot(true, 24, 0, 6, 0, 100);
        let cmd = ControlCommand::SetChargeSlot { slot: 1, config: slot };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(
            err,
            EncoderError::InvalidScheduleTime { .. }
        ));
    }

    #[test]
    fn encode_set_charge_slot_rejects_bad_minute() {
        let slot = make_slot(true, 0, 60, 6, 0, 100);
        let cmd = ControlCommand::SetChargeSlot { slot: 1, config: slot };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(
            err,
            EncoderError::InvalidScheduleTime { .. }
        ));
    }

    #[test]
    fn encode_set_charge_slot_rejects_bad_soc() {
        let slot = make_slot(true, 0, 0, 6, 0, 101);
        let cmd = ControlCommand::SetChargeSlot { slot: 1, config: slot };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::SocOutOfRange(101)));
    }

    // -- SetDischargeSlot -----------------------------------------------------

    #[test]
    fn encode_set_discharge_slot_1() {
        let slot = make_slot(true, 16, 0, 19, 0, 10);
        let cmd = ControlCommand::SetDischargeSlot { slot: 1, config: slot };
        let writes = encode_command(&cmd).unwrap();
        // Discharge slot 1 base = discharge_slot_base(0) = 23
        assert_eq!(writes[0].address, 23);
        assert_eq!(writes[0].values, vec![16, 0, 19, 0, 10, 1]);
    }

    #[test]
    fn encode_set_discharge_slot_2() {
        let slot = make_slot(true, 20, 0, 22, 30, 20);
        let cmd = ControlCommand::SetDischargeSlot { slot: 2, config: slot };
        let writes = encode_command(&cmd).unwrap();
        // Discharge slot 2 base = discharge_slot_base(1) = 29
        assert_eq!(writes[0].address, 29);
    }

    #[test]
    fn encode_set_discharge_slot_rejects_slot_3() {
        let slot = make_slot(true, 0, 0, 6, 0, 100);
        let cmd = ControlCommand::SetDischargeSlot { slot: 3, config: slot };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::InvalidSlot(3)));
    }

    #[test]
    fn encode_set_discharge_slot_rejects_slot_0() {
        let slot = make_slot(true, 0, 0, 6, 0, 100);
        let cmd = ControlCommand::SetDischargeSlot { slot: 0, config: slot };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::InvalidSlot(0)));
    }

    // -- ForceCharge ----------------------------------------------------------

    #[test]
    fn encode_force_charge() {
        let cmd = ControlCommand::ForceCharge { minutes: 30 };
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes[0].address, HR_FORCE_CHARGE_ENABLE);
        assert_eq!(writes[0].values, vec![1, 30]);
    }

    #[test]
    fn encode_force_charge_rejects_zero() {
        let cmd = ControlCommand::ForceCharge { minutes: 0 };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::DurationOutOfRange(0)));
    }

    // -- ForceDischarge -------------------------------------------------------

    #[test]
    fn encode_force_discharge() {
        let cmd = ControlCommand::ForceDischarge { minutes: 60 };
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes[0].address, HR_FORCE_DISCHARGE_ENABLE);
        assert_eq!(writes[0].values, vec![1, 60]);
    }

    #[test]
    fn encode_force_discharge_rejects_over_24h() {
        let cmd = ControlCommand::ForceDischarge { minutes: 1441 };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::DurationOutOfRange(1441)));
    }

    // -- PauseBattery ---------------------------------------------------------

    #[test]
    fn encode_pause_battery() {
        let cmd = ControlCommand::PauseBattery { minutes: 45 };
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes[0].address, HR_PAUSE_BATTERY_ENABLE);
        assert_eq!(writes[0].values, vec![1, 45]);
    }

    #[test]
    fn encode_pause_battery_rejects_zero() {
        let cmd = ControlCommand::PauseBattery { minutes: 0 };
        let err = encode_command(&cmd).unwrap_err();
        assert!(matches!(err, EncoderError::DurationOutOfRange(0)));
    }

    // -- SyncClock ------------------------------------------------------------

    #[test]
    fn encode_sync_clock() {
        let cmd = ControlCommand::SyncClock;
        let writes = encode_command(&cmd).unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].address, HR_CLOCK_YEAR);
        assert_eq!(writes[0].values.len(), 6);
        // The values should be valid ranges for year-offset, month, day, hour, minute, second
        let v = &writes[0].values;
        assert!(v[0] <= 100, "year offset should be <= 100"); // 2000-2100 => 0-100
        assert!(v[1] >= 1 && v[1] <= 12, "month should be 1-12");
        assert!(v[2] >= 1 && v[2] <= 31, "day should be 1-31");
        assert!(v[3] <= 23, "hour should be 0-23");
        assert!(v[4] <= 59, "minute should be 0-59");
        assert!(v[5] <= 59, "second should be 0-59");
    }

    // -- Error display --------------------------------------------------------

    #[test]
    fn encoder_error_messages() {
        let e = EncoderError::InvalidSlot(5);
        assert!(format!("{e}").contains("5"));

        let e = EncoderError::SocOutOfRange(200);
        assert!(format!("{e}").contains("200"));

        let e = EncoderError::RateOutOfRange(9999);
        assert!(format!("{e}").contains("9999"));

        let e = EncoderError::DurationOutOfRange(0);
        assert!(format!("{e}").contains("0"));

        let e = EncoderError::InvalidScheduleTime {
            start_h: 25,
            start_m: 0,
            end_h: 6,
            end_m: 0,
        };
        let msg = format!("{e}");
        assert!(msg.contains("25:00"));
        assert!(msg.contains("6:00"));
    }
}
