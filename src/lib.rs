// Written by sigroot
//! sig_rp2040_board - library

use sig_rp2040_applet::{Applet, Command, Opcode, Separator};
pub use sig_rp2040_interface as matrix;

use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::process::exit;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::spawn;
use tokio::time::interval;

use tokio::io::AsyncWriteExt;

pub const ON: [[u8; 9]; 34] = [[255; 9]; 34];
pub const OFF: [[u8; 9]; 34] = [[0; 9]; 34];

pub const BAUDRATE: u32 = 1000000;
pub const TIMEOUT: u64 = 10000;
pub const BUFFER_SIZE: usize = 8192;
pub const HELP_PAGE: &str = "\
Framework LED matrix controller.

Acts as an interface between the Framework LED matrix and applet programs

    $sig_rp2040_board [-trh] [-p <port>] [-f <framerate>]

Flags:
    -t  Run a frame test
    -p  Set port (default 27072)
    -f  Set framerate (default 60)
    -r  Permit runtime applet replacement
    -h  Display this menu
";

pub struct Options {
    pub test: bool,
}

/// Runs a test of all pixels and varying brightnesses
pub fn frame_test(board: &mut matrix::LedMatrixInterface) {
    let mut pattern = [[0; 9]; 34];
    let start = SystemTime::now();
    for i in 0..34 {
        for j in 0..9 {
            pattern[i][j] = (i % 16) as u8 ^ j as u8;
            pwm(board, &pattern);
        }
    }
    let time = SystemTime::now().duration_since(start);
    println!(
        "Max Average FPS: {}",
        34.0 * 9.0 * (1000.0 / (time.unwrap().as_millis() as f64))
    );
    pwm(board, &OFF);
}

/// Sets every pixel's brightness to the values of an inputted matrix
pub fn pwm(board: &mut matrix::LedMatrixInterface, input_matrix: &[[u8; 9]; 34]) {
    board.set_pwm(input_matrix);
    board.write_pwm();
}

/// Sets every pixel's scale to the values of an inputted matrix
pub fn scale(board: &mut matrix::LedMatrixInterface, input_matrix: &[[u8; 9]; 34]) {
    board.set_scale(input_matrix);
    board.write_scale();
}

/// Initializes the matrix to full scale and off pwm
pub fn init(board: &mut matrix::LedMatrixInterface) {
    board.set_pwm(&OFF);
    board.set_scale(&ON);
    board.write();
}

/// Processes each stream (one stream per applet)
pub async fn handle_streams(
    local_addr: SocketAddr,
    applets_mutex: Arc<Mutex<[Option<Applet>; 4]>>,
    options: Arc<Mutex<Options>>,
) {
    // Create TCP listener from address
    let listener = TcpListener::bind(local_addr)
        .await
        .expect("Failed to bind port!");

    // Start server
    loop {
        // Handle applet connection
        match listener.accept().await {
            Ok((stream, client_addr)) => {
                println!("Connected to {client_addr}!");
                spawn(run_commands(
                    stream,
                    client_addr,
                    Arc::clone(&applets_mutex),
                    Arc::clone(&options),
                ));
            }
            Err(e) => {
                eprintln!("A connection error has occured: {e}");
            }
        }
    }
}

/// Converts stream input to applet display
async fn run_commands(
    mut stream: TcpStream,
    client_addr: SocketAddr,
    applets_mutex: Arc<Mutex<[Option<Applet>; 4]>>,
    options: Arc<Mutex<Options>>,
) {
    let mut buffer: [u8; BUFFER_SIZE];
    let mut read_data: VecDeque<char> = VecDeque::new();
    let mut app_num = None;

    // Run for each recieved packet
    loop {
        // Wait for data
        stream
            .readable()
            .await
            .expect("Error reading stream: {client_addr}");
        // Initialize buffer
        buffer = [0; BUFFER_SIZE];

        // Read stream data to buffer (may not be complete packet or may be multiple packets)
        match stream.try_read(&mut buffer) {
            // Stream cleanly ended, no longer connected
            Ok(0) => {
                stop_applet(applets_mutex, app_num);
                break;
            }
            // Stream not cleanly ended, no longer connected
            Err(ref e) if e.kind() == io::ErrorKind::ConnectionReset => {
                stop_applet(applets_mutex, app_num);
                break;
            }
            // Read x bytes
            Ok(_) => (),
            // Not ready to read
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            // Read has failed
            Err(e) => {
                eprintln!("Failed read: {client_addr} from {e}");
                stream.write_u8(10).await.unwrap();
                panic!()
            }
        };

        // Convert buffer to UTF-8 string
        let buffer_string = match std::str::from_utf8(&buffer) {
            Ok(x) => x.trim_matches('\0'),
            Err(_) => {
                eprintln!("Could not parse stream as utf8");
                stream.write_u8(20).await.unwrap();
                panic!()
            }
        };

        // Push buffer to read_data
        for c in buffer_string.chars() {
            match c {
                '\0' => break,
                _ => read_data.push_back(c),
            }
        }

        // Parse every command in read_data
        loop {
            // Check if there is another command in read_data
            if !read_data.contains(&'}') {
                break;
            };

            // Pop next command
            let mut command_string = String::with_capacity(256);
            loop {
                match read_data.pop_front() {
                    Some('}') => {
                        command_string.push('}');
                        break;
                    }
                    Some(x) => command_string.push(x),
                    None => {
                        eprintln!(
                            "Could not parse command:\n{:?}\nError: No '{}' in JSON",
                            command_string.as_str().as_bytes(),
                            '}'
                        );
                        stream.write_u8(21).await.unwrap();
                        continue;
                    }
                }
            }

            // Parse command from string to object
            let command: Command = match serde_json::de::from_str(command_string.as_str()) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!(
                        "Could not parse command:\n{:?}\nError: {e}",
                        command_string.as_str().as_bytes()
                    );
                    stream.write_u8(21).await.unwrap();
                    continue;
                }
            };

            // Test for invalid command
            if command.app_num > 3 {
                eprintln!("Invalid applet number: {client_addr}");
                stream.write_u8(30).await.unwrap();
                panic!();
            }

            // Test if command applet number matches applet created by stream
            match app_num {
                Some(x) => {
                    if x != command.app_num {
                        eprintln!("{client_addr} attempted to modify wrong applet: applet {x}");
                        stream.write_u8(31).await.unwrap();
                        panic!();
                    }
                }
                None => (),
            }

            // Response variable to avoid async with applet
            let mut response = 0;

            // Contain use of mutex lock (match requires reference during 'None')
            {
                let applet = &mut applets_mutex.try_lock().unwrap()[command.app_num as usize];
                match applet {
                    // Write command
                    Some(x) => {
                        if app_num == Some(0) {
                            match command.opcode {
                                Opcode::UpdateBar => match x.command_applet(&command) {
                                    Ok(x) => x,
                                    Err(_) => response = 33,
                                },
                                Opcode::UpdateGrid => response = 32,
                                Opcode::CreateApplet => response = 34,
                            }
                        } else {
                            if command.opcode != Opcode::CreateApplet {
                                match x.command_applet(&command) {
                                    Ok(x) => x,
                                    Err(_) => response = 33,
                                }
                            } else {
                                response = 34;
                            }
                        }
                    }
                    // Create applet
                    None => {
                        if command.opcode == Opcode::CreateApplet && command.parameters.len() == 1 {
                            // Only allow stream to modify its own applet
                            app_num = Some(command.app_num);
                            match command.parameters[0] {
                                0 => *applet = Some(Applet::new(Separator::Empty)),
                                1 => *applet = Some(Applet::new(Separator::Solid)),
                                2 => *applet = Some(Applet::new(Separator::Dotted)),
                                3 => *applet = Some(Applet::new(Separator::Variable)),
                                _ => response = 40,
                            };
                        }
                    }
                };
            }

            // Send response from previous match
            match response {
                // Finish command successfully
                0 => {
                    stream.write_u8(0).await.unwrap();
                }
                // Attempt to update applet 0 grid
                32 => {
                    eprintln!("Attempted to update applet 0 grid");
                    stream.write_u8(32).await.unwrap();
                    stop_applet(applets_mutex, app_num);
                    panic!();
                }
                // Applet command error
                33 => {
                    eprintln!("Command failed");
                    stream.write_u8(33).await.unwrap();
                    continue;
                }
                // Attempt to create applet on top of another
                34 => {
                    eprintln!(
                        "Attempted to generate new applet on existing applet {}",
                        command.app_num
                    );
                    stream.write_u8(34).await.unwrap();
                    stop_applet(applets_mutex, app_num);
                    panic!();
                }
                // Invalid separator parameter
                40 => {
                    eprintln!("Invalid separator value: {client_addr}");
                    stream.write_u8(40).await.unwrap();
                    stop_applet(applets_mutex, app_num);
                    panic!();
                }
                // Unknown error (should never be reached)
                _ => {
                    eprintln!("Unkown Error!");
                    stream.write_u8(255).await.unwrap();
                    exit(1);
                }
            };
        }
    }
}

/// Resets applet
fn stop_applet(applets_mutex: Arc<Mutex<[Option<Applet>; 4]>>, app_num: Option<u8>) {
    match app_num {
        Some(x) => {
            if x > 3 {
                eprintln!("stop_applet recieved invalid app_num: {x}")
            };
            applets_mutex.try_lock().unwrap()[x as usize] = None
        }
        None => (),
    }
}

/// Periodically writes entire LED matrix
pub async fn write_board(
    applets_mutex: Arc<Mutex<[Option<Applet>; 4]>>,
    board: Arc<Mutex<matrix::LedMatrixInterface>>,
    write_interval: Duration,
) {
    let mut board_input = [[0; 9]; 34];

    // Only attempt pause if write_interval > 0
    if write_interval >= Duration::from_nanos(1) {
        // Define interval length to repeatedly wait
        let mut clock = interval(write_interval);

        // Refesh board repeatedly
        loop {
            // Wait for interval (regardless of time spent refreshing or handling requests)
            clock.tick().await;
            // Copy stored data to board_input
            let status_bar = match &applets_mutex.try_lock().unwrap()[0] {
                Some(x) => x.get_board()[0],
                None => [0; 9],
            };
            for i in 0..9 {
                board_input[0][i] = status_bar[i];
            }
            for i in 1..4 {
                let applet = match &applets_mutex.try_lock().unwrap()[i] {
                    Some(x) => x.get_board(),
                    None => [[0; 9]; 11],
                };
                {
                    for j in 0..11 {
                        for k in 0..9 {
                            board_input[11 * (i - 1) + j + 1][k] = applet[j][k];
                        }
                    }
                }
            }
            // Write board input to Framework LED matrix
            pwm(&mut board.try_lock().unwrap(), &board_input);
        }
    } else {
        loop {
            // Copy stored data to board_input
            let status_bar = match &applets_mutex.try_lock().unwrap()[0] {
                Some(x) => x.get_board()[0],
                None => [0; 9],
            };
            for i in 0..9 {
                board_input[0][i] = status_bar[i];
            }
            for i in 1..4 {
                let applet = match &applets_mutex.try_lock().unwrap()[i] {
                    Some(x) => x.get_board(),
                    None => [[0; 9]; 11],
                };
                {
                    for j in 0..11 {
                        for k in 0..9 {
                            board_input[11 * (i - 1) + j + 1][k] = applet[j][k];
                        }
                    }
                }
            }
            // Write board input to Framework LED matrix
            pwm(&mut board.try_lock().unwrap(), &board_input);
        }
    }
}

/// Give error message and exit
pub fn error_argument() {
    eprintln!("{HELP_PAGE}\nCan not use combined flag with flag that requires arguments");
    exit(0);
}
