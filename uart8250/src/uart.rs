use bitflags::bitflags;
#[cfg(feature = "fmt")]
use core::fmt;

use crate::registers::Registers;

bitflags! {
    /// Interrupt Enable Register (bitflags)
    pub struct IER: u8 {
        /// Enable Received Data Available Interrupt
        const RDAI  = 0b0000_0001;
        /// Enable Transmitter Holding Register Empty Interrupt
        const THREI = 0b0000_0010;
        /// Enable Receiver Line Status Interrupt
        const RLSI  = 0b0000_0100;
        /// Enable Modem Status Interrupt
        const MSI   = 0b0000_1000;
        /// Enable Sleep Mode (16750)
        const SM    = 0b0001_0000;
        /// Enable Low Power Mode (16750)
        const LPM   = 0b0010_0000;
    }
}

bitflags! {
    /// Line Status Register (bitflags)
    pub struct LSR: u8 {
        /// Data Ready
        const DR = 0b0000_0001;
        /// Overrun Error
        const OE = 0b0000_0010;
        /// Parity Error
        const PE = 0b0000_0100;
        /// Framing Error
        const FE = 0b0000_1000;
        /// Break Interrupt
        const BI = 0b0001_0000;
        /// Transmitter Holding Register Empty
        const THRE = 0b0010_0000;
        /// Data Holding Regiters Empty
        const DHRE = 0b0100_0000;
        /// Error in Received FIFO
        const RFE = 0b1000_0000;
    }
}

bitflags! {
    /// Modem Status Register (bitflags)
    pub struct MSR: u8 {
        /// Delta Clear To Send
        const DCTS = 0b0000_0001;
        ///Delta Data Set Ready
        const DDSR = 0b0000_0010;
        ///Trailing Edge Ring Indicator
        const TERI = 0b0000_0100;
        ///Delta Data Carrier Detect
        const DDCD = 0b0000_1000;
        ///Clear To Send
        const CTS = 0b0001_0000;
        ///Data Set Ready
        const DSR = 0b0010_0000;
        ///Ring Indicator
        const RI = 0b0100_0000;
        ///Carrier Detect
        const CD = 0b1000_0000;
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ChipFifoInfo {
    NoFifo,
    Reserved,
    EnabledNoFunction,
    Enabled,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InterruptType {
    ModemStatus,
    TransmitterHoldingRegisterEmpty,
    ReceivedDataAvailable,
    ReceiverLineStatus,
    Timeout,
    Reserved,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Parity {
    No,
    Odd,
    Even,
    Mark,
    Space,
}

/// # MMIO version of an 8250 UART.
///
/// **Note** This is only tested on the NS16550 compatible UART used in QEMU 5.0 virt machine of RISC-V.
pub struct MmioUart8250<'a> {
    reg: &'a mut Registers,
}

impl<'a> MmioUart8250<'a> {
    /// Creates a new UART.
    pub fn new(base_address: usize) -> Self {
        Self {
            reg: Registers::from_base_address(base_address),
        }
    }

    /// Initialises the UART with common settings and interrupts enabled.
    ///
    /// More customised initialisation can be done using other methods below.
    pub fn init(&self, clock: usize, baud_rate: usize) {
        // Enable DLAB and Set divisor
        self.set_divisor(clock, baud_rate);

        // Disable DLAB and set word length 8 bits, no parity, 1 stop bit
        self.write_lcr(3);
        // Enable FIFO
        self.write_fcr(1);
        // No modem control
        self.write_mcr(0);
        // Enable received_data_available_interrupt
        self.enable_received_data_available_interrupt();
        // Enable transmitter_holding_register_empty_interrupt
        // self.enable_transmitter_holding_register_empty_interrupt();
    }

    /// Sets a new base address for the UART.
    pub fn set_base_address(&mut self, base_address: usize) {
        self.reg = Registers::from_base_address(base_address);
    }

    /// Reads a byte from the UART.
    ///
    /// Returns `None` when data is not ready (RBR\[0\] != 1)
    pub fn read_byte(&self) -> Option<u8> {
        if self.is_data_ready() {
            Some(self.read_rbr())
        } else {
            None
        }
    }

    /// Writes a byte to the UART.
    ///
    /// TODO: This currently ignores errors.
    pub fn write_byte(&self, byte: u8) {
        self.write_thr(byte);
    }

    /// write THR (offset + 0)
    ///
    /// Write Transmitter Holding Buffer to send data
    ///
    /// > ## Transmitter Holding Buffer/Receiver Buffer
    /// >
    /// > Offset: +0 . The Transmit and Receive buffers are related, and often even use the very same memory. This is also one of the areas where later versions of the 8250 chip have a significant impact, as the later models incorporate some internal buffering of the data within the chip before it gets transmitted as serial data. The base 8250 chip can only receive one byte at a time, while later chips like the 16550 chip will hold up to 16 bytes either to transmit or to receive (sometimes both... depending on the manufacturer) before you have to wait for the character to be sent. This can be useful in multi-tasking environments where you have a computer doing many things, and it may be a couple of milliseconds before you get back to dealing with serial data flow.
    /// >
    /// > These registers really are the "heart" of serial data communication, and how data is transferred from your software to another computer and how it gets data from other devices. Reading and Writing to these registers is simply a matter of accessing the Port I/O address for the respective UART.
    /// >
    /// > If the receive buffer is occupied or the FIFO is full, the incoming data is discarded and the Receiver Line Status interrupt is written to the IIR register. The Overrun Error bit is also set in the Line Status Register.
    #[inline]
    pub fn write_thr(&self, value: u8) {
        unsafe { self.reg.thr_rbr_dll.write(value) }
    }

    /// read RBR (offset + 0)
    ///
    /// Read Receiver Buffer to get data
    #[inline]
    pub fn read_rbr(&self) -> u8 {
        self.reg.thr_rbr_dll.read()
    }

    /// read DLL (offset + 0)
    ///
    /// get divisor latch low byte in the register
    ///
    /// > ## Divisor Latch Bytes
    /// >
    /// > Offset: +0 and +1 . The Divisor Latch Bytes are what control the baud rate of the modem. As you might guess from the name of this register, it is used as a divisor to determine what baud rate that the chip is going to be transmitting at.
    ///
    /// Used clock 1.8432 MHz as example, first divide 16 and get 115200. Then use the formula to get divisor latch value:
    ///
    /// *DivisorLatchValue = 115200 / BaudRate*
    ///
    /// This gives the following table:
    ///
    /// | Baud Rate | Divisor (in decimal) | Divisor Latch High Byte | Divisor Latch Low Byte |
    /// | --------- | -------------------- | ----------------------- | ---------------------- |
    /// | 50        | 2304                 | $09                     | $00                    |
    /// | 110       | 1047                 | $04                     | $17                    |
    /// | 220       | 524                  | $02                     | $0C                    |
    /// | 300       | 384                  | $01                     | $80                    |
    /// | 600       | 192                  | $00                     | $C0                    |
    /// | 1200      | 96                   | $00                     | $60                    |
    /// | 2400      | 48                   | $00                     | $30                    |
    /// | 4800      | 24                   | $00                     | $18                    |
    /// | 9600      | 12                   | $00                     | $0C                    |
    /// | 19200     | 6                    | $00                     | $06                    |
    /// | 38400     | 3                    | $00                     | $03                    |
    /// | 57600     | 2                    | $00                     | $02                    |
    /// | 115200    | 1                    | $00                     | $01                    |
    #[inline]
    pub fn read_dll(&self) -> u8 {
        self.reg.thr_rbr_dll.read()
    }

    /// write DLL (offset + 0)
    ///
    /// set divisor latch low byte in the register
    #[inline]
    pub fn write_dll(&self, value: u8) {
        unsafe { self.reg.thr_rbr_dll.write(value) }
    }

    /// read DLH (offset + 1)
    ///
    /// get divisor latch high byte in the register
    #[inline]
    pub fn read_dlh(&self) -> u8 {
        self.reg.ier_dlh.read()
    }

    /// write DLH (offset + 1)
    ///
    /// set divisor latch high byte in the register
    #[inline]
    pub fn write_dlh(&self, value: u8) {
        unsafe { self.reg.ier_dlh.write(value) }
    }

    /// Set divisor latch according to clock and baud_rate, then set DLAB to false
    #[inline]
    pub fn set_divisor(&self, clock: usize, baud_rate: usize) {
        self.enable_divisor_latch_accessible();
        let divisor = clock / (16 * baud_rate);
        self.write_dll(divisor as u8);
        self.write_dlh((divisor >> 8) as u8);
        self.disable_divisor_latch_accessible();
    }

    /// Read IER (offset + 1)
    ///
    /// Read IER to get what interrupts are enabled
    ///
    /// > ## Interrupt Enable Register
    /// >
    /// > Offset: +1 . This register allows you to control when and how the UART is going to trigger an interrupt event with the hardware interrupt associated with the serial COM port. If used properly, this can enable an efficient use of system resources and allow you to react to information being sent across a serial data line in essentially real-time conditions. Some more on that will be covered later, but the point here is that you can use the UART to let you know exactly when you need to extract some data. This register has both read- and write-access.
    /// >
    /// > The following is a table showing each bit in this register and what events that it will enable to allow you check on the status of this chip:
    /// >
    /// > | Bit | Notes                                               |
    /// > | --- | --------------------------------------------------- |
    /// > | 7   | Reserved                                            |
    /// > | 6   | Reserved                                            |
    /// > | 5   | Enables Low Power Mode (16750)                      |
    /// > | 4   | Enables Sleep Mode (16750)                          |
    /// > | 3   | Enable Modem Status Interrupt                       |
    /// > | 2   | Enable Receiver Line Status Interrupt               |
    /// > | 1   | Enable Transmitter Holding Register Empty Interrupt |
    /// > | 0   | Enable Received Data Available Interrupt            |
    #[inline]
    pub fn read_ier(&self) -> u8 {
        self.reg.ier_dlh.read()
    }

    /// Write IER (offset + 1)
    ///
    /// Write Interrupt Enable Register to turn on/off interrupts
    #[inline]
    pub fn write_ier(&self, value: u8) {
        unsafe { self.reg.ier_dlh.write(value) }
    }

    /// Get IER bitflags
    #[inline]
    pub fn ier(&self) -> IER {
        IER::from_bits_truncate(self.read_ier())
    }

    /// Set IER via bitflags
    #[inline]
    pub fn set_ier(&self, flag: IER) {
        self.write_ier(flag.bits())
    }

    /// get whether low power mode (16750) is enabled (IER\[5\])
    pub fn is_low_power_mode_enabled(&self) -> bool {
        self.ier().contains(IER::LPM)
    }

    /// toggle low power mode (16750) (IER\[5\])
    pub fn toggle_low_power_mode(&self) {
        self.set_ier(self.ier() ^ IER::LPM)
    }

    /// enable low power mode (16750) (IER\[5\])
    pub fn enable_low_power_mode(&self) {
        self.set_ier(self.ier() | IER::LPM)
    }

    /// disable low power mode (16750) (IER\[5\])
    pub fn disable_low_power_mode(&self) {
        self.set_ier(self.ier() & !IER::LPM)
    }

    /// get whether sleep mode (16750) is enabled (IER\[4\])
    pub fn is_sleep_mode_enabled(&self) -> bool {
        self.ier().contains(IER::SM)
    }

    /// toggle sleep mode (16750) (IER\[4\])
    pub fn toggle_sleep_mode(&self) {
        self.set_ier(self.ier() ^ IER::SM)
    }

    /// enable sleep mode (16750) (IER\[4\])
    pub fn enable_sleep_mode(&self) {
        self.set_ier(self.ier() | IER::SM)
    }

    /// disable sleep mode (16750) (IER\[4\])
    pub fn disable_sleep_mode(&self) {
        self.set_ier(self.ier() & !IER::SM)
    }

    /// get whether modem status interrupt is enabled (IER\[3\])
    pub fn is_modem_status_interrupt_enabled(&self) -> bool {
        self.ier().contains(IER::MSI)
    }

    /// toggle modem status interrupt (IER\[3\])
    pub fn toggle_modem_status_interrupt(&self) {
        self.set_ier(self.ier() ^ IER::MSI)
    }

    /// enable modem status interrupt (IER\[3\])
    pub fn enable_modem_status_interrupt(&self) {
        self.set_ier(self.ier() | IER::MSI)
    }

    /// disable modem status interrupt (IER\[3\])
    pub fn disable_modem_status_interrupt(&self) {
        self.set_ier(self.ier() & !IER::MSI)
    }

    /// get whether receiver line status interrupt is enabled (IER\[2\])
    pub fn is_receiver_line_status_interrupt_enabled(&self) -> bool {
        self.ier().contains(IER::RLSI)
    }

    /// toggle receiver line status interrupt (IER\[2\])
    pub fn toggle_receiver_line_status_interrupt(&self) {
        self.set_ier(self.ier() ^ IER::RLSI)
    }

    /// enable receiver line status interrupt (IER\[2\])
    pub fn enable_receiver_line_status_interrupt(&self) {
        self.set_ier(self.ier() | IER::RLSI)
    }

    /// disable receiver line status interrupt (IER\[2\])
    pub fn disable_receiver_line_status_interrupt(&self) {
        self.set_ier(self.ier() & !IER::RLSI)
    }

    /// get whether transmitter holding register empty interrupt is enabled (IER\[1\])
    pub fn is_transmitter_holding_register_empty_interrupt_enabled(&self) -> bool {
        self.ier().contains(IER::THREI)
    }

    /// toggle transmitter holding register empty interrupt (IER\[1\])
    pub fn toggle_transmitter_holding_register_empty_interrupt(&self) {
        self.set_ier(self.ier() ^ IER::THREI)
    }

    /// enable transmitter holding register empty interrupt (IER\[1\])
    pub fn enable_transmitter_holding_register_empty_interrupt(&self) {
        self.set_ier(self.ier() | IER::THREI)
    }

    /// disable transmitter holding register empty interrupt (IER\[1\])
    pub fn disable_transmitter_holding_register_empty_interrupt(&self) {
        self.set_ier(self.ier() & !IER::THREI)
    }

    /// get whether received data available is enabled (IER\[0\])
    pub fn is_received_data_available_interrupt_enabled(&self) -> bool {
        self.ier().contains(IER::RDAI)
    }

    /// toggle received data available (IER\[0\])
    pub fn toggle_received_data_available_interrupt(&self) {
        self.set_ier(self.ier() ^ IER::RDAI)
    }

    /// enable received data available (IER\[0\])
    pub fn enable_received_data_available_interrupt(&self) {
        self.set_ier(self.ier() | IER::RDAI)
    }

    /// disable received data available (IER\[0\])
    pub fn disable_received_data_available_interrupt(&self) {
        self.set_ier(self.ier() & !IER::RDAI)
    }

    /// Read IIR (offset + 2)
    ///
    /// > ## Interrupt Identification Register
    /// >
    /// > Offset: +2 . This register is to be used to help identify what the unique characteristics of the UART chip that you are using has. This chip has two uses:
    /// >
    /// > - Identification of why the UART triggered an interrupt.
    /// > - Identification of the UART chip itself.
    /// >
    /// > Of these, identification of why the interrupt service routine has been invoked is perhaps the most important.
    /// >
    /// > The following table explains some of the details of this register, and what each bit on it represents:
    /// >
    /// > | Bit        | Notes                             |       |                                   |                                              |                                                                                           |
    /// > | ---------- | --------------------------------- | ----- | --------------------------------- | -------------------------------------------- | ----------------------------------------------------------------------------------------- |
    /// > | 7 and 6    | Bit 7                             | Bit 6 |                                   |                                              |                                                                                           |
    /// > |            | 0                                 | 0     | No FIFO on chip                   |                                              |                                                                                           |
    /// > |            | 0                                 | 1     | Reserved condition                |                                              |                                                                                           |
    /// > |            | 1                                 | 0     | FIFO enabled, but not functioning |                                              |                                                                                           |
    /// > |            | 1                                 | 1     | FIFO enabled                      |                                              |                                                                                           |
    /// > | 5          | 64 Byte FIFO Enabled (16750 only) |       |                                   |                                              |                                                                                           |
    /// > | 4          | Reserved                          |       |                                   |                                              |                                                                                           |
    /// > | 3, 2 and 1 | Bit 3                             | Bit 2 | Bit 1                             |                                              | Reset Method                                                                              |
    /// > |            | 0                                 | 0     | 0                                 | Modem Status Interrupt                       | Reading Modem Status Register(MSR)                                                        |
    /// > |            | 0                                 | 0     | 1                                 | Transmitter Holding Register Empty Interrupt | Reading Interrupt Identification Register(IIR) or Writing to Transmit Holding Buffer(THR) |
    /// > |            | 0                                 | 1     | 0                                 | Received Data Available Interrupt            | Reading Receive Buffer Register(RBR)                                                      |
    /// > |            | 0                                 | 1     | 1                                 | Receiver Line Status Interrupt               | Reading Line Status Register(LSR)                                                         |
    /// > |            | 1                                 | 0     | 0                                 | Reserved                                     | N/A                                                                                       |
    /// > |            | 1                                 | 0     | 1                                 | Reserved                                     | N/A                                                                                       |
    /// > |            | 1                                 | 1     | 0                                 | Time-out Interrupt Pending (16550 & later)   | Reading Receive Buffer Register(RBR)                                                      |
    /// > |            | 1                                 | 1     | 1                                 | Reserved                                     | N/A                                                                                       |
    /// > | 0          | Interrupt Pending Flag            |       |                                   |                                              |                                                                                           |
    #[inline]
    pub fn read_iir(&self) -> u8 {
        self.reg.iir_fcr.read()
    }

    /// Read IIR\[7:6\] to get FIFO status
    pub fn read_fifo_status(&self) -> ChipFifoInfo {
        match self.reg.iir_fcr.read() & 0b1100_0000 {
            0 => ChipFifoInfo::NoFifo,
            0b0100_0000 => ChipFifoInfo::Reserved,
            0b1000_0000 => ChipFifoInfo::EnabledNoFunction,
            0b1100_0000 => ChipFifoInfo::Enabled,
            _ => panic!("Can't reached"),
        }
    }

    /// get whether 64 Byte fifo (16750 only) is enabled (IIR\[5\])
    pub fn is_64byte_fifo_enabled(&self) -> bool {
        self.reg.iir_fcr.read() & 0b0010_0000 != 0
    }

    /// Read IIR\[3:1\] to get interrupt type
    pub fn read_interrupt_type(&self) -> Option<InterruptType> {
        let irq = self.reg.iir_fcr.read() & 0b0000_1111;
        if irq & 1 != 0 {
            None
        } else {
            match irq {
                0b0000 => Some(InterruptType::ModemStatus),
                0b0010 => Some(InterruptType::TransmitterHoldingRegisterEmpty),
                0b0100 => Some(InterruptType::ReceivedDataAvailable),
                0b0110 => Some(InterruptType::ReceiverLineStatus),
                0b1100 => Some(InterruptType::Timeout),
                0b1000 | 0b1010 | 0b1110 => Some(InterruptType::Reserved),
                _ => panic!("Can't reached"),
            }
        }
    }

    /// get whether interrupt is pending (IIR\[0\])
    ///
    /// # Safety
    ///
    /// read iir will reset THREI, so use read_interrupt_type may be better
    pub unsafe fn is_interrupt_pending(&self) -> bool {
        self.reg.iir_fcr.read() & 1 == 0
    }

    /// Write FCR (offset + 2) to control FIFO buffers
    ///
    /// > ## FIFO Control Register
    /// >
    /// > Offset: +2 . This is a relatively "new" register that was not a part of the original 8250 UART implementation. The purpose of this register is to control how the First In/First Out (FIFO) buffers will behave on the chip and to help you fine-tune their performance in your application. This even gives you the ability to "turn on" or "turn off" the FIFO.
    /// >
    /// > Keep in mind that this is a "write only" register. Attempting to read in the contents will only give you the Interrupt Identification Register (IIR), which has a totally different context.
    /// >
    /// > | Bit   | Notes                       |       |                                   |                         |
    /// > | ----- | --------------------------- | ----- | --------------------------------- | ----------------------- |
    /// > | 7 & 6 | Bit 7                       | Bit 6 | Interrupt Trigger Level (16 byte) | Trigger Level (64 byte) |
    /// > |       | 0                           | 0     | 1 Byte                            | 1 Byte                  |
    /// > |       | 0                           | 1     | 4 Bytes                           | 16 Bytes                |
    /// > |       | 1                           | 0     | 8 Bytes                           | 32 Bytes                |
    /// > |       | 1                           | 1     | 14 Bytes                          | 56 Bytes                |
    /// > | 5     | Enable 64 Byte FIFO (16750) |       |                                   |                         |
    /// > | 4     | Reserved                    |       |                                   |                         |
    /// > | 3     | DMA Mode Select             |       |                                   |                         |
    /// > | 2     | Clear Transmit FIFO         |       |                                   |                         |
    /// > | 1     | Clear Receive FIFO          |       |                                   |                         |
    /// > | 0     | Enable FIFOs                |       |                                   |                         |
    #[inline]
    pub fn write_fcr(&self, value: u8) {
        unsafe { self.reg.iir_fcr.write(value) }
    }

    /// Read LCR (offset + 3)
    ///
    /// Read Line Control Register to get the data protocol and DLAB
    ///
    /// > ## Line Control Register
    /// >
    /// > Offset: +3 . This register has two major purposes:
    /// >
    /// > - Setting the Divisor Latch Access Bit (DLAB), allowing you to set the values of the Divisor Latch Bytes.
    /// > - Setting the bit patterns that will be used for both receiving and transmitting the serial data. In other words, the serial data protocol you will be using (8-1-None, 5-2-Even, etc.).
    /// >
    /// > | Bit      | Notes                    |                              |             |               |
    /// > | -------- | ------------------------ | ---------------------------- | ----------- | ------------- |
    /// > | 7        | Divisor Latch Access Bit |                              |             |               |
    /// > | 6        | Set Break Enable         |                              |             |               |
    /// > | 3, 4 & 5 | Bit 5                    | Bit 4                        | Bit 3       | Parity Select |
    /// > |          | 0                        | 0                            | 0           | No Parity     |
    /// > |          | 0                        | 0                            | 1           | Odd Parity    |
    /// > |          | 0                        | 1                            | 1           | Even Parity   |
    /// > |          | 1                        | 0                            | 1           | Mark          |
    /// > |          | 1                        | 1                            | 1           | Space         |
    /// > | 2        | 0                        | One Stop Bit                 |             |               |
    /// > |          | 1                        | 1.5 Stop Bits or 2 Stop Bits |             |               |
    /// > | 0 & 1    | Bit 1                    | Bit 0                        | Word Length |               |
    /// > |          | 0                        | 0                            | 5 Bits      |               |
    /// > |          | 0                        | 1                            | 6 Bits      |               |
    /// > |          | 1                        | 0                            | 7 Bits      |               |
    /// > |          | 1                        | 1                            | 8 Bits      |               |
    #[inline]
    pub fn read_lcr(&self) -> u8 {
        self.reg.lcr.read()
    }

    /// Write LCR (offset + 3)
    ///
    /// Write Line Control Register to set DLAB and the serial data protocol
    #[inline]
    pub fn write_lcr(&self, value: u8) {
        unsafe { self.reg.lcr.write(value) }
    }

    /// get whether DLAB is enabled
    pub fn is_divisor_latch_accessible(&self) -> bool {
        self.reg.lcr.read() & 0b1000_0000 != 0
    }

    /// toggle DLAB
    pub fn toggle_divisor_latch_accessible(&self) {
        unsafe { self.reg.lcr.modify(|v| v ^ 0b1000_0000) }
    }

    /// enable DLAB
    pub fn enable_divisor_latch_accessible(&self) {
        unsafe { self.reg.lcr.modify(|v| v | 0b1000_0000) }
    }

    /// disable DLAB
    pub fn disable_divisor_latch_accessible(&self) {
        unsafe { self.reg.lcr.modify(|v| v & !0b1000_0000) }
    }

    /// get parity of used data protocol
    pub fn get_parity(&self) -> Parity {
        match self.reg.lcr.read() & 0b0011_1000 {
            0b0000_0000 => Parity::No,
            0b0000_1000 => Parity::Odd,
            0b0001_1000 => Parity::Even,
            0b0010_1000 => Parity::Mark,
            0b0011_1000 => Parity::Space,
            _ => panic!("Invalid Parity! Please check your uart"),
        }
    }

    /// set parity
    pub fn set_parity(&self, parity: Parity) {
        match parity {
            Parity::No => unsafe { self.reg.lcr.modify(|v| (v & 0b1100_0111)) },
            Parity::Odd => unsafe { self.reg.lcr.modify(|v| (v & 0b1100_0111) | 0b0000_1000) },
            Parity::Even => unsafe { self.reg.lcr.modify(|v| (v & 0b1100_0111) | 0b0001_1000) },
            Parity::Mark => unsafe { self.reg.lcr.modify(|v| (v & 0b1100_0111) | 0b0010_1000) },
            Parity::Space => unsafe { self.reg.lcr.modify(|v| v | 0b0011_1000) },
        }
    }

    /// get stop bit of used data protocol
    ///
    /// Simply return a u8 to indicate 1 or 1.5/2 bits
    pub fn get_stop_bit(&self) -> u8 {
        ((self.reg.lcr.read() & 0b100) >> 2) + 1
    }

    /// set stop bit, only 1 and 2 can be used as `stop_bit`
    pub fn set_stop_bit(&self, stop_bit: u8) {
        match stop_bit {
            1 => unsafe { self.reg.lcr.modify(|v| v & 0b1111_1011) },
            2 => unsafe { self.reg.lcr.modify(|v| v | 0b0000_0100) },
            _ => panic!("Invalid stop bit"),
        }
    }

    /// get word length of used data protocol
    pub fn get_word_length(&self) -> u8 {
        (self.reg.lcr.read() & 0b11) + 5
    }

    /// set word length, only 5..=8 can be used as `length`
    pub fn set_word_length(&self, length: u8) {
        if (5..=8).contains(&length) {
            unsafe { self.reg.lcr.modify(|v| v | (length - 5)) }
        } else {
            panic!("Invalid word length")
        }
    }

    /// Read MCR (offset + 4)
    ///
    /// Read Modem Control Register to get how flow is controlled
    ///
    /// > ## Modem Control Register
    /// >
    /// > Offset: +4 . This register allows you to do "hardware" flow control, under software control. Or in a more practical manner, it allows direct manipulation of four different wires on the UART that you can set to any series of independent logical states, and be able to offer control of the modem. It should also be noted that most UARTs need Auxiliary Output 2 set to a logical "1" to enable interrupts.
    /// >
    /// > | Bit | Notes                            |
    /// > | --- | -------------------------------- |
    /// > | 7   | Reserved                         |
    /// > | 6   | Reserved                         |
    /// > | 5   | Autoflow Control Enabled (16750) |
    /// > | 4   | Loopback Mode                    |
    /// > | 3   | Auxiliary Output 2               |
    /// > | 2   | Auxiliary Output 1               |
    /// > | 1   | Request To Send                  |
    /// > | 0   | Data Terminal Ready              |
    #[inline]
    pub fn read_mcr(&self) -> u8 {
        self.reg.mcr.read()
    }

    /// Write MCR (offset + 4)
    ///
    /// Write Modem Control Register to control flow
    #[inline]
    pub fn write_mcr(&self, value: u8) {
        unsafe { self.reg.mcr.write(value) }
    }

    /// Read LSR (offset + 5)
    ///
    /// > ## Line Status Register
    /// >
    /// > Offset: +5 . This register is used primarily to give you information on possible error conditions that may exist within the UART, based on the data that has been received. Keep in mind that this is a "read only" register, and any data written to this register is likely to be ignored or worse, cause different behavior in the UART. There are several uses for this information, and some information will be given below on how it can be useful for diagnosing problems with your serial data connection:
    /// >
    /// > | Bit | Notes                              |
    /// > | --- | ---------------------------------- |
    /// > | 7   | Error in Received FIFO             |
    /// > | 6   | Empty Data Holding Registers       |
    /// > | 5   | Empty Transmitter Holding Register |
    /// > | 4   | Break Interrupt                    |
    /// > | 3   | Framing Error                      |
    /// > | 2   | Parity Error                       |
    /// > | 1   | Overrun Error                      |
    /// > | 0   | Data Ready                         |
    #[inline]
    pub fn read_lsr(&self) -> u8 {
        self.reg.lsr.read()
    }

    /// Get LSR bitflags
    #[inline]
    pub fn lsr(&self) -> LSR {
        LSR::from_bits_truncate(self.read_lsr())
    }

    /// get whether there is an error in received FIFO
    pub fn is_received_fifo_error(&self) -> bool {
        self.lsr().contains(LSR::RFE)
    }

    /// get whether data holding registers are empty
    pub fn is_data_holding_registers_empty(&self) -> bool {
        self.lsr().contains(LSR::DHRE)
    }

    /// get whether transmitter holding register is empty
    pub fn is_transmitter_holding_register_empty(&self) -> bool {
        self.lsr().contains(LSR::THRE)
    }

    pub fn is_break_interrupt(&self) -> bool {
        self.lsr().contains(LSR::BI)
    }

    pub fn is_framing_error(&self) -> bool {
        self.lsr().contains(LSR::FE)
    }

    pub fn is_parity_error(&self) -> bool {
        self.lsr().contains(LSR::PE)
    }

    pub fn is_overrun_error(&self) -> bool {
        self.lsr().contains(LSR::OE)
    }

    pub fn is_data_ready(&self) -> bool {
        self.lsr().contains(LSR::DR)
    }

    /// Read MSR (offset + 6)
    ///
    /// > ## Modem Status Register
    /// >
    /// > Offset: +6 . This register is another read-only register that is here to inform your software about the current status of the modem. The modem accessed in this manner can either be an external modem, or an internal modem that uses a UART as an interface to the computer.
    /// >
    /// > | Bit | Notes                        |
    /// > | --- | ---------------------------- |
    /// > | 7   | Carrier Detect               |
    /// > | 6   | Ring Indicator               |
    /// > | 5   | Data Set Ready               |
    /// > | 4   | Clear To Send                |
    /// > | 3   | Delta Data Carrier Detect    |
    /// > | 2   | Trailing Edge Ring Indicator |
    /// > | 1   | Delta Data Set Ready         |
    /// > | 0   | Delta Clear To Send          |
    #[inline]
    pub fn read_msr(&self) -> u8 {
        self.reg.msr.read()
    }

    /// Get MSR bitflags
    #[inline]
    pub fn msr(&self) -> MSR {
        MSR::from_bits_truncate(self.read_msr())
    }

    pub fn is_carrier_detect(&self) -> bool {
        self.msr().contains(MSR::CD)
    }

    pub fn is_ring_indicator(&self) -> bool {
        self.msr().contains(MSR::RI)
    }

    pub fn is_data_set_ready(&self) -> bool {
        self.msr().contains(MSR::DSR)
    }

    pub fn is_clear_to_send(&self) -> bool {
        self.msr().contains(MSR::CTS)
    }

    pub fn is_delta_data_carrier_detect(&self) -> bool {
        self.msr().contains(MSR::DDCD)
    }

    pub fn is_trailing_edge_ring_indicator(&self) -> bool {
        self.msr().contains(MSR::TERI)
    }

    pub fn is_delta_data_set_ready(&self) -> bool {
        self.msr().contains(MSR::DDSR)
    }

    pub fn is_delta_clear_to_send(&self) -> bool {
        self.msr().contains(MSR::DCTS)
    }

    #[inline]
    pub fn read_sr(&self) -> u8 {
        self.reg.scratch.read()
    }

    #[inline]
    pub fn write_sr(&self, value: u8) {
        unsafe { self.reg.scratch.write(value) }
    }
}

/// ## fmt::Write
///
/// A simple implementation, may be changed in the future
#[cfg(feature = "fmt")]
impl<'a> fmt::Write for MmioUart8250<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.as_bytes() {
            self.write_thr(*c);
        }
        Ok(())
    }
}
