use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;

use crate::usb;

/// flat memory image ready to flash, aligned to block boundaries
pub struct FirmwareImage {
    pub data: Vec<u8>,
    pub base_address: u32,
}

impl FirmwareImage {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self> {
        let mut extended_address = 0u32;
        let mut segments: Vec<(u32, Vec<u8>)> = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let rec = parse_record(line, line_num + 1)?;

            match rec.record_type {
                0x00 => {
                    let full_address = extended_address | (rec.address as u32);
                    segments.push((full_address, rec.data));
                }
                0x01 => break,
                0x04 => {
                    if rec.data.len() != 2 {
                        return Err(anyhow!(
                            "line {}: extended address must be 2 bytes",
                            line_num + 1
                        ));
                    }
                    extended_address = ((rec.data[0] as u32) << 24) | ((rec.data[1] as u32) << 16);
                }
                0x05 => continue,
                _ => {
                    return Err(anyhow!(
                        "line {}: unsupported record type 0x{:02X}",
                        line_num + 1,
                        rec.record_type
                    ));
                }
            }
        }

        if segments.is_empty() {
            return Err(anyhow!("hex file contains no data"));
        }

        // determine address range
        let base_address = segments.iter().map(|(a, _)| *a).min().unwrap();
        let end_address = segments
            .iter()
            .map(|(a, d)| *a + d.len() as u32)
            .max()
            .unwrap();

        // build flat image, aligned to block size
        let raw_size = (end_address - base_address) as usize;
        let aligned_size = raw_size.div_ceil(usb::BLOCK_SIZE) * usb::BLOCK_SIZE;
        let mut data = vec![0xFFu8; aligned_size];

        for (addr, segment_data) in &segments {
            let offset = (*addr - base_address) as usize;
            data[offset..offset + segment_data.len()].copy_from_slice(segment_data);
        }

        Ok(FirmwareImage { data, base_address })
    }

    pub fn block_count(&self) -> usize {
        self.data.len() / usb::BLOCK_SIZE
    }

    pub fn byte_count(&self) -> usize {
        self.data.len()
    }
}

struct RawRecord {
    address: u16,
    record_type: u8,
    data: Vec<u8>,
}

fn parse_record(line: &str, line_num: usize) -> Result<RawRecord> {
    if !line.starts_with(':') {
        return Err(anyhow!("line {}: missing start code", line_num));
    }

    let hex = &line[1..];
    if hex.len() < 10 {
        return Err(anyhow!("line {}: record too short", line_num));
    }

    let byte_count = parse_hex_u8(hex, 0, line_num)?;
    let address = parse_hex_u16(hex, 2, line_num)?;
    let record_type = parse_hex_u8(hex, 6, line_num)?;

    let expected_len = 8 + (byte_count as usize * 2) + 2;
    if hex.len() < expected_len {
        return Err(anyhow!("line {}: data truncated", line_num));
    }

    let mut data = Vec::with_capacity(byte_count as usize);
    for i in 0..byte_count as usize {
        data.push(parse_hex_u8(hex, 8 + i * 2, line_num)?);
    }

    let checksum = parse_hex_u8(hex, 8 + byte_count as usize * 2, line_num)?;

    let mut sum: u8 = byte_count;
    sum = sum.wrapping_add((address >> 8) as u8);
    sum = sum.wrapping_add((address & 0xFF) as u8);
    sum = sum.wrapping_add(record_type);
    for &b in &data {
        sum = sum.wrapping_add(b);
    }
    sum = sum.wrapping_add(checksum);

    if sum != 0 {
        return Err(anyhow!("line {}: checksum mismatch", line_num));
    }

    Ok(RawRecord {
        address,
        record_type,
        data,
    })
}

fn parse_hex_u8(hex: &str, offset: usize, line_num: usize) -> Result<u8> {
    u8::from_str_radix(&hex[offset..offset + 2], 16)
        .map_err(|_| anyhow!("line {}: invalid hex at offset {}", line_num, offset))
}

fn parse_hex_u16(hex: &str, offset: usize, line_num: usize) -> Result<u16> {
    u16::from_str_radix(&hex[offset..offset + 4], 16)
        .map_err(|_| anyhow!("line {}: invalid hex at offset {}", line_num, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_hex() {
        let hex = ":0200000460009A\n:100000004643464200000156000000000103030081\n:00000001FF\n";
        let image = FirmwareImage::parse(hex).unwrap();
        assert_eq!(image.base_address, 0x60000000);
        assert_eq!(image.data[0..4], [0x46, 0x43, 0x46, 0x42]);
    }

    #[test]
    fn test_invalid_checksum() {
        assert!(FirmwareImage::parse(":020000040000FB\n").is_err());
    }

    #[test]
    fn test_missing_start_code() {
        assert!(FirmwareImage::parse("020000040000FA\n").is_err());
    }

    #[test]
    fn test_empty_hex() {
        assert!(FirmwareImage::parse(":00000001FF\n").is_err());
    }

    #[test]
    fn test_block_alignment() {
        let hex = ":0200000460009A\n:100000004643464200000156000000000103030081\n:00000001FF\n";
        let image = FirmwareImage::parse(hex).unwrap();
        assert_eq!(image.data.len() % usb::BLOCK_SIZE, 0);
    }
}
