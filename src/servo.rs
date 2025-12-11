use esp_hal::ledc::{
    channel::{self, ChannelIFace},
    timer::{self, TimerIFace, config::Duty},
    Ledc, HighSpeed,
};
use esp_hal::gpio::{DriveMode, interconnect::PeripheralOutput};
use esp_println::println;

/// SG90 servo configuration
/// - PWM frequency: 50Hz (20ms period)
/// - Pulse width: 0.5ms (0°) to 2.5ms (180°)
const SERVO_FREQ_HZ: u32 = 50;

/// Minimum pulse width in microseconds (0 degrees)
const MIN_PULSE_US: u32 = 500;

/// Maximum pulse width in microseconds (180 degrees)
const MAX_PULSE_US: u32 = 2500;

/// Period in microseconds (1/50Hz = 20000us)
const PERIOD_US: u32 = 1_000_000 / SERVO_FREQ_HZ;

/// Servo controller using LEDC PWM
pub struct ServoController<'d> {
    channel: channel::Channel<'d, HighSpeed>,
}

impl<'d> ServoController<'d> {
    /// Create a new servo controller
    pub fn new<P: PeripheralOutput<'d>>(
        timer: &'d timer::Timer<'d, HighSpeed>,
        pin: P,
    ) -> Self {
        println!("Initializing servo controller (HighSpeed LEDC)");
        println!("  PWM frequency: {} Hz", SERVO_FREQ_HZ);
        println!("  Period: {} us", PERIOD_US);
        println!("  Pulse range: {} - {} us", MIN_PULSE_US, MAX_PULSE_US);
        
        let mut channel = channel::Channel::new(channel::Number::Channel0, pin);
        channel.configure(channel::config::Config {
            timer,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        }).unwrap();
        
        Self { channel }
    }

    /// Set servo angle (0-180 degrees)
    pub fn set_angle(&mut self, angle: u8) {
        let angle = angle.min(180);
        
        // Calculate pulse width for the given angle
        let pulse_us = MIN_PULSE_US + ((MAX_PULSE_US - MIN_PULSE_US) * angle as u32) / 180;
        
        // Convert pulse width to duty percentage (0-100)
        let duty_pct = ((pulse_us * 100) / PERIOD_US) as u8;
        
        println!("Servo: angle={}° pulse={}us duty={}%", angle, pulse_us, duty_pct);
        
        self.channel.set_duty(duty_pct).unwrap();
    }
}

/// Initialize LEDC timer for servo control (HighSpeed for better precision)
pub fn init_servo_timer<'d>(ledc: &'d Ledc<'d>) -> timer::Timer<'d, HighSpeed> {
    let mut timer = ledc.timer::<HighSpeed>(timer::Number::Timer0);
    timer.configure(timer::config::Config {
        duty: Duty::Duty14Bit,
        clock_source: timer::HSClockSource::APBClk,
        frequency: esp_hal::time::Rate::from_hz(SERVO_FREQ_HZ),
    }).unwrap();
    timer
}
