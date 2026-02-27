# Teensy Flasher

cross-platform CLI tool for flashing firmware onto Teensy 4.0/4.1 microcontrollers via USB

## usage

```bash
# flash firmware (device must be in bootloader mode)
flasher flash firmware.hex

# wait for device to appear, then flash
flasher flash -w firmware.hex

# wait with custom timeout (default 30s)
flasher flash -w -t 60 firmware.hex

# list devices in bootloader mode (prints serial numbers)
flasher list
```

exit codes: `0` success, `1` error

## build

```bash
cargo build --release -p flasher
```

## platform setup

### Linux

```bash
echo 'SUBSYSTEMS=="usb", ATTRS{idVendor}=="16c0", MODE="0666"' | sudo tee /etc/udev/rules.d/99-teensy.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

### macOS

no setup required

### Windows

no setup required for HID devices

## entering bootloader mode

1. press the button on the Teensy board
2. LED should blink slowly
3. run `flasher list` to confirm detection

## technical details

uses the HalfKay bootloader protocol over USB HID (hidapi):

- packet: 1 byte report ID + 64 byte header + 1024 byte data = 1089 bytes
- header: 24-bit LE address in bytes 0-2, rest zero
- boot: address bytes set to 0xFF, rest zero
- first block triggers chip erase (45s timeout)
- subsequent blocks: 500ms timeout
- automatic retry with device reopen on broken pipe

## development

```bash
cargo test -p flasher
```

the vendored `blink-test.hex` blinks the LED in a 3-fast-blink pattern.
source: `test-firmware/src/main.cpp`.
