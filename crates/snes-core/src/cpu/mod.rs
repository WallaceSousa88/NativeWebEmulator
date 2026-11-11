pub mod registers;

use crate::bus::Bus;
use crate::cpu::registers::{flags, Registers};

pub struct Cpu {
    pub regs: Registers,
    pub cycles: u64,
    pub is_waiting: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            regs: Registers::new(),
            cycles: 0,
            is_waiting: false,
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) {
        self.regs = Registers::new();
        self.cycles = 0;
        self.is_waiting = false;

        // Reset vector no modo de emulação lê em $00:FFFC
        let reset_vector = bus.read_u16(0x00, 0xFFFC);
        self.regs.pc = reset_vector;
        self.regs.pbr = 0x00;
        self.regs.dbr = 0x00;
    }

    // =========================================================================
    // Operações de Pilha (Stack Push / Pull)
    // =========================================================================
    pub fn push_u8(&mut self, bus: &mut Bus, val: u8) {
        bus.write_u8(0x00, self.regs.sp, val);
        if self.regs.e {
            self.regs.sp = 0x0100 | ((self.regs.sp.wrapping_sub(1)) & 0xFF);
        } else {
            self.regs.sp = self.regs.sp.wrapping_sub(1);
        }
    }

    pub fn pull_u8(&mut self, bus: &mut Bus) -> u8 {
        if self.regs.e {
            self.regs.sp = 0x0100 | ((self.regs.sp.wrapping_add(1)) & 0xFF);
        } else {
            self.regs.sp = self.regs.sp.wrapping_add(1);
        }
        bus.read_u8(0x00, self.regs.sp)
    }

    pub fn push_u16(&mut self, bus: &mut Bus, val: u16) {
        self.push_u8(bus, ((val >> 8) & 0xFF) as u8);
        self.push_u8(bus, (val & 0xFF) as u8);
    }

    pub fn pull_u16(&mut self, bus: &mut Bus) -> u16 {
        let low = self.pull_u8(bus) as u16;
        let high = self.pull_u8(bus) as u16;
        (high << 8) | low
    }

    // =========================================================================
    // Busca de Instruções (Fetch)
    // =========================================================================
    #[inline(always)]
    pub fn fetch_u8(&mut self, bus: &mut Bus) -> u8 {
        let byte = bus.read_u8(self.regs.pbr, self.regs.pc);
        self.regs.pc = self.regs.pc.wrapping_add(1);
        byte
    }

    #[inline(always)]
    pub fn fetch_u16(&mut self, bus: &mut Bus) -> u16 {
        let low = self.fetch_u8(bus) as u16;
        let high = self.fetch_u8(bus) as u16;
        (high << 8) | low
    }

    // =========================================================================
    // Interrupção NMI (Disparada no início do V-Blank)
    // =========================================================================
    pub fn trigger_nmi(&mut self, bus: &mut Bus) {
        if self.regs.e {
            // Emulação 6502: Push PC (16-bit), Push P, Jmp ($FFFA)
            self.push_u16(bus, self.regs.pc);
            self.push_u8(bus, self.regs.p & !flags::X); // B flag é limpa
            self.regs.set_flag(flags::I, true);
            self.regs.set_flag(flags::D, false);
            self.regs.pc = bus.read_u16(0x00, 0xFFFA);
            self.regs.pbr = 0;
        } else {
            // Nativo 65c816: Push PBR, Push PC, Push P, Jmp ($FFEA)
            self.push_u8(bus, self.regs.pbr);
            self.push_u16(bus, self.regs.pc);
            self.push_u8(bus, self.regs.p);
            self.regs.set_flag(flags::I, true);
            self.regs.set_flag(flags::D, false);
            self.regs.pc = bus.read_u16(0x00, 0xFFEA);
            self.regs.pbr = 0;
        }
        self.is_waiting = false;
        self.cycles += 7;
    }

    // =========================================================================
    // Ciclo de Execução de Uma Instrução (Step)
    // =========================================================================
    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        if self.is_waiting {
            return 1;
        }

        let opcode = self.fetch_u8(bus);
        self.execute_opcode(bus, opcode)
    }

    fn execute_opcode(&mut self, bus: &mut Bus, op: u8) -> u32 {
        let m_8 = self.regs.is_m_8bit();
        let x_8 = self.regs.is_x_8bit();

        match op {
            // NOP
            0xEA => 2,

            // WAI (Wait for Interrupt)
            0xCB => {
                self.is_waiting = true;
                3
            }

            // XCE: Troca Carry com a Flag de Emulação (E)
            0x18 => {
                // CLC (Clear Carry)
                self.regs.set_flag(flags::C, false);
                2
            }
            0x38 => {
                // SEC (Set Carry)
                self.regs.set_flag(flags::C, true);
                2
            }
            0x58 => {
                // CLI (Clear Interrupt)
                self.regs.set_flag(flags::I, false);
                2
            }
            0x78 => {
                // SEI (Set Interrupt)
                self.regs.set_flag(flags::I, true);
                2
            }
            0xD8 => {
                // CLD (Clear Decimal)
                self.regs.set_flag(flags::D, false);
                2
            }
            0xF8 => {
                // SED (Set Decimal)
                self.regs.set_flag(flags::D, true);
                2
            }
            0xB8 => {
                // CLV (Clear Overflow)
                self.regs.set_flag(flags::V, false);
                2
            }

            // XCE (Exchange Carry and Emulation)
            0xFB => {
                let carry = self.regs.get_flag(flags::C);
                let emu = self.regs.e;
                self.regs.set_flag(flags::C, emu);
                self.regs.e = carry;
                if self.regs.e {
                    self.regs.p |= flags::M | flags::X;
                    self.regs.sp = 0x0100 | (self.regs.sp & 0xFF);
                }
                2
            }

            // REP: Reset Processor Status Bits
            0xC2 => {
                let mask = self.fetch_u8(bus);
                self.regs.p &= !mask;
                3
            }

            // SEP: Set Processor Status Bits
            0xE2 => {
                let mask = self.fetch_u8(bus);
                self.regs.p |= mask;
                3
            }

            // =================================================================
            // Cargas e Armazenamento (LDA, LDX, LDY, STA, STX, STY, STZ)
            // =================================================================
            // LDA Immediate
            0xA9 => {
                if m_8 {
                    let val = self.fetch_u8(bus);
                    self.regs.a = (self.regs.a & 0xFF00) | (val as u16);
                    self.regs.set_nz_8(val);
                    2
                } else {
                    let val = self.fetch_u16(bus);
                    self.regs.a = val;
                    self.regs.set_nz_16(val);
                    3
                }
            }

            // LDA Absolute
            0xAD => {
                let addr = self.fetch_u16(bus);
                if m_8 {
                    let val = bus.read_u8(self.regs.dbr, addr);
                    self.regs.a = (self.regs.a & 0xFF00) | (val as u16);
                    self.regs.set_nz_8(val);
                    4
                } else {
                    let val = bus.read_u16(self.regs.dbr, addr);
                    self.regs.a = val;
                    self.regs.set_nz_16(val);
                    5
                }
            }

            // LDA Direct Page
            0xA5 => {
                let dp_offset = self.fetch_u8(bus) as u16;
                let addr = self.regs.dp.wrapping_add(dp_offset);
                if m_8 {
                    let val = bus.read_u8(0x00, addr);
                    self.regs.a = (self.regs.a & 0xFF00) | (val as u16);
                    self.regs.set_nz_8(val);
                    3
                } else {
                    let val = bus.read_u16(0x00, addr);
                    self.regs.a = val;
                    self.regs.set_nz_16(val);
                    4
                }
            }

            // STA Absolute
            0x8D => {
                let addr = self.fetch_u16(bus);
                if m_8 {
                    bus.write_u8(self.regs.dbr, addr, (self.regs.a & 0xFF) as u8);
                    4
                } else {
                    bus.write_u16(self.regs.dbr, addr, self.regs.a);
                    5
                }
            }

            // STA Direct Page
            0x85 => {
                let dp_offset = self.fetch_u8(bus) as u16;
                let addr = self.regs.dp.wrapping_add(dp_offset);
                if m_8 {
                    bus.write_u8(0x00, addr, (self.regs.a & 0xFF) as u8);
                    3
                } else {
                    bus.write_u16(0x00, addr, self.regs.a);
                    4
                }
            }

            // LDX Immediate
            0xA2 => {
                if x_8 {
                    let val = self.fetch_u8(bus);
                    self.regs.x = val as u16;
                    self.regs.set_nz_8(val);
                    2
                } else {
                    let val = self.fetch_u16(bus);
                    self.regs.x = val;
                    self.regs.set_nz_16(val);
                    3
                }
            }

            // LDX Absolute
            0xAE => {
                let addr = self.fetch_u16(bus);
                if x_8 {
                    let val = bus.read_u8(self.regs.dbr, addr);
                    self.regs.x = val as u16;
                    self.regs.set_nz_8(val);
                    4
                } else {
                    let val = bus.read_u16(self.regs.dbr, addr);
                    self.regs.x = val;
                    self.regs.set_nz_16(val);
                    5
                }
            }

            // LDY Immediate
            0xA0 => {
                if x_8 {
                    let val = self.fetch_u8(bus);
                    self.regs.y = val as u16;
                    self.regs.set_nz_8(val);
                    2
                } else {
                    let val = self.fetch_u16(bus);
                    self.regs.y = val;
                    self.regs.set_nz_16(val);
                    3
                }
            }

            // STX Absolute
            0x8E => {
                let addr = self.fetch_u16(bus);
                if x_8 {
                    bus.write_u8(self.regs.dbr, addr, (self.regs.x & 0xFF) as u8);
                    4
                } else {
                    bus.write_u16(self.regs.dbr, addr, self.regs.x);
                    5
                }
            }

            // STY Absolute
            0x8C => {
                let addr = self.fetch_u16(bus);
                if x_8 {
                    bus.write_u8(self.regs.dbr, addr, (self.regs.y & 0xFF) as u8);
                    4
                } else {
                    bus.write_u16(self.regs.dbr, addr, self.regs.y);
                    5
                }
            }

            // STZ Absolute (Store Zero)
            0x9C => {
                let addr = self.fetch_u16(bus);
                if m_8 {
                    bus.write_u8(self.regs.dbr, addr, 0);
                    4
                } else {
                    bus.write_u16(self.regs.dbr, addr, 0);
                    5
                }
            }

            // STZ Direct Page
            0x64 => {
                let dp_offset = self.fetch_u8(bus) as u16;
                let addr = self.regs.dp.wrapping_add(dp_offset);
                if m_8 {
                    bus.write_u8(0x00, addr, 0);
                    3
                } else {
                    bus.write_u16(0x00, addr, 0);
                    4
                }
            }

            // =================================================================
            // Transferências Entre Registradores (TAX, TXA, TAY, TYA, etc.)
            // =================================================================
            0xAA => { // TAX
                if x_8 {
                    self.regs.x = self.regs.a & 0xFF;
                    self.regs.set_nz_8(self.regs.x as u8);
                } else {
                    self.regs.x = self.regs.a;
                    self.regs.set_nz_16(self.regs.x);
                }
                2
            }
            0x8A => { // TXA
                if m_8 {
                    self.regs.a = (self.regs.a & 0xFF00) | (self.regs.x & 0xFF);
                    self.regs.set_nz_8((self.regs.a & 0xFF) as u8);
                } else {
                    self.regs.a = self.regs.x;
                    self.regs.set_nz_16(self.regs.a);
                }
                2
            }
            0xA8 => { // TAY
                if x_8 {
                    self.regs.y = self.regs.a & 0xFF;
                    self.regs.set_nz_8(self.regs.y as u8);
                } else {
                    self.regs.y = self.regs.a;
                    self.regs.set_nz_16(self.regs.y);
                }
                2
            }
            0x98 => { // TYA
                if m_8 {
                    self.regs.a = (self.regs.a & 0xFF00) | (self.regs.y & 0xFF);
                    self.regs.set_nz_8((self.regs.a & 0xFF) as u8);
                } else {
                    self.regs.a = self.regs.y;
                    self.regs.set_nz_16(self.regs.a);
                }
                2
            }
            0xBA => { // TSX
                self.regs.x = self.regs.sp;
                if x_8 {
                    self.regs.x &= 0xFF;
                    self.regs.set_nz_8(self.regs.x as u8);
                } else {
                    self.regs.set_nz_16(self.regs.x);
                }
                2
            }
            0x9A => { // TXS
                if self.regs.e {
                    self.regs.sp = 0x0100 | (self.regs.x & 0xFF);
                } else {
                    self.regs.sp = self.regs.x;
                }
                2
            }
            0x5B => { // TCD (Transfer C/Accumulator to Direct Page)
                self.regs.dp = self.regs.a;
                self.regs.set_nz_16(self.regs.dp);
                2
            }
            0x7B => { // TDC (Transfer Direct Page to C/Accumulator)
                self.regs.a = self.regs.dp;
                self.regs.set_nz_16(self.regs.a);
                2
            }

            // =================================================================
            // Saltos, Chamadas e Retornos (JMP, JML, JSR, JSL, RTS, RTL)
            // =================================================================
            // JMP Absolute
            0x4C => {
                let target = self.fetch_u16(bus);
                self.regs.pc = target;
                3
            }

            // JML Absolute Long (Jump Long 24-bit)
            0x5C => {
                let target_pc = self.fetch_u16(bus);
                let target_bank = self.fetch_u8(bus);
                self.regs.pc = target_pc;
                self.regs.pbr = target_bank;
                4
            }

            // JSR Absolute
            0x20 => {
                let target = self.fetch_u16(bus);
                let ret_pc = self.regs.pc.wrapping_sub(1);
                self.push_u16(bus, ret_pc);
                self.regs.pc = target;
                6
            }

            // JSL Absolute Long (Jump to Subroutine Long)
            0x22 => {
                let target_pc = self.fetch_u16(bus);
                let target_bank = self.fetch_u8(bus);
                let ret_pc = self.regs.pc.wrapping_sub(1);
                self.push_u8(bus, self.regs.pbr);
                self.push_u16(bus, ret_pc);
                self.regs.pc = target_pc;
                self.regs.pbr = target_bank;
                8
            }

            // RTS (Return from Subroutine)
            0x60 => {
                let ret_pc = self.pull_u16(bus);
                self.regs.pc = ret_pc.wrapping_add(1);
                6
            }

            // RTL (Return from Subroutine Long)
            0x6B => {
                let ret_pc = self.pull_u16(bus);
                let ret_bank = self.pull_u8(bus);
                self.regs.pc = ret_pc.wrapping_add(1);
                self.regs.pbr = ret_bank;
                6
            }

            // RTI (Return from Interrupt)
            0x40 => {
                let p = self.pull_u8(bus);
                self.regs.p = p;
                self.regs.pc = self.pull_u16(bus);
                if !self.regs.e {
                    self.regs.pbr = self.pull_u8(bus);
                }
                6
            }

            // =================================================================
            // Desvios Relativos Condicionais (Branching)
            // =================================================================
            0x80 => { // BRA (Branch Always)
                let offset = self.fetch_u8(bus) as i8 as i16;
                self.regs.pc = ((self.regs.pc as i32) + (offset as i32)) as u16;
                3
            }
            0x82 => { // BRL (Branch Relative Long 16-bit)
                let offset = self.fetch_u16(bus) as i16;
                self.regs.pc = ((self.regs.pc as i32) + (offset as i32)) as u16;
                4
            }
            0xF0 => self.branch_if(bus, self.regs.get_flag(flags::Z)),  // BEQ
            0xD0 => self.branch_if(bus, !self.regs.get_flag(flags::Z)), // BNE
            0xB0 => self.branch_if(bus, self.regs.get_flag(flags::C)),  // BCS
            0x90 => self.branch_if(bus, !self.regs.get_flag(flags::C)), // BCC
            0x30 => self.branch_if(bus, self.regs.get_flag(flags::N)),  // BMI
            0x10 => self.branch_if(bus, !self.regs.get_flag(flags::N)), // BPL
            0x70 => self.branch_if(bus, self.regs.get_flag(flags::V)),  // BVS
            0x50 => self.branch_if(bus, !self.regs.get_flag(flags::V)), // BVC

            // =================================================================
            // Comparações e Aritmética Básica (CMP, CPX, CPY, INX, DEX, etc.)
            // =================================================================
            0xC9 => { // CMP Immediate
                if m_8 {
                    let val = self.fetch_u8(bus);
                    let a = (self.regs.a & 0xFF) as u8;
                    self.regs.set_flag(flags::C, a >= val);
                    self.regs.set_nz_8(a.wrapping_sub(val));
                    2
                } else {
                    let val = self.fetch_u16(bus);
                    self.regs.set_flag(flags::C, self.regs.a >= val);
                    self.regs.set_nz_16(self.regs.a.wrapping_sub(val));
                    3
                }
            }
            0xE8 => { // INX
                if x_8 {
                    let val = ((self.regs.x & 0xFF) as u8).wrapping_add(1);
                    self.regs.x = val as u16;
                    self.regs.set_nz_8(val);
                } else {
                    self.regs.x = self.regs.x.wrapping_add(1);
                    self.regs.set_nz_16(self.regs.x);
                }
                2
            }
            0xCA => { // DEX
                if x_8 {
                    let val = ((self.regs.x & 0xFF) as u8).wrapping_sub(1);
                    self.regs.x = val as u16;
                    self.regs.set_nz_8(val);
                } else {
                    self.regs.x = self.regs.x.wrapping_sub(1);
                    self.regs.set_nz_16(self.regs.x);
                }
                2
            }
            0xC8 => { // INY
                if x_8 {
                    let val = ((self.regs.y & 0xFF) as u8).wrapping_add(1);
                    self.regs.y = val as u16;
                    self.regs.set_nz_8(val);
                } else {
                    self.regs.y = self.regs.y.wrapping_add(1);
                    self.regs.set_nz_16(self.regs.y);
                }
                2
            }
            0x88 => { // DEY
                if x_8 {
                    let val = ((self.regs.y & 0xFF) as u8).wrapping_sub(1);
                    self.regs.y = val as u16;
                    self.regs.set_nz_8(val);
                } else {
                    self.regs.y = self.regs.y.wrapping_sub(1);
                    self.regs.set_nz_16(self.regs.y);
                }
                2
            }

            // Opcode não implementado / fallback padrão
            _ => 2,
        }
    }

    fn branch_if(&mut self, bus: &mut Bus, condition: bool) -> u32 {
        let offset = self.fetch_u8(bus) as i8 as i16;
        if condition {
            self.regs.pc = ((self.regs.pc as i32) + (offset as i32)) as u16;
            3
        } else {
            2
        }
    }
}
