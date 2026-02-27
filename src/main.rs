mod halfkay;
mod hex;
mod usb;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use hex::FirmwareImage;
use usb::TeensyDevice;

#[derive(Parser)]
#[command(name = "flasher")]
#[command(about = "flash firmware onto Teensy 4.x microcontrollers")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// flash a hex file onto the connected Teensy
    Flash {
        /// path to the Intel HEX file
        hex_file: PathBuf,

        /// wait for device to appear in bootloader mode
        #[arg(short, long)]
        wait: bool,

        /// how long to wait for device in seconds (default: 30)
        #[arg(short, long, default_value = "30")]
        timeout: u64,
    },

    /// list Teensy devices in bootloader mode
    List,
}

fn main() {
    let cli = Cli::parse();

    let code = match cli.command {
        Command::Flash {
            hex_file,
            wait,
            timeout,
        } => run_flash(hex_file, wait, timeout),
        Command::List => run_list(),
    };

    process::exit(code);
}

fn run_flash(hex_file: PathBuf, wait: bool, timeout_secs: u64) -> i32 {
    if let Err(e) = flash(hex_file, wait, timeout_secs) {
        eprintln!("error: {:#}", e);
        return 1;
    }
    0
}

fn flash(hex_file: PathBuf, wait: bool, timeout_secs: u64) -> Result<()> {
    let image = FirmwareImage::from_file(&hex_file)
        .with_context(|| format!("failed to read {}", hex_file.display()))?;

    eprintln!(
        "{}: {} bytes, {} blocks",
        hex_file.display(),
        image.byte_count(),
        image.block_count(),
    );

    let mut device = if wait {
        let timeout = Duration::from_secs(timeout_secs);
        eprintln!("waiting for device ({}s timeout)...", timeout_secs);
        TeensyDevice::open_wait(timeout).context("device not found")?
    } else {
        TeensyDevice::open().context("device not found")?
    };

    eprintln!("flashing...");
    halfkay::flash(&mut device, &image).context("flash failed")?;
    halfkay::reboot(&mut device).context("reboot failed")?;
    eprintln!("done");

    Ok(())
}

fn run_list() -> i32 {
    match usb::list_devices() {
        Ok(serials) if serials.is_empty() => {
            println!("no devices found");
            0
        }
        Ok(serials) => {
            for serial in &serials {
                println!("{}", serial);
            }
            0
        }
        Err(e) => {
            eprintln!("error: {:#}", e);
            1
        }
    }
}
