/*!
# uart8250

This crate provide a unit struct with many methods to operate uart 8250.

[REF: Serial Programming/8250 UART Programming](https://en.wikibooks.org/wiki/Serial_Programming/8250_UART_Programming#UART_Registers)
*/

#![no_std]

#[macro_use]
pub mod registers;
pub mod uart;

pub use uart::{ChipFifoInfo, InterruptType, MmioUart8250, Parity};
