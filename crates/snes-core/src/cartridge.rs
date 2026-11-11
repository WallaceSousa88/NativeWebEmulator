#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomType {
    LoRom,
    HiRom,
}

#[derive(Debug, Clone)]
pub struct CartridgeHeader {
    pub title: String,
    pub rom_type: RomType,
    pub rom_size: usize,
    pub ram_size: usize,
    pub country_code: u8,
    pub checksum: u16,
    pub nmi_vector: u16,
    pub reset_vector: u16,
    pub irq_vector: u16,
}

pub struct Cartridge {
    pub rom: Vec<u8>,
    pub ram: Vec<u8>,
    pub header: CartridgeHeader,
}

impl Cartridge {
    pub fn load(raw_data: &[u8]) -> Result<Self, &'static str> {
        if raw_data.len() < 0x8000 {
            return Err("ROM muito pequena para ser um jogo de SNES válido.");
        }

        // 1. Remover header de copiador (ex: cópias SMC/SWC com 512 bytes extras)
        let rom_data = if raw_data.len() % 1024 == 512 {
            &raw_data[512..]
        } else {
            raw_data
        };

        // 2. Detectar se é LoROM ou HiROM
        let (rom_type, header_offset) = Self::detect_rom_type(rom_data);

        // 3. Parsear o cabeçalho interno
        let header = Self::parse_header(rom_data, header_offset, rom_type)?;

        // 4. Alocar SRAM
        let ram_size = header.ram_size;
        let ram = vec![0u8; ram_size];

        Ok(Self {
            rom: rom_data.to_vec(),
            ram,
            header,
        })
    }

    fn detect_rom_type(rom: &[u8]) -> (RomType, usize) {
        fn score_header(rom: &[u8], offset: usize) -> i32 {
            if rom.len() < offset + 64 {
                return -100;
            }
            let mut score = 0;
            let checksum = u16::from_le_bytes([rom[offset + 0x1E], rom[offset + 0x1F]]);
            let complement = u16::from_le_bytes([rom[offset + 0x1C], rom[offset + 0x1D]]);
            if checksum ^ complement == 0xFFFF {
                score += 50;
            }

            let reset_vector = u16::from_le_bytes([rom[offset + 0x3C], rom[offset + 0x3D]]);
            if reset_vector >= 0x8000 {
                score += 20;
            }

            // Checar caracteres do título (ASCII válido)
            let mut valid_ascii = 0;
            for i in 0..21 {
                let c = rom[offset + i];
                if (0x20..=0x7E).contains(&c) {
                    valid_ascii += 1;
                }
            }
            score += valid_ascii;
            score
        }

        let lorom_score = score_header(rom, 0x7FC0);
        let hirom_score = score_header(rom, 0xFFC0);

        if hirom_score > lorom_score {
            (RomType::HiRom, 0xFFC0)
        } else {
            (RomType::LoRom, 0x7FC0)
        }
    }

    fn parse_header(rom: &[u8], offset: usize, rom_type: RomType) -> Result<CartridgeHeader, &'static str> {
        if rom.len() < offset + 0x40 {
            return Err("Tamanho insuficiente para o cabeçalho interno do SNES.");
        }

        // Título interno (21 bytes)
        let title_bytes = &rom[offset..offset + 21];
        let title = String::from_utf8_lossy(title_bytes)
            .trim()
            .to_string();

        let rom_size_byte = rom[offset + 0x17];
        let ram_size_byte = rom[offset + 0x18];
        let country_code = rom[offset + 0x19];
        let checksum = u16::from_le_bytes([rom[offset + 0x1E], rom[offset + 0x1F]]);

        let rom_size = if rom_size_byte > 0 { 1024 << rom_size_byte } else { rom.len() };
        let ram_size = if ram_size_byte > 0 && ram_size_byte < 16 {
            1024 << ram_size_byte
        } else {
            0
        };

        // Vetores de interrupção no modo nativo 65c816
        // Reset vector no modo de emulação fica em offset + 0x3C
        let nmi_vector = u16::from_le_bytes([rom[offset + 0x3A], rom[offset + 0x3B]]);
        let reset_vector = u16::from_le_bytes([rom[offset + 0x3C], rom[offset + 0x3D]]);
        let irq_vector = u16::from_le_bytes([rom[offset + 0x3E], rom[offset + 0x3F]]);

        Ok(CartridgeHeader {
            title,
            rom_type,
            rom_size,
            ram_size,
            country_code,
            checksum,
            nmi_vector,
            reset_vector,
            irq_vector,
        })
    }

    pub fn read(&self, bank: u8, addr: u16) -> u8 {
        match self.header.rom_type {
            RomType::LoRom => self.read_lorom(bank, addr),
            RomType::HiRom => self.read_hirom(bank, addr),
        }
    }

    pub fn write(&mut self, bank: u8, addr: u16, val: u8) {
        match self.header.rom_type {
            RomType::LoRom => self.write_lorom(bank, addr, val),
            RomType::HiRom => self.write_hirom(bank, addr, val),
        }
    }

    // =========================================================================
    // Mapeamento LoROM
    // Banks $00-$7D, $80-$FF : Addr $8000-$FFFF mapeiam ROM
    // Banks $70-$7D, $F0-$FF : Addr $0000-$7FFF mapeiam SRAM
    // =========================================================================
    fn read_lorom(&self, bank: u8, addr: u16) -> u8 {
        if addr >= 0x8000 {
            let rom_bank = (bank & 0x7F) as usize;
            let offset = (rom_bank << 15) | ((addr & 0x7FFF) as usize);
            if !self.rom.is_empty() {
                return self.rom[offset % self.rom.len()];
            }
        } else if (0x70..=0x7D).contains(&bank) || bank >= 0xF0 {
            if !self.ram.is_empty() {
                let ram_offset = ((bank as usize & 0x0F) << 15) | (addr as usize);
                return self.ram[ram_offset % self.ram.len()];
            }
        }
        0
    }

    fn write_lorom(&mut self, bank: u8, addr: u16, val: u8) {
        if addr < 0x8000 && ((0x70..=0x7D).contains(&bank) || bank >= 0xF0) {
            if !self.ram.is_empty() {
                let ram_offset = ((bank as usize & 0x0F) << 15) | (addr as usize);
                let idx = ram_offset % self.ram.len();
                self.ram[idx] = val;
            }
        }
    }

    // =========================================================================
    // Mapeamento HiROM
    // Banks $00-$3F, $80-$BF : Addr $8000-$FFFF mapeiam ROM
    // Banks $40-$7D, $C0-$FF : Addr $0000-$FFFF mapeiam ROM
    // Banks $20-$3F, $A0-$BF : Addr $6000-$7FFF mapeiam SRAM
    // =========================================================================
    fn read_hirom(&self, bank: u8, addr: u16) -> u8 {
        if (0x40..=0x7D).contains(&bank) || bank >= 0xC0 {
            let rom_bank = (bank & 0x3F) as usize;
            let offset = (rom_bank << 16) | (addr as usize);
            if !self.rom.is_empty() {
                return self.rom[offset % self.rom.len()];
            }
        } else if addr >= 0x8000 {
            let rom_bank = (bank & 0x3F) as usize;
            let offset = (rom_bank << 16) | (addr as usize);
            if !self.rom.is_empty() {
                return self.rom[offset % self.rom.len()];
            }
        } else if (0x6000..0x8000).contains(&addr) && ((0x20..=0x3F).contains(&bank) || (0xA0..=0xBF).contains(&bank)) {
            if !self.ram.is_empty() {
                let ram_offset = ((bank as usize & 0x1F) << 13) | ((addr & 0x1FFF) as usize);
                return self.ram[ram_offset % self.ram.len()];
            }
        }
        0
    }

    fn write_hirom(&mut self, bank: u8, addr: u16, val: u8) {
        if (0x6000..0x8000).contains(&addr) && ((0x20..=0x3F).contains(&bank) || (0xA0..=0xBF).contains(&bank)) {
            if !self.ram.is_empty() {
                let ram_offset = ((bank as usize & 0x1F) << 13) | ((addr & 0x1FFF) as usize);
                let idx = ram_offset % self.ram.len();
                self.ram[idx] = val;
            }
        }
    }
}
