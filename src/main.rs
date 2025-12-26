#![cfg_attr(not(target_os = "macos"), allow(dead_code, unused_imports))]

use core_foundation::base::{CFRelease, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef, CFMutableDictionary};
use core_foundation::number::CFNumber;
use core_foundation::set::CFSet;
use core_foundation::string::CFString;
use core_foundation_sys::base::{CFIndex, CFTypeRef, kCFAllocatorDefault};
use std::io::Write;
use std::os::raw::{c_void, c_int};
use std::thread;
use std::time::Duration;

// IOKit HID constants
const K_IO_HID_VENDOR_ID_KEY: &str = "VendorID";
const K_IO_HID_PRODUCT_ID_KEY: &str = "ProductID";
const K_IO_HID_PRIMARY_USAGE_PAGE_KEY: &str = "PrimaryUsagePage";
const K_IO_HID_PRIMARY_USAGE_KEY: &str = "PrimaryUsage";

// Lid angle sensor identifiers (from research)
const LID_ANGLE_VENDOR_ID: i32 = 0x05AC; // Apple
const LID_ANGLE_PRODUCT_ID: i32 = 0x8104;
const LID_ANGLE_USAGE_PAGE: i32 = 0x0020; // Sensor
const LID_ANGLE_USAGE: i32 = 0x008A; // Orientation

// IOKit HID types
type IOHIDManagerRef = *mut c_void;
type IOHIDDeviceRef = *mut c_void;
type IOReturn = c_int;

#[repr(C)]
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
    fn IOHIDManagerCreate(
        allocator: CFTypeRef,
        options: IOHIDOptionsType,
    ) -> IOHIDManagerRef;
    
    fn IOHIDManagerSetDeviceMatching(
        manager: IOHIDManagerRef,
        matching: CFDictionaryRef,
    );
    
    fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFTypeRef;
    
    fn IOHIDManagerOpen(
        manager: IOHIDManagerRef,
        options: IOHIDOptionsType,
    ) -> IOReturn;
    
    fn IOHIDDeviceOpen(
        device: IOHIDDeviceRef,
        options: IOHIDOptionsType,
    ) -> IOReturn;
    
    fn IOHIDDeviceGetReport(
        device: IOHIDDeviceRef,
        report_type: IOHIDReportType,
        report_id: CFIndex,
        report: *mut u8,
        report_length: *mut CFIndex,
    ) -> IOReturn;
    
    fn IOHIDDeviceClose(
        device: IOHIDDeviceRef,
        options: IOHIDOptionsType,
    ) -> IOReturn;
}

#[cfg(target_os = "macos")]
fn create_matching_dictionary() -> CFDictionary {
    let vendor_id = CFNumber::from(LID_ANGLE_VENDOR_ID);
    let product_id = CFNumber::from(LID_ANGLE_PRODUCT_ID);
    let usage_page = CFNumber::from(LID_ANGLE_USAGE_PAGE);
    let usage = CFNumber::from(LID_ANGLE_USAGE);
    
    let dict = CFMutableDictionary::new();
    
    dict.set(
        CFString::from_static_string(K_IO_HID_VENDOR_ID_KEY).as_CFType(),
        vendor_id.as_CFType(),
    );
    dict.set(
        CFString::from_static_string(K_IO_HID_PRODUCT_ID_KEY).as_CFType(),
        product_id.as_CFType(),
    );
    dict.set(
        CFString::from_static_string(K_IO_HID_PRIMARY_USAGE_PAGE_KEY).as_CFType(),
        usage_page.as_CFType(),
    );
    dict.set(
        CFString::from_static_string(K_IO_HID_PRIMARY_USAGE_KEY).as_CFType(),
        usage.as_CFType(),
    );
    
    dict.to_immutable()
}

#[cfg(target_os = "macos")]
fn find_lid_angle_device() -> Option<IOHIDDeviceRef> {
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
        
        let devices_set = CFSet::wrap_under_create_rule(devices_set_ref as *const c_void);
        
        if devices_set.len() == 0 {
            eprintln!("No lid angle sensor found. This feature may not be supported on your MacBook.");
            CFRelease(manager as CFTypeRef);
            return None;
        }
        
        // Get the first device (safe because we checked len() > 0 above)
        let device = *devices_set.get_values().first().unwrap() as IOHIDDeviceRef;
        
        // Open the device
        let open_result = IOHIDDeviceOpen(device, IOHIDOptionsType::None);
        if open_result != 0 {
            eprintln!("Failed to open HID device: {}", open_result);
            CFRelease(manager as CFTypeRef);
            return None;
        }
        
        println!("✓ Lid angle sensor found and opened");
        
        Some(device)
    }
}

#[cfg(target_os = "macos")]
fn read_lid_angle(device: IOHIDDeviceRef) -> Option<f32> {
    unsafe {
        let mut report: [u8; 256] = [0; 256];
        let mut report_length: CFIndex = report.len() as CFIndex;
        
        let result = IOHIDDeviceGetReport(
            device,
            IOHIDReportType::Feature,
            0,
            report.as_mut_ptr(),
            &mut report_length,
        );
        
        if result != 0 {
            return None;
        }
        
        // The lid angle is typically a 16-bit value at a specific offset
        // Based on reverse engineering, it's often at offset 1-2
        if report_length >= 3 {
            let raw_value = u16::from_le_bytes([report[1], report[2]]);
            // The value is in units of 0.01 degrees
            let angle = raw_value as f32 / 100.0;
            return Some(angle);
        }
        
        None
    }
}

fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("Error: This tool only works on macOS.");
        eprintln!("The MacBook lid angle sensor is only available on macOS systems.");
        std::process::exit(1);
    }
    
    #[cfg(target_os = "macos")]
    {
        println!("MacBook Lid Angle Sensor Reader");
        println!("================================\n");
        
        println!("Searching for lid angle sensor...");
        
        let device = match find_lid_angle_device() {
            Some(dev) => dev,
            None => {
                eprintln!("\nError: Could not find or open lid angle sensor.");
                eprintln!("This tool requires a compatible MacBook (2019 16-inch MacBook Pro or newer).");
                eprintln!("\nTroubleshooting:");
                eprintln!("1. Make sure you're running on a compatible MacBook");
                eprintln!("2. Try running with sudo: sudo lid-angle");
                eprintln!("3. Check if the sensor exists by running:");
                eprintln!("   hidutil list --matching '{{\"VendorID\":0x5ac,\"ProductID\":0x8104,\"PrimaryUsagePage\":32,\"PrimaryUsage\":138}}'");
                std::process::exit(1);
            }
        };
        
        println!("\nReading lid angle in real-time (press Ctrl+C to exit)...\n");
        
        let mut last_angle: Option<f32> = None;
        
        loop {
            if let Some(angle) = read_lid_angle(device) {
                // Only print if angle changed significantly (to reduce console spam)
                let should_print = match last_angle {
                    None => true,
                    Some(last) => (angle - last).abs() > 0.5,
                };
                
                if should_print {
                    // Create a visual bar representation
                    // Clamp bar_length to prevent overflow if angle > 180
                    let bar_length = ((angle / 180.0 * 50.0) as usize).min(50);
                    let bar = "█".repeat(bar_length);
                    
                    print!("\r");
                    print!("Lid Angle: {:6.2}° [{}{}] ",
                        angle,
                        bar,
                        " ".repeat(50 - bar_length)
                    );
                    
                    // Ignore flush errors (e.g., if stdout is closed)
                    let _ = std::io::stdout().flush();
                    
                    last_angle = Some(angle);
                }
            }
            
            thread::sleep(Duration::from_millis(100));
        }
    }
}
