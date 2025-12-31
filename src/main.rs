#![cfg_attr(not(target_os = "macos"), allow(dead_code, unused_imports))]

use core_foundation::base::{CFRelease, CFRetain, CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef, CFMutableDictionary};
use core_foundation::number::CFNumber;
use core_foundation::set::{CFSet, CFSetRef};
use core_foundation::string::CFString;
use core_foundation_sys::base::{kCFAllocatorDefault, CFIndex, CFTypeRef};
use core_foundation_sys::set::CFSetGetValues;
use std::io::Write;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::thread;
use std::time::Duration;

// IOKit HID constants
const K_IO_HID_VENDOR_ID_KEY: &str = "VendorID";
const K_IO_HID_PRODUCT_ID_KEY: &str = "ProductID";
const K_IO_HID_USAGE_PAGE_KEY: &str = "UsagePage";
const K_IO_HID_USAGE_KEY: &str = "Usage";

// Lid angle sensor identifiers (from research)
const LID_ANGLE_VENDOR_ID: i32 = 0x05AC; // Apple
const LID_ANGLE_PRODUCT_ID: i32 = 0x8104;
const LID_ANGLE_USAGE_PAGE: i32 = 0x0020; // Sensor
const LID_ANGLE_USAGE: i32 = 0x008A; // Orientation
const LID_ANGLE_REPORT_ID_PRIMARY: CFIndex = 1; // Primary report ID observed in working Objective-C sample
const LID_ANGLE_REPORT_ID_FALLBACK: CFIndex = 0; // Fallback if 1 fails
const DISPLAY_WIDTH: usize = 50; // Width of the printed bar
const MIN_PRINT_DELTA_DEGREES: f32 = 0.5; // Smallest difference before printing
const READ_INTERVAL_MS: u64 = 100; // Poll interval

// IOKit HID types
type IOHIDManagerRef = *mut c_void;
type IOHIDDeviceRef = *mut c_void;
type IOReturn = c_int;

#[derive(Clone, Copy)]
struct HidDevice {
    device: IOHIDDeviceRef,
    report_id: CFIndex,
}

#[repr(C)]
#[allow(dead_code)]
enum IOHIDReportType {
    Input = 0,
    Output = 1,
    Feature = 2,
}

#[repr(C)]
enum IOHIDOptionsType {
    None = 0,
}

// External IOKit HID functions
#[cfg(target_os = "macos")]
#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOHIDManagerCreate(allocator: CFTypeRef, options: IOHIDOptionsType) -> IOHIDManagerRef;

    fn IOHIDManagerSetDeviceMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);

    fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFTypeRef;

    fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: IOHIDOptionsType) -> IOReturn;

    fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: IOHIDOptionsType) -> IOReturn;

    fn IOHIDDeviceGetReport(
        device: IOHIDDeviceRef,
        report_type: IOHIDReportType,
        report_id: CFIndex,
        report: *mut u8,
        report_length: *mut CFIndex,
    ) -> IOReturn;

    fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: IOHIDOptionsType) -> IOReturn;
}

#[cfg(target_os = "macos")]
fn create_matching_dictionary() -> CFDictionary<CFType, CFType> {
    let vendor_id = CFNumber::from(LID_ANGLE_VENDOR_ID);
    let product_id = CFNumber::from(LID_ANGLE_PRODUCT_ID);
    let usage_page = CFNumber::from(LID_ANGLE_USAGE_PAGE);
    let usage = CFNumber::from(LID_ANGLE_USAGE);

    let mut dict = CFMutableDictionary::new();

    dict.set(
        CFString::from_static_string(K_IO_HID_VENDOR_ID_KEY).as_CFType(),
        vendor_id.as_CFType(),
    );
    dict.set(
        CFString::from_static_string(K_IO_HID_PRODUCT_ID_KEY).as_CFType(),
        product_id.as_CFType(),
    );
    dict.set(
        CFString::from_static_string(K_IO_HID_USAGE_PAGE_KEY).as_CFType(),
        usage_page.as_CFType(),
    );
    dict.set(
        CFString::from_static_string(K_IO_HID_USAGE_KEY).as_CFType(),
        usage.as_CFType(),
    );

    dict.to_immutable()
}

#[cfg(target_os = "macos")]
fn find_lid_angle_device() -> Option<HidDevice> {
    unsafe {
        let manager = IOHIDManagerCreate(kCFAllocatorDefault, IOHIDOptionsType::None);
        if manager.is_null() {
            eprintln!("Failed to create IOHIDManager");
            return None;
        }

        let matching_dict = create_matching_dictionary();
        IOHIDManagerSetDeviceMatching(manager, matching_dict.as_concrete_TypeRef());

        let result = IOHIDManagerOpen(manager, IOHIDOptionsType::None);
        if result != 0 {
            eprintln!("Failed to open IOHIDManager: {}", result);
            CFRelease(manager as CFTypeRef);
            return None;
        }

        let devices_set_ref = IOHIDManagerCopyDevices(manager);
        if devices_set_ref.is_null() {
            eprintln!("No devices found matching criteria");
            CFRelease(manager as CFTypeRef);
            return None;
        }

        let devices_set: CFSet<*const c_void> =
            CFSet::wrap_under_create_rule(devices_set_ref as CFSetRef);

        if devices_set.is_empty() {
            eprintln!(
                "No lid angle sensor found. This feature may not be supported on your MacBook."
            );
            CFRelease(manager as CFTypeRef);
            return None;
        }

        println!("Found {} matching HID device(s)", devices_set.len());

        let mut device_values: Vec<*const c_void> = vec![ptr::null(); devices_set.len()];
        CFSetGetValues(
            devices_set.as_concrete_TypeRef(),
            device_values.as_mut_ptr(),
        );

        for (idx, dev) in device_values.iter().enumerate() {
            if dev.is_null() {
                continue;
            }
            let device = *dev as IOHIDDeviceRef;

            let open_result = IOHIDDeviceOpen(device, IOHIDOptionsType::None);
            if open_result != 0 {
                eprintln!("Device {} failed to open: {}", idx, open_result);
                continue;
            }

            let mut report: [u8; 8] = [0; 8];
            let mut report_length: CFIndex = report.len() as CFIndex;
            let primary_result = IOHIDDeviceGetReport(
                device,
                IOHIDReportType::Feature,
                LID_ANGLE_REPORT_ID_PRIMARY,
                report.as_mut_ptr(),
                &mut report_length,
            );

            let mut chosen_report_id = None;

            if primary_result == 0 && report_length >= 3 {
                chosen_report_id = Some(LID_ANGLE_REPORT_ID_PRIMARY);
            } else {
                // Try fallback report ID 0
                report.fill(0);
                report_length = report.len() as CFIndex;
                let fallback_result = IOHIDDeviceGetReport(
                    device,
                    IOHIDReportType::Feature,
                    LID_ANGLE_REPORT_ID_FALLBACK,
                    report.as_mut_ptr(),
                    &mut report_length,
                );
                if fallback_result == 0 && report_length >= 3 {
                    chosen_report_id = Some(LID_ANGLE_REPORT_ID_FALLBACK);
                } else {
                    eprintln!(
                        "Device {} failed to read report (primary res {}, len {}; fallback res {}, len {})",
                        idx, primary_result, report_length, fallback_result, report_length
                    );
                }
            }

            if let Some(id) = chosen_report_id {
                // Close the device after testing (like Objective-C code does)
                IOHIDDeviceClose(device, IOHIDOptionsType::None);
                // Keep the device alive after the device set is released.
                CFRetain(device as CFTypeRef);
                println!("✓ Using HID device {} (report id {})", idx, id);
                CFRelease(manager as CFTypeRef);
                return Some(HidDevice {
                    device,
                    report_id: id,
                });
            }

            IOHIDDeviceClose(device, IOHIDOptionsType::None);
        }

        CFRelease(manager as CFTypeRef);
        eprintln!("No usable lid angle HID device found.");
        None
    }
}

#[cfg(target_os = "macos")]
fn read_lid_angle(hid_device: HidDevice) -> Option<f32> {
    unsafe {
        let mut report: [u8; 8] = [0; 8];
        let mut report_length: CFIndex = report.len() as CFIndex;

        let result = IOHIDDeviceGetReport(
            hid_device.device,
            IOHIDReportType::Feature,
            hid_device.report_id,
            report.as_mut_ptr(),
            &mut report_length,
        );

        if result != 0 || report_length < 3 {
            return None;
        }

        let raw_value = u16::from_le_bytes([report[1], report[2]]);
        Some(raw_value as f32)
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Error: This tool only works on macOS.");
    eprintln!("The MacBook lid angle sensor is only available on macOS systems.");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
fn main() {
    run();
}

#[cfg(target_os = "macos")]
fn run() {
    println!("MacBook Lid Angle Sensor Reader");
    println!("================================\n");

    println!("Searching for lid angle sensor...");

    let device = match find_lid_angle_device() {
        Some(dev) => dev,
        None => {
            eprintln!("\nError: Could not find or open lid angle sensor.");
            eprintln!(
                "This tool requires a compatible MacBook (2019 16-inch MacBook Pro or newer)."
            );
            eprintln!("\nTroubleshooting:");
            eprintln!("1. Make sure you're running on a compatible MacBook");
            eprintln!("2. Try running with sudo: sudo lid-angle");
            eprintln!("3. Check if the sensor exists by running:");
            eprintln!("   hidutil list --matching '{{\"VendorID\":0x5ac,\"ProductID\":0x8104,\"UsagePage\":32,\"Usage\":138}}'");
            std::process::exit(1);
        }
    };

    // Open the device for reading (like Objective-C init does after findLidAngleSensor)
    unsafe {
        let open_result = IOHIDDeviceOpen(device.device, IOHIDOptionsType::None);
        if open_result != 0 {
            eprintln!("Failed to open HID device: {}", open_result);
            std::process::exit(1);
        }
    }

    println!("\nReading lid angle in real-time (press Ctrl+C to exit)...\n");

    let mut last_angle: Option<f32> = None;

    loop {
        if let Some(angle) = read_lid_angle(device) {
            // Only print if angle changed significantly (to reduce console spam)
            let should_print = match last_angle {
                None => true,
                Some(last) => (angle - last).abs() > MIN_PRINT_DELTA_DEGREES,
            };

            if should_print {
                // Create a visual bar representation
                // Clamp bar_length to prevent overflow if angle > 180
                let bar_length =
                    ((angle / 180.0 * DISPLAY_WIDTH as f32) as usize).min(DISPLAY_WIDTH);
                let bar = "█".repeat(bar_length);

                print!("\r");
                print!(
                    "Lid Angle: {:6.2}° [{}{}] ",
                    angle,
                    bar,
                    " ".repeat(DISPLAY_WIDTH - bar_length)
                );

                // Ignore flush errors (e.g., if stdout is closed)
                let _ = std::io::stdout().flush();

                last_angle = Some(angle);
            }
        }

        thread::sleep(Duration::from_millis(READ_INTERVAL_MS));
    }
}
