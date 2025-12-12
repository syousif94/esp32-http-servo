# ESP32 HTTP Servo Controller

Control an SG90 servo motor via HTTP requests or serial commands using an ESP32 microcontroller, written in Rust with `no_std` embedded development.

## Features

- **HTTP Control**: Set servo angle via GET requests (`/servo/90` or `/servo?angle=90`)
- **Serial Control**: Type angle values directly in the serial monitor
- **WiFi Connected**: Connects to your WiFi network and serves HTTP on port 80
- **Async Runtime**: Uses Embassy for efficient async/await embedded programming

## Hardware Requirements

- **ESP32** development board (tested with ESP32-WROOM)
- **SG90 Servo Motor** (or compatible PWM servo)

### Wiring

| Servo Wire             | Connection              |
| ---------------------- | ----------------------- |
| Red (VCC)              | 3.3/5V power supply     |
| Brown/Black (GND)      | GND (shared with ESP32) |
| Orange/Yellow (Signal) | GPIO18                  |

## Software Requirements

### Install Rust and ESP32 Toolchain

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install espup (ESP32 Rust toolchain installer)
cargo install espup

# Install the ESP32 toolchain
espup install

# Source the environment (add to your shell profile)
source ~/export-esp.sh

# Install cargo-espflash for flashing
cargo install cargo-espflash
```

### Configure WiFi

Edit `cfg.toml` with your WiFi credentials:

```toml
wifi_ssid = "YourNetworkName"
wifi_password = "YourPassword"
```

## Building and Flashing

```bash
cargo espflash flash --monitor
```

## Usage

### HTTP Control

Once connected, the ESP32 will print its IP address. Use curl or a browser:

```bash
# Move to 90 degrees (center)
curl http://192.168.x.x/servo/90

# Move to 0 degrees
curl http://192.168.x.x/servo/0

# Move to 180 degrees
curl http://192.168.x.x/servo/180

# Alternative query string format
curl http://192.168.x.x/servo?angle=45

# Check server status
curl http://192.168.x.x/
```

**Response format** (JSON):

```json
{ "angle": 90 }
```

### Serial Control

While connected via `cargo espflash flash --monitor`, type commands directly:

```
90        # Move to 90 degrees
0         # Move to 0 degrees
180       # Move to 180 degrees
servo 45  # Also works
```

## Project Structure

```
src/
├── bin/
│   └── main.rs        # Entry point, WiFi setup, main loop
├── lib.rs             # Library root
├── http_server.rs     # HTTP server and request handling
├── serial_cmd.rs      # Serial command parsing
└── servo.rs           # PWM servo control using LEDC
```

## How It Works

### Servo Control (`servo.rs`)

The servo is controlled using the ESP32's **LEDC (LED Control)** peripheral, which generates PWM signals:

- **Frequency**: 50 Hz (20ms period) - standard for hobby servos
- **Duty Cycle**:
  - 0° → 0.5ms pulse (2.5% duty)
  - 90° → 1.5ms pulse (7.5% duty)
  - 180° → 2.5ms pulse (12.5% duty)
- **Resolution**: 14-bit for precise angle control
- **Timer**: HighSpeed LEDC timer with 80MHz APB clock

### HTTP Server (`http_server.rs`)

A simple async TCP server running on port 80:

1. Accepts TCP connections
2. Parses HTTP GET requests
3. Extracts angle from URL path or query string
4. Signals the main loop via `embassy_sync::Signal`
5. Returns JSON response

**Endpoints**:

- `GET /` - Server status
- `GET /health` - Health check
- `GET /servo/<angle>` - Set servo angle (0-180)
- `GET /servo?angle=<angle>` - Alternative format

### Serial Commands (`serial_cmd.rs`)

Polls UART0 for input and parses simple commands:

- Runs as an Embassy task
- Non-blocking read with `read_ready()` check
- Echoes characters back to terminal
- Parses numbers or `servo <angle>` format

### Main Loop (`main.rs`)

1. Initializes peripherals (LEDC, UART, WiFi)
2. Connects to WiFi network
3. Spawns background tasks:
   - WiFi connection manager
   - Network stack runner
   - HTTP server
   - Serial command handler
4. Main loop uses `embassy_futures::select` to wait for angle updates from either HTTP or serial, then moves the servo

## Async Execution Model

### Is the main loop executing every tick?

**No.** The main loop is _not_ polling or running on a fixed tick. It **sleeps** until woken by an event.

```rust
loop {
    let angle = match select(SERVO_ANGLE.wait(), SERIAL_SERVO_ANGLE.wait()).await {
        Either::First(angle) => angle,
        Either::Second(angle) => angle,
    };
    servo.set_angle(angle);
}
```

When execution hits `.await`, the task **yields** and the CPU can sleep or run other tasks. The main task only wakes when:

- The HTTP server signals a new angle via `SERVO_ANGLE.signal(angle)`
- The serial handler signals via `SERIAL_SERVO_ANGLE.signal(angle)`

### How `select()` Works

`select()` is a **future combinator** that polls both futures concurrently:

1. Both `SERVO_ANGLE.wait()` and `SERIAL_SERVO_ANGLE.wait()` are checked
2. When **either** completes, `select()` returns immediately with that result
3. The other future is dropped (but its signal remains for next iteration)

This is **not busy-polling**. If neither signal is ready, the executor puts the task to sleep.

### Embassy Executor Model

Embassy uses a **single-threaded, cooperative, interrupt-driven** scheduler:

| Concept              | Description                                           |
| -------------------- | ----------------------------------------------------- |
| **Cooperative**      | Tasks voluntarily yield at `.await` points            |
| **Single-threaded**  | One task runs at a time (no preemption between tasks) |
| **Interrupt-driven** | Hardware interrupts wake sleeping tasks               |
| **No fixed tick**    | Wakeups happen on-demand, not periodically            |

**Task lifecycle:**

1. Task runs until it hits `.await` on a pending future
2. Task yields to executor and goes to sleep
3. Hardware interrupt fires (timer, UART RX, WiFi packet, etc.)
4. Interrupt handler calls `Waker::wake()` to mark task as ready
5. Executor polls the task again

### Example Flow: HTTP Request → Servo Movement

```
1. WiFi packet arrives       → Hardware interrupt
2. Interrupt wakes net_task  → Executor runs net_task
3. net_task processes packet → Wakes http_server_task
4. HTTP server parses URL    → Calls SERVO_ANGLE.signal(90)
5. signal() wakes main task  → Executor runs main task
6. select() returns angle    → servo.set_angle(90)
7. Main task loops, awaits   → Goes back to sleep
```

The CPU spends most of its time **sleeping**. It wakes only for interrupts, does minimal work, then sleeps again. This is extremely power-efficient.

### RTOS Timing

There is no fixed "tick rate" like traditional RTOS (e.g., FreeRTOS 1ms tick). Instead:

- **Timers**: `Timer::after(Duration::from_millis(500))` sets a hardware timer interrupt; the executor sleeps until it fires
- **Signals**: Wake immediately when `signal()` is called from another task
- **I/O**: UART/WiFi interrupts wake tasks when data arrives

The ESP32's timer peripheral runs at **80 MHz APB clock**, providing microsecond-level precision for timing operations.

## Dependencies

Key crates used:

| Crate              | Purpose                              |
| ------------------ | ------------------------------------ |
| `esp-hal`          | Hardware abstraction layer for ESP32 |
| `esp-radio`        | WiFi driver                          |
| `esp-rtos`         | Embassy integration for ESP32        |
| `embassy-net`      | Async TCP/IP networking              |
| `embassy-executor` | Async task executor                  |
| `embassy-sync`     | Async synchronization primitives     |
| `embassy-time`     | Async timers and delays              |
