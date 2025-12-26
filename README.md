# lid-angle-rs

A Rust command-line tool to display real-time MacBook lid angle using the hidden lid angle sensor.

## Overview

This tool uses the IOKit HID framework to access the MacBook's built-in lid angle sensor and displays the current angle of your laptop's lid in real-time in the terminal.

## Features

- 🔍 Automatic detection of the lid angle sensor
- 📊 Real-time angle display with visual progress bar
- 🎯 High precision (0.01 degree resolution)
- 🚀 Written in Rust for performance and safety

## Requirements

- **macOS 11.0+** (Big Sur or later)
- **Compatible MacBook**: 
  - MacBook Pro 16-inch (2019) or newer
  - MacBook Pro 14-inch/16-inch (M1/M2/M3/M4)
  - MacBook Air (M1/M2/M3)
  - Note: Not all models have this sensor; compatibility varies

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/wangfu91/lid-angle-rs.git
cd lid-angle-rs

# Build the project
cargo build --release

# Run the tool
cargo run --release
```

The compiled binary will be available at `target/release/lid-angle`.

## Usage

Simply run the tool:

```bash
cargo run --release
```

Or if you've installed the binary:

```bash
./target/release/lid-angle
```

The tool will:
1. Search for the lid angle sensor
2. Display real-time angle updates as you open/close your MacBook lid
3. Show a visual progress bar representing the angle (0-180°)

Press `Ctrl+C` to exit.

### Example Output

```
MacBook Lid Angle Sensor Reader
================================

Searching for lid angle sensor...
✓ Lid angle sensor found and opened

Reading lid angle in real-time (press Ctrl+C to exit)...

Lid Angle: 125.43° [█████████████████████████████████████            ]
```

## How It Works

This tool interfaces with macOS's IOKit HID framework to access the lid angle sensor. The sensor is exposed as a HID device with the following properties:

- **VendorID**: 0x05AC (Apple)
- **ProductID**: 0x8104
- **Usage Page**: 0x0020 (Sensor)
- **Usage**: 0x008A (Orientation)

The implementation is based on reverse-engineering work from the [LidAngleSensor](https://github.com/samhenrigold/LidAngleSensor) Objective-C project.

## Troubleshooting

### "No lid angle sensor found"

If you see this error:

1. **Check device compatibility**: Run the diagnostic command:
   ```bash
   hidutil list --matching '{"VendorID":0x5ac,"ProductID":0x8104,"PrimaryUsagePage":32,"PrimaryUsage":138}'
   ```
   If this returns no devices, your MacBook may not have the sensor.

2. **Try with elevated privileges**:
   ```bash
   sudo cargo run --release
   ```

3. **Verify your MacBook model**: The sensor is typically available on MacBook Pro models from 2019 onwards.

### Permission Issues

If you encounter permission errors, you may need to run the tool with `sudo`:

```bash
sudo ./target/release/lid-angle
```

## Credits

This project was inspired by and based on the research from:
- [LidAngleSensor](https://github.com/samhenrigold/LidAngleSensor) by Sam Henri Gold (Objective-C implementation)
- [mac-angle](https://github.com/ufoym/mac-angle) (C++ implementation)

## Technical Details

The tool uses the following Rust crates:
- `core-foundation` and `core-foundation-sys`: For CoreFoundation types and bindings
- Direct FFI bindings to IOKit HID framework

The lid angle sensor reports values as 16-bit integers in units of 0.01 degrees, providing a range from 0° (closed) to ~180° (fully open).

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.