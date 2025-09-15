# Framework LED Matrix Board
## Preamble
**THIS IS UNOFFICIAL SOFTWARE. I AM NOT AFFILIATED WITH FRAMEWORK**

This repository contains a language agnostic server for the [Framework LED Matrix](https://frame.work/products/16-led-matrix) which can display and modify one 9x1 'status bar' and three 9x11 'applets'. 

This repository requires the [FW_LED_Matrix_Firmware](https://github.com/sigroot/FW_LED_Matrix_Firmware) and [FW_LED_Matrix_Interface](https://github.com/sigroot/FW_LED_Matrix_Interface) repositories.

This repository is designed to be run initially and then communicated with through the [FW_LED_Matrix_Applet](https://github.com/sigroot/FW_LED_Matrix_Applet) library.

Thanks to [Ecca](https://community.frame.work/t/use-cases-for-the-led-matrix-module/39171/75) on the Framework Forums for the idea of separating the LED matrix into separate modules.
## Capabilities
This repository contains a server that holds four total applets. Applet 0 is located across the top of the LED matrix and only accepts modifications to its separator bar. Applets 1-3 are located in order from top to bottom of the LED matrix. Applets 1-3 each have an optionally variable separator bar at its top and a modifiable grid at its bottom. Each separator bar is 9 LEDs wide and 1 LED high. Each grid is 9 LEDs wide and 10 LEDs high.

The server can update each applet at roughly 80 frames per second.
### Associated Software
[FW_LED_Matrix_Firmware](https://github.com/sigroot/FW_LED_Matrix_Firmware) is Arduino-based firmware and is a prerequisite installation for this library.

[FW_LED_Matrix_Interface](https://github.com/sigroot/FW_LED_Matrix_Interface) is a Rust library for interfacing between this firmware and other Rust programs.

[FW_LED_Applet_Interface](https://github.com/sigroot/FW_LED_Applet_Interface) is a Rust Library for interfacing between Rust programs and [FW_LED_Matrix_Board](https://github.com/sigroot/FW_LED_Matrix_Board).
### Communication
Communication is over TCP

Commands are received with JSON encoded 'Command' structres in the format:
{
    "opcode": "<Command Name>",
    "app_num": <Applet Number (0-2)>,
    "parameters": [x<,y<,...z> (where each value is a u8)]
}

**Commands**:

CreateApplet - Creates a new applet assigned to the requesting TCP stream

Parameters: 1 u8 from 0-3
    0 - Applet separator is empty (all LED's off)

    1 - Applet separator is solid (all LED's on)

    2 - Applet separator is dotted (alternating LED's on & off)

    3 - Applet seprator is variable (default off)

UpdateGrid - Rewrites the current 9x10 applet grid with new values

Parameters: 
    90 u8 representing grid brightnesses - rows then columns (1st 10 is row1, 2nd 10 is row2, etc.)

UpdateBar - Rewrites the current 9x1 applet separator

Parameters:
    9 u8 representing separator brightnesses

    Note: Error 32 returned if bar is not variable

sig_rp2040_board will respond with a single u8 error code (not JSON):

0:	    Command successfully processed

10:	    Failed to read data from stream

20:	    Failed to parse stream data as UTF-8

21:	    Failed to parse stream data as JSON

30:	    Command uses invalid applet number (greater than 2)

31:	    Command attempts to modify applet stream did not create

32:     Attempt to update applet 0 grid

33:	    Error in commanding applet

34:	    Attempt to create new applet when applet already exists

40:	    Invalid separator value when creating applet

255:	Unknown error
