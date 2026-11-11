#[derive(Debug, Clone, Copy)]
pub struct DmaChannel {
    pub control: u8,      // $43x0 DMAP: Direção, modo de transferência
    pub dest_reg: u8,     // $43x1 BBAD: Registro de destino no barramento B ($21xx)
    pub src_addr: u16,    // $43x2-$43x3 A1TxL/H: Endereço fonte no barramento A
    pub src_bank: u8,     // $43x4 A1Bx: Banco fonte
    pub byte_count: u16,  // $43x5-$43x6 DASxL/H: Contador de bytes
    pub hdma_bank: u8,    // $43x7 DASBx
    pub a2_addr: u16,     // $43x8-$43x9 A2AxL/H
    pub line_counter: u8, // $43xA NTRLx
}

impl DmaChannel {
    pub fn new() -> Self {
        Self {
            control: 0xFF,
            dest_reg: 0xFF,
            src_addr: 0xFFFF,
            src_bank: 0xFF,
            byte_count: 0xFFFF,
            hdma_bank: 0xFF,
            a2_addr: 0xFFFF,
            line_counter: 0xFF,
        }
    }
}
