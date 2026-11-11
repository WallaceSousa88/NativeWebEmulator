pub mod flags {
    pub const C: u8 = 1 << 0; // Carry
    pub const Z: u8 = 1 << 1; // Zero
    pub const I: u8 = 1 << 2; // IRQ Disable
    pub const D: u8 = 1 << 3; // Decimal Mode
    pub const X: u8 = 1 << 4; // Index Size (0 = 16-bit, 1 = 8-bit) ou Break Flag em emulação
    pub const M: u8 = 1 << 5; // Memory/Accumulator Size (0 = 16-bit, 1 = 8-bit)
    pub const V: u8 = 1 << 6; // Overflow
    pub const N: u8 = 1 << 7; // Negative
}

#[derive(Debug, Clone)]
pub struct Registers {
    pub a: u16,      // Acumulador (ou A/B em 8-bit)
    pub x: u16,      // Índice X
    pub y: u16,      // Índice Y
    pub sp: u16,     // Stack Pointer
    pub dp: u16,     // Direct Page
    pub pc: u16,     // Program Counter
    pub pbr: u8,     // Program Bank Register
    pub dbr: u8,     // Data Bank Register
    pub p: u8,       // Status Register (Flags)
    pub e: bool,     // Emulation Mode (true no boot/reset)
}

impl Registers {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0x01FF,
            dp: 0,
            pc: 0,
            pbr: 0,
            dbr: 0,
            p: flags::M | flags::X | flags::I, // Padrão de boot: M=1, X=1, I=1
            e: true,                            // Inicia no modo de emulação 6502
        }
    }

    #[inline(always)]
    pub fn is_m_8bit(&self) -> bool {
        self.e || (self.p & flags::M) != 0
    }

    #[inline(always)]
    pub fn is_x_8bit(&self) -> bool {
        self.e || (self.p & flags::X) != 0
    }

    #[inline(always)]
    pub fn set_flag(&mut self, flag: u8, val: bool) {
        if val {
            self.p |= flag;
        } else {
            self.p &= !flag;
        }
    }

    #[inline(always)]
    pub fn get_flag(&self, flag: u8) -> bool {
        (self.p & flag) != 0
    }

    #[inline(always)]
    pub fn set_nz_8(&mut self, val: u8) {
        self.set_flag(flags::Z, val == 0);
        self.set_flag(flags::N, (val & 0x80) != 0);
    }

    #[inline(always)]
    pub fn set_nz_16(&mut self, val: u16) {
        self.set_flag(flags::Z, val == 0);
        self.set_flag(flags::N, (val & 0x8000) != 0);
    }
}
