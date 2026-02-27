use anyhow::{anyhow, Context, Result};
use std::time::{Duration, Instant};

use crate::hex::FirmwareImage;
use crate::usb::{self, TeensyDevice};

// timing from PJRC teensy_loader_cli
// first blocks need long timeout because block 0 triggers full chip erase
const ERASE_BLOCK_COUNT: usize = 4;
const ERASE_TIMEOUT: Duration = Duration::from_secs(45);
const WRITE_TIMEOUT: Duration = Duration::from_millis(500);
const RETRY_SLEEP: Duration = Duration::from_millis(10);
const REOPEN_THROTTLE: Duration = Duration::from_millis(100);

pub fn flash(device: &mut TeensyDevice, image: &FirmwareImage) -> Result<()> {
    flash_with_progress(device, image, |_| {})
}

pub fn flash_with_progress(
    device: &mut TeensyDevice,
    image: &FirmwareImage,
    on_block: impl Fn(usize),
) -> Result<()> {
    let total_blocks = image.data.len() / usb::BLOCK_SIZE;
    let mut report = [0u8; usb::REPORT_SIZE];

    for i in 0..total_blocks {
        let offset = i * usb::BLOCK_SIZE;
        let block = &image.data[offset..offset + usb::BLOCK_SIZE];

        // first block must always be sent (triggers chip erase)
        // skip subsequent blocks that are blank (all 0xFF)
        if i > 0 && block.iter().all(|&b| b == 0xFF) {
            continue;
        }

        on_block(i);
        fill_block_report(&mut report, offset, block);
        write_with_retry(device, &report, i).with_context(|| {
            format!(
                "block {} (0x{:08X})",
                i,
                image.base_address as usize + offset
            )
        })?;
    }

    Ok(())
}

pub fn reboot(device: &mut TeensyDevice) -> Result<()> {
    let mut report = [0u8; usb::REPORT_SIZE];
    fill_boot_report(&mut report);

    // device may reboot before we get a response, so errors are expected
    let start = Instant::now();
    let mut last_reopen = Instant::now();

    loop {
        match device.write_report(&report) {
            Ok(_) => return Ok(()),
            Err(_) if start.elapsed() >= WRITE_TIMEOUT => return Ok(()),
            Err(_) => {
                if last_reopen.elapsed() >= REOPEN_THROTTLE {
                    device.reopen();
                    last_reopen = Instant::now();
                }
                std::thread::sleep(RETRY_SLEEP);
            }
        }
    }
}

fn write_with_retry(device: &mut TeensyDevice, report: &[u8], block_index: usize) -> Result<()> {
    let timeout = if block_index <= ERASE_BLOCK_COUNT {
        ERASE_TIMEOUT
    } else {
        WRITE_TIMEOUT
    };

    let start = Instant::now();
    let mut last_reopen = Instant::now();

    loop {
        match device.write_report(report) {
            Ok(n) if n == report.len() => return Ok(()),
            Ok(n) => return Err(anyhow!("short write: {} of {} bytes", n, report.len())),
            Err(e) if start.elapsed() >= timeout => {
                return Err(anyhow!("write timed out after {:?}: {}", timeout, e));
            }
            Err(e) => {
                // chip erase invalidates the HID handle
                if e.to_string().contains("Broken pipe") && last_reopen.elapsed() >= REOPEN_THROTTLE
                {
                    device.reopen();
                    last_reopen = Instant::now();
                }
                std::thread::sleep(RETRY_SLEEP);
            }
        }
    }
}

fn fill_block_report(report: &mut [u8; usb::REPORT_SIZE], addr: usize, data: &[u8]) {
    report.fill(0);
    // byte 0: HID report ID (0x00, stripped by hidapi before sending)
    // bytes 1-3: 24-bit little-endian block address
    // bytes 4-64: zero padding
    // bytes 65-1088: block data
    report[1] = (addr & 0xFF) as u8;
    report[2] = ((addr >> 8) & 0xFF) as u8;
    report[3] = ((addr >> 16) & 0xFF) as u8;
    report[1 + usb::HEADER_SIZE..1 + usb::HEADER_SIZE + data.len()].copy_from_slice(data);
}

fn fill_boot_report(report: &mut [u8; usb::REPORT_SIZE]) {
    report.fill(0);
    report[1] = 0xFF;
    report[2] = 0xFF;
    report[3] = 0xFF;
}
