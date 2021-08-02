use volatile_register::{RO, WO};

#[macro_export]
macro_rules! cast {
    ($expr:expr) => {
        unsafe { &mut *(($expr) as *mut crate::registers::Registers) };
    };
}

/// # UART Registers
#[repr(C)]
pub struct Registers {
    pub rx: RO<u32>,
    pub tx: WO<u32>,
    pub stat: RO<u32>,
    pub ctrl: WO<u32>,
}
