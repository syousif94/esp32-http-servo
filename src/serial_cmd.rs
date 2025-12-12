use esp_println::println;
use esp_hal::uart::Uart;
use esp_hal::Blocking;
use embassy_time::{Duration, Timer};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

/// Signal for servo angle updates from serial
pub static SERIAL_SERVO_ANGLE: Signal<CriticalSectionRawMutex, u8> = Signal::new();

/// Parse a servo command from input
/// Accepts formats like: "90", "servo 90", "angle 90", "s90", "a90"
fn parse_servo_command(input: &str) -> Option<u8> {
    let input = input.trim();
    
    // Try direct number
    if let Ok(angle) = input.parse::<u8>() {
        if angle <= 180 {
            return Some(angle);
        }
    }
    
    // Try "servo X" or "s X" or "sX"
    for prefix in ["servo ", "angle ", "s ", "a ", "s", "a"] {
        if let Some(rest) = input.strip_prefix(prefix) {
            if let Ok(angle) = rest.trim().parse::<u8>() {
                if angle <= 180 {
                    return Some(angle);
                }
            }
        }
    }
    
    None
}

/// Task to read serial input and parse servo commands
#[embassy_executor::task]
pub async fn serial_input_task(mut uart: Uart<'static, Blocking>) {
    println!("Serial command interface ready");
    println!("  Commands: <angle> or 'servo <angle>' (0-180)");
    println!("  Example: 90");
    
    let mut buffer = [0u8; 64];
    let mut pos = 0usize;
    let mut read_buf = [0u8; 1];
    
    loop {
        // Check if data is available (non-blocking check)
        if uart.read_ready() {
            // Try to read a byte
            match uart.read(&mut read_buf) {
                Ok(1) => {
                    let byte = read_buf[0];
                    
                    // Echo the character back
                    let _ = uart.write(&[byte]);
                    
                    if byte == b'\r' || byte == b'\n' {
                        if pos > 0 {
                            // Try to parse the command
                            if let Ok(cmd) = core::str::from_utf8(&buffer[..pos]) {
                                if let Some(angle) = parse_servo_command(cmd) {
                                    println!("\nSerial: Setting servo to {} degrees", angle);
                                    SERIAL_SERVO_ANGLE.signal(angle);
                                } else if !cmd.trim().is_empty() {
                                    println!("\nUnknown command: '{}'. Use 0-180 for angle.", cmd);
                                }
                            }
                            pos = 0;
                        }
                        println!("");
                    } else if pos < buffer.len() - 1 {
                        buffer[pos] = byte;
                        pos += 1;
                    }
                }
                _ => {}
            }
        } else {
            // No data available, yield to other tasks
            Timer::after(Duration::from_millis(10)).await;
        }
    }
}
