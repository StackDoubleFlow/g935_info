//! Much of this was ported from [HeadsetControl](https://github.com/Sapd/HeadsetControl)

use std::{process::exit, thread::sleep, time::Duration};

use clap::{Parser, Subcommand};
use hidapi::{HidApi, HidDevice, HidResult};

const VID: u16 = 0x046d;
const PID: u16 = 0x0a87;

const HIDPP_LONG_MESSAGE: u8 = 0x11;
const HIDPP_LONG_MESSAGE_LENGTH: usize = 20;
const HIDPP_DEVICE_RECEIVER: u8 = 0xff;

const I3_STATUS_INTERVAL: Duration = Duration::from_millis(500);
const PULSE_CARD: &str = "alsa_card.usb-Logitech_G935_Gaming_Headset-00";
const PULSE_PROFILE: &str = "output:analog-stereo+input:mono-fallback";

#[derive(Subcommand, PartialEq, Eq, Debug)]
enum Command {
    GetBatteryPercentage,
    GetBatteryVoltage,
    GetI3Status {
        #[arg(long)]
        update_pulseaudio: bool,
    },
}

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn estimate_battery_level(voltage: u16) -> f32 {
    let voltage = voltage as f32;
    if voltage <= 3525.0 {
        return (0.03 * voltage) - 101.0;
    }
    if voltage > 4030.0 {
        return 100.0;
    }
    // f(x)=3.7268473047*10^(-9)x^(4)-0.00005605626214573775*x^(3)+0.3156051902814949*x^(2)-788.0937250298629*x+736315.3077118985
    0.0000000037268473047 * voltage.powf(4.0) - 0.00005605626214573775 * voltage.powf(3.0)
        + 0.3156051902814949 * voltage.powf(2.0)
        - 788.0937250298629 * voltage
        + 736315.3077118985
}

fn get_device() -> HidResult<HidDevice> {
    let api = HidApi::new()?;
    api.open(VID, PID)
}

fn get_battery_voltage() -> HidResult<(u16, bool)> {
    let device = get_device()?;

    let data_request: [u8; HIDPP_LONG_MESSAGE_LENGTH] = [
        HIDPP_LONG_MESSAGE,
        HIDPP_DEVICE_RECEIVER,
        0x08,
        0x0a,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
    ];

    device.write(&data_request)?;

    let mut data_read = [0; 7];
    let bytes_read = device.read_timeout(&mut data_read, 5000)?;
    if bytes_read == 0 {
        eprintln!("Device read timed out.");
        exit(1);
    }

    // 6th byte is state; 0x1 for idle, 0x3 for charging
    let state = data_read[6];
    let charging = state == 0x03;

    let voltage = ((data_read[4] as u16) << 8) | data_read[5] as u16;

    Ok((voltage, charging))
}

fn get_i3_status(percentage: f32, charging: bool) -> String {
    let state = if charging {
        if percentage >= 100.0 {
            "Good"
        } else {
            "info"
        }
    } else {
        if percentage < 0.0 {
            // Disconnected
            "Idle"
        } else if percentage < 5.0 {
            "Critical"
        } else if percentage < 15.0 {
            "Warning"
        } else {
            "Info"
        }
    };

    let text = format!("{:.0}%", percentage);
    let text = if percentage < 0.0 {
        "Disconnected"
    } else {
        &text
    };
    let icon = if charging {
        "headset_charging"
    } else {
        "headset"
    };
    format!("{{\"state\":\"{state}\",\"text\":\"{text}\",\"icon\":\"{icon}\"}}")
}

fn pulse_set_card_profile(card: &str, profile: &str) {
    std::process::Command::new("pactl")
        .arg("set-card-profile")
        .arg(card)
        .arg(profile)
        .spawn()
        .unwrap();
}

fn main() {
    let cli = Cli::parse();

    if let Command::GetI3Status { update_pulseaudio } = cli.command {
        let mut last_connected = true;
        loop {
            let Ok((voltage, charging)) = get_battery_voltage() else {
                println!("{{\"text\":\"\"}}");
                sleep(I3_STATUS_INTERVAL);
                continue;
            };
            let percentage = estimate_battery_level(voltage);
            println!("{}", get_i3_status(percentage, charging));

            if update_pulseaudio {
                let connected = percentage >= 0.0;
                if connected && !last_connected {
                    pulse_set_card_profile(PULSE_CARD, PULSE_PROFILE);
                } else if !connected && last_connected {
                    pulse_set_card_profile(PULSE_CARD, "off");
                }
                last_connected = connected;
            }

            sleep(I3_STATUS_INTERVAL);
        }
    }

    let (voltage, charging) = match get_battery_voltage() {
        Ok(x) => x,
        Err(err) => {
            eprintln!("{}", err);
            exit(1);
        }
    };
    let percentage = estimate_battery_level(voltage);

    // TODO: find a bettery way to check this
    if percentage < 0.0 {
        eprintln!("Wireless connection disconnected.");
        exit(1);
    }
    match cli.command {
        Command::GetBatteryVoltage => println!("{}", voltage),
        Command::GetBatteryPercentage => println!("{}", percentage),
        _ => unreachable!(),
    }
    println!("Charging: {}", charging as u8);
}
