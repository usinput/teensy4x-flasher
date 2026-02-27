use anyhow::{anyhow, Result};
use hidapi::{HidApi, HidDevice};
use std::ffi::CString;
use std::time::{Duration, Instant};

const TEENSY_VID: u16 = 0x16C0;
const TEENSY_4X_PID: u16 = 0x0478;

pub const BLOCK_SIZE: usize = 1024;
pub const HEADER_SIZE: usize = 64;
pub const REPORT_SIZE: usize = 1 + HEADER_SIZE + BLOCK_SIZE; // report ID + header + data

pub struct TeensyDevice {
    device: HidDevice,
    path: String,
}

impl TeensyDevice {
    pub fn open() -> Result<Self> {
        let api = HidApi::new()?;

        for info in api.device_list() {
            if info.vendor_id() != TEENSY_VID || info.product_id() != TEENSY_4X_PID {
                continue;
            }

            let path = info.path().to_string_lossy().to_string();
            let device = api.open_path(info.path())?;
            return Ok(TeensyDevice { device, path });
        }

        Err(anyhow!("no Teensy found in bootloader mode"))
    }

    pub fn open_wait(timeout: Duration) -> Result<Self> {
        let start = Instant::now();
        loop {
            match Self::open() {
                Ok(dev) => return Ok(dev),
                Err(_) if start.elapsed() < timeout => {
                    std::thread::sleep(Duration::from_millis(250));
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn write_report(&self, report: &[u8]) -> Result<usize, hidapi::HidError> {
        self.device.write(report)
    }

    // reopen the HID handle after a broken pipe (chip erase invalidates it)
    // tries same path first, falls back to VID/PID scan
    pub fn reopen(&mut self) -> bool {
        if let Ok(api) = HidApi::new() {
            if let Ok(cpath) = CString::new(self.path.clone()) {
                if let Ok(dev) = api.open_path(&cpath) {
                    self.device = dev;
                    return true;
                }
            }

            for info in api.device_list() {
                if info.vendor_id() != TEENSY_VID || info.product_id() != TEENSY_4X_PID {
                    continue;
                }
                if let Ok(dev) = api.open_path(info.path()) {
                    self.path = info.path().to_string_lossy().to_string();
                    self.device = dev;
                    return true;
                }
            }
        }

        false
    }
}

pub fn list_devices() -> Result<Vec<String>> {
    let api = HidApi::new()?;
    let mut serials = Vec::new();

    for info in api.device_list() {
        if info.vendor_id() != TEENSY_VID || info.product_id() != TEENSY_4X_PID {
            continue;
        }
        serials.push(info.serial_number().unwrap_or("unknown").to_string());
    }

    Ok(serials)
}
