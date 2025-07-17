// Written by sigroot
//! sig_rp2040_applet - Smaller applet squares for the sig_rp2040_board main
//! binary
//!
//! Commands:
//!
//! CreateApplet - Creates a new applet assigned to the requesting TCP stream
//!     Parameters: 1 u8 from 0-3
//!         0 - Applet separator is empty (all LED's off)
//!         1 - Applet separator is solid (all LED's on)
//!         2 - Applet separator is dotted (alternating LED's on & off)
//!         3 - Applet seprator is variable (default off)
//!
//! UpdateGrid - Rewrites the current 9x10 applet grid with new values
//!     Parameters: 90 u8 representing grid brightnesses - rows then columns
//!                 (1st 10 is row1, 2nd 10 is row2, etc.)
//!
//! UpdateBar - Rewrites the current 9x1 applet separator
//!     Parameters: 9 u8 representing separator brightnesses
//!     Note: Returns error if separator is not variable
//!

use serde::Deserialize;

pub struct Applet {
    grid: [[u8; 9]; 10],
    separator_type: Separator,
    separator: [u8; 9],
}

impl Applet {
    pub fn new(separator_type: Separator) -> Self {
        Applet {
            separator: match separator_type {
                Separator::Empty => [0; 9],
                Separator::Solid => [255; 9],
                Separator::Dotted => [255, 0, 255, 0, 255, 0, 255, 0, 255],
                Separator::Variable => [0; 9],
            },
            separator_type: separator_type,
            grid: [[0; 9]; 10],
        }
    }

    pub fn command_applet(&mut self, command: &Command) -> Result<(), &'static str> {
        match command.opcode {
            Opcode::UpdateGrid => {
                // UpdateGrid command is 90 characters long
                match command.parameters.len() {
                    90 => {
                        for i in 0..10 {
                            for j in 0..9 {
                                self.grid[i][j] = command.parameters[i * 9 + j];
                            }
                        }
                    }
                    _ => return Err("Invalid parameter length"),
                }
                Ok(())
            }
            Opcode::UpdateBar => {
                // UpdateBar command requires variable separator
                match self.separator_type {
                    Separator::Variable => match command.parameters.len() {
                        9 => {
                            for i in 0..9 {
                                self.separator[i] = command.parameters[i];
                            }
                        }
                        _ => return Err("Invalid parameter length"),
                    },
                    _ => return Err("Bar not variable"),
                }
                Ok(())
            }
            Opcode::CreateApplet => return Err("Applet cannot sign new applet"),
        }
    }

    pub fn get_board(&self) -> [[u8; 9]; 11] {
        let mut output: [[u8; 9]; 11] = [[0; 9]; 11];
        output[0] = self.separator.clone();
        for i in 1..11 {
            output[i] = self.grid[i - 1].clone();
        }
        output
    }
}

pub enum Separator {
    Empty,
    Solid,
    Dotted,
    Variable,
}

#[derive(Deserialize)]
pub struct Command {
    pub opcode: Opcode,
    pub app_num: u8,
    pub parameters: Vec<u8>,
}

#[derive(Deserialize, PartialEq, Eq)]
pub enum Opcode {
    CreateApplet,
    UpdateGrid,
    UpdateBar,
}
