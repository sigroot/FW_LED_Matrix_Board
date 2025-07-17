// Written by sigroot
//! sig_rp2040_board - A board to contain multiple applets
//!
//! Main library contains functions for interfacting with whole 9x34 matrix
//!
//! Main binary accepts TCP communication from other processes to display
//! one 9 pixel status bar (applet 0) and three 9x(10+1) pixel applets
//! (applets 1-3)
//!
//! Communication is in the following format:
//!
//! Communication is over TCP
//!
//! Commands are received with JSON encoded 'Command' strucutres in the format:
//! ```text
//! {
//!     "opcode": "<Command Name>",
//!     "app_num": <Applet Number (0-2)>,
//!     "parameters": [x<,y<,...z> (where each value is a u8)]
//! }
//! ```
//!
//! Commands:
//!
//! CreateApplet - Creates a new applet assigned to the requesting TCP stream
//!
//! Parameters: 1 u8 from 0-3
//! ```text
//!     0 - Applet separator is empty (all LED's off)
//!
//!     1 - Applet separator is solid (all LED's on)
//!
//!     2 - Applet separator is dotted (alternating LED's on & off)
//!
//!     3 - Applet seprator is variable (default off)
//! ```
//!
//! UpdateGrid - Rewrites the current 9x10 applet grid with new values
//!
//! Parameters: 
//! 
//!     90 u8 representing grid brightnesses - rows then columns (1st 10 is row1, 2nd 10 is row2, etc.)
//!
//! UpdateBar - Rewrites the current 9x1 applet separator
//!
//! Parameters: 
//!
//!     9 u8 representing separator brightnesses
//!
//!     Note: Error 32 returned if bar is not variable
//!
//!
//! sig_rp2040_board will respond with a single u8 error code (not JSON):
//!
//! 0:	    Command successfully processed
//!
//! 10:	    Failed to read data from stream
//!
//! 20:	    Failed to parse stream data as UTF-8
//!
//! 21:	    Failed to parse stream data as JSON
//!
//! 30:	    Command uses invalid applet number (greater than 2)
//!
//! 31:	    Command attempts to modify applet stream did not create
//!
//! 32:     Attempt to update applet 0 grid
//!
//! 33:	    Error in commanding applet
//!
//! 34:	    Attempt to create new applet when applet already exists
//!
//! 40:	    Invalid separator value when creating applet
//!
//! 255:	Unknown error

use std::env;

use std::time::Duration;

use sig_rp2040_board::*;

use std::net::SocketAddr;
use std::process::exit;
use std::sync::Arc;
use std::sync::Mutex;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut options: Options = Options { test: false }; // [frame test, port, allow runtime replacement]
    let mut port: u16 = 27072;
    let mut write_interval: Duration = Duration::from_millis(17);

    // Collect user parameters
    let mut current_parameter = 1;
    while current_parameter < args.len() {
        if args[current_parameter].starts_with("-") {
            for j in args[current_parameter][1..].chars() {
                match j {
                    't' => options.test = true,
                    'p' => {
                        if args[current_parameter].len() > 2 {
                            error_argument()
                        };
                        if args.len() < current_parameter + 2 {
                            error_argument()
                        };
                        port = args[current_parameter + 1]
                            .parse::<u16>()
                            .expect("Invalid port number");
                        current_parameter += 1;
                    }
                    'f' => {
                        if args[current_parameter].len() > 2 {
                            error_argument()
                        };
                        if args.len() < current_parameter + 2 {
                            error_argument()
                        };
                        write_interval = Duration::from_millis(
                            1000 / args[current_parameter + 1]
                                .parse::<u64>()
                                .expect("Invalid framerate"),
                        );
                        current_parameter += 1;
                    }
                    _ => {
                        println!("{HELP_PAGE}");
                        exit(0);
                    }
                };
            }
        } else {
            println!("{HELP_PAGE}");
            exit(0);
        }
        current_parameter += 1;
    }

    // Start connection to LED matrix
    let mut board = matrix::LedMatrixInterface::new(BAUDRATE, TIMEOUT);
    init(&mut board);

    // Run test of board if in that mode
    if options.test == true {
        frame_test(&mut board);
        exit(0);
    }

    // Wrap board array, applet array, and options in send safe mutexes
    let board_mutex = Arc::new(Mutex::new(board));
    let applets_mutex = Arc::new(Mutex::new([None, None, None, None]));
    let options = Arc::new(Mutex::new(options));

    // Define TCP server address
    let local_addr = SocketAddr::from(([127, 0, 0, 1], port));

    // Create list to store handles for async task
    let mut task_handles = Vec::with_capacity(2);

    // Create single threaded (congruent) runtime
    let rt = tokio::runtime::Builder::new_current_thread()
        .worker_threads(1)
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    // Run 'handle_streams' and 'write_board' as tasks
    rt.block_on(async {
        task_handles.push(tokio::spawn(handle_streams(
            local_addr,
            Arc::clone(&applets_mutex),
            Arc::clone(&options),
        )));
        task_handles.push(tokio::spawn(write_board(
            Arc::clone(&applets_mutex),
            Arc::clone(&board_mutex),
            write_interval,
        )));

        for task in task_handles {
            let task_id = task.id();
            task.await.unwrap();
            println!("Task failure: {task_id}");
            break;
        }
    });
}
