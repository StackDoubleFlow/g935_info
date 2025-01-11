use std::{fs, path::PathBuf, process::exit, thread::sleep, time::Duration};

use clap::{Parser, Subcommand};

const VID: u16 = 0x046d;
const PID: u16 = 0x0a87;

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

fn get_i3_status(connected: bool, percentage: u32, charging: bool) -> String {
    let state = if connected {
        if charging {
            if percentage >= 99 {
                "Good"
            } else {
                "info"
            }
        } else {
            if percentage <= 5 {
                "Critical"
            } else if percentage <= 15 {
                "Warning"
            } else {
                "Info"
            }
        }
    } else {
        "Idle"
    };

    let text = format!("{:.0}%", percentage);
    let text = if !connected { "Disconnected" } else { &text };
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

fn get_device_path() -> Option<PathBuf> {
    for device in rusb::devices().unwrap().iter() {
        let desc = device.device_descriptor().unwrap();
        if desc.product_id() == PID && desc.vendor_id() == VID {
            let port_numbers = device.port_numbers().unwrap();
            let path = PathBuf::from(format!(
                "/sys/bus/usb/devices/{}-{}:1.3",
                device.bus_number(),
                port_numbers
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(".")
            ));
            return Some(path);
        }
    }

    None
}

fn get_wireless_status() -> Option<bool> {
    let device_path = get_device_path()?;
    Some(
        match fs::read_to_string(device_path.join("wireless_status"))
            .unwrap()
            .trim_end()
        {
            "connected" => true,
            "disconnected" => false,
            status => panic!("unknown wireless status: {}", status),
        },
    )
}

#[derive(Debug)]
struct BatteryInfo {
    charging: bool,
    percentage: u32,
    voltage: u32,
}

fn get_battery() -> Option<BatteryInfo> {
    for power_supply_dir in fs::read_dir("/sys/class/power_supply").unwrap() {
        let power_supply_dir = power_supply_dir.unwrap().path();
        let model_name = fs::read_to_string(power_supply_dir.join("model_name")).unwrap();
        if model_name.trim_end() == "G935 Gaming Headset" {
            return Some(BatteryInfo {
                charging: match fs::read_to_string(power_supply_dir.join("status"))
                    .unwrap()
                    .as_str()
                    .trim_end()
                {
                    "Unknown" => return None,
                    "Discharging" => false,
                    "Charging" => true,
                    status => panic!("unknown battery status: {}", status),
                },
                voltage: fs::read_to_string(power_supply_dir.join("voltage_now"))
                    .unwrap()
                    .trim_end()
                    .parse()
                    .unwrap(),
                percentage: fs::read_to_string(power_supply_dir.join("capacity"))
                    .unwrap()
                    .trim_end()
                    .parse()
                    .unwrap(),
            });
        }
    }

    None
}

fn main() {
    let cli = Cli::parse();

    if let Command::GetI3Status { update_pulseaudio } = cli.command {
        let mut last_connected = true;
        loop {
            let Some(connected) = get_wireless_status() else {
                println!("{{\"text\":\"\"}}");
                sleep(I3_STATUS_INTERVAL);
                continue;
            };
            if update_pulseaudio {
                if connected && !last_connected {
                    pulse_set_card_profile(PULSE_CARD, PULSE_PROFILE);
                } else if !connected && last_connected {
                    pulse_set_card_profile(PULSE_CARD, "off");
                }
                last_connected = connected;
            }

            if connected {
                let battery = get_battery().unwrap();
                println!(
                    "{}",
                    get_i3_status(connected, battery.percentage, battery.charging)
                );
            } else {
                println!("{}", get_i3_status(connected, 0, false));
            }

            sleep(I3_STATUS_INTERVAL);
        }
    }

    let Some(connected) = get_wireless_status() else {
        eprintln!("usb device not found");
        exit(1);
    };
    let Some(battery) = get_battery() else {
        eprintln!("battery not found");
        exit(1);
    };

    if !connected {
        eprintln!("Wireless connection disconnected.");
        exit(1);
    }
    match cli.command {
        Command::GetBatteryVoltage => println!("{}", battery.voltage),
        Command::GetBatteryPercentage => println!("{}", battery.percentage),
        _ => unreachable!(),
    }
    println!("Charging: {}", battery.charging as u8);
}
