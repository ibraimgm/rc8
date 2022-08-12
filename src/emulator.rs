use std::io::Read;

// memory size
const MEM_SIZE: usize = 4096;

// minimum subroutine stack size (to preallocate)
const MIN_SUB_STACK_SIZE: usize = 12;

// start and end of the free are for user programs
// end address is inclusive
const ADDR_START: usize = 0x200;
const ADDR_END: usize = 0xE8F;

// rom size
const MAX_ROM_SIZE: usize = ADDR_END - ADDR_START + 1;

#[derive(Debug)]
pub enum Error {
    ProgramTerminated,
    InvalidInstruction(u8, u8, u16),
    InvalidJump(u8, u8, u16),
    IoError(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Error::ProgramTerminated => write!(f, "Program reached last instruction"),
            Error::InvalidInstruction(a, b, addr) => write!(
                f,
                "Invalid instruction at address {:#05X}: {:02X}{:02X}",
                addr, a, b
            ),
            Error::InvalidJump(a, b, addr) => write!(
                f,
                "Invalid jump on instruction at address {:#05X}: {:02X}{:02X}",
                addr, a, b
            ),
            Error::IoError(err) => write!(f, "IO Error: {}", err),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError(err)
    }
}

#[inline(always)]
fn nibble_h(b: u8) -> u8 {
    (b >> 4) & 0xF
}

#[inline(always)]
fn nibble_l(b: u8) -> u8 {
    b & 0xF
}

#[inline(always)]
fn nnn(a: u8, b: u8) -> u16 {
    (((a as u16) << 8) | (b as u16)) & 0xFFF
}

#[allow(non_snake_case)]
struct Emulator {
    // program counter
    pub PC: usize,

    // full memory
    pub memory: [u8; MEM_SIZE],

    // data registers: V0 - VF
    pub V: [u8; 16],

    // address register
    pub I: u16,

    // subroutine stack (min. 12 is required)
    pub sub_stack: Vec<usize>,

    // delay timer
    pub DT: u8,

    // sound timer
    pub ST: u8,
}

impl Emulator {
    /// Load a chip-8 rom, up to the maximum allowed rom size.
    pub fn load_rom<T>(rom: T) -> Result<Self, Error>
    where
        T: Read,
    {
        let mut emu = Emulator {
            PC: ADDR_START,
            memory: [0u8; MEM_SIZE],
            V: [0u8; 16],
            I: 0,
            sub_stack: Vec::with_capacity(MIN_SUB_STACK_SIZE),
            DT: 0,
            ST: 0,
        };

        let mut rom = rom.take((MAX_ROM_SIZE) as u64);
        let mut total_read = ADDR_START;

        loop {
            let bytes_read = rom.read(&mut emu.memory[total_read..ADDR_END + 1])?;
            if bytes_read == 0 {
                break;
            } else {
                total_read += bytes_read
            }
        }

        Ok(emu)
    }

    /// Execute a single chip-8 CPU instruction.
    pub fn execute(&mut self) -> Result<(), Error> {
        if self.PC > ADDR_END {
            return Err(Error::ProgramTerminated);
        }

        // read a command
        let a = self.memory[self.PC];
        let b = self.memory[self.PC + 1];
        self.PC += 2;

        // choose the instruction to run
        match nibble_h(a) {
            0x0 if a == 0x00 && b == 0xE0 => {
                todo!("clear screen");
            }
            0x0 if a == 0x00 && b == 0xEE => {
                todo!("return from subroutine");
            }
            0x0 => {
                todo!("execute machine subroutine");
            }
            // 1NNN - jump to address NNN
            0x1 => {
                self.PC = nnn(a, b) as usize;
            }
            0x2 => {
                todo!("execute subroutine")
            }
            // 3XNN - skip next if VX == NN
            0x3 => {
                let index = nibble_l(a) as usize;
                if self.V[index] == b {
                    self.PC += 2;
                }
            }
            // 4XNN - skip next if VX != NN
            0x4 => {
                let index = nibble_l(a) as usize;
                if self.V[index] != b {
                    self.PC += 2;
                }
            }
            // 5XY0 - skip next if VX == VY
            0x5 if nibble_l(b) == 0x0 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                if self.V[x] == self.V[y] {
                    self.PC += 2;
                }
            }
            // 6XNN - Set VX to NN
            0x6 => {
                let index = nibble_l(a) as usize;
                self.V[index] = b;
            }
            // 7XNN - Set VX to VX + NN (ignore VF)
            0x7 => {
                let index = nibble_l(a) as usize;
                let (result, _) = self.V[index].overflowing_add(b);
                self.V[index] = result;
            }
            // 8XY0 - Set VX = VY
            0x8 if nibble_l(b) == 0x0 => {
                let dst = nibble_l(a) as usize;
                let src = nibble_h(b) as usize;
                self.V[dst] = self.V[src];
            }
            // 8XY1 - Set VX = VX | VY
            0x8 if nibble_l(b) == 0x1 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                self.V[x] |= self.V[y];
            }
            // 8XY2 - Set VX = VX & VY
            0x8 if nibble_l(b) == 0x2 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                self.V[x] &= self.V[y];
            }
            // 8XY3 - Set VX = VX ^ VY
            0x8 if nibble_l(b) == 0x3 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                self.V[x] ^= self.V[y];
            }
            // 8XY4 - Set VX = VX + VY, set VF to 1 if carry
            0x8 if nibble_l(b) == 0x4 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                let (result, carry) = self.V[x].overflowing_add(self.V[y]);
                self.V[x] = result;
                self.V[0xF] = carry as u8;
            }
            // 8XY5 - Set VX = VX - VY, set VF to 0 if borrow
            0x8 if nibble_l(b) == 0x5 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                let (result, carry) = self.V[x].overflowing_sub(self.V[y]);
                self.V[x] = result;
                self.V[0xF] = (!carry) as u8;
            }
            // 8XY6 - Set VX = VY >> 1; set VF to shifted bit
            0x8 if nibble_l(b) == 0x6 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                self.V[0xF] = self.V[y] & 1;
                self.V[x] = self.V[y] >> 1;
            }
            // 8XY7 - Set VX = VY - VX, set VF to 0 if borrow
            0x8 if nibble_l(b) == 0x7 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                let (result, carry) = self.V[y].overflowing_sub(self.V[x]);
                self.V[x] = result;
                self.V[0xF] = (!carry) as u8;
            }
            // 8XYE - Set VX = VY << 1; set VF to shitfted bit
            0x8 if nibble_l(b) == 0xE => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                self.V[0xF] = self.V[y] >> 7;
                self.V[x] = self.V[y] << 1;
            }
            // 9XY0 - skip next if VX != VY
            0x9 if nibble_l(b) == 0x0 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                if self.V[x] != self.V[y] {
                    self.PC += 2;
                }
            }
            // ANNN - Set I = NNN
            0xA => {
                self.I = nnn(a, b);
            }
            // 0xBNNN - Jump doaddress NNN + V0
            0xB => {
                let addr = ((self.V[0x0] as u16) + nnn(a, b)) as usize;
                if addr >= MEM_SIZE {
                    self.PC -= 2;
                    return Err(Error::InvalidJump(a, b, self.PC as u16));
                }
                self.PC = addr;
            }
            0xC => {
                todo!("set Vx to a random number with mask NN")
            }
            0xD => {
                todo!("Draw sprite at address I, with coords VX, VY, with size N; set VF to 1 is any pixel is cleared")
            }
            0xE if b == 0x9E => {
                todo!("skip next if key VX is pressed")
            }
            0xE if b == 0xA1 => {
                todo!("skip next if key VX is not pressed")
            }
            // FX07 - Store the DT value into VX
            0xF if b == 0x07 => {
                let x = nibble_l(a) as usize;
                self.V[x] = self.DT;
            }
            0xF if b == 0x0A => {
                todo!("wait for key and store in VX")
            }
            // FX15 - Store the VX value into DT
            0xF if b == 0x15 => {
                let x = nibble_l(a) as usize;
                self.DT = self.V[x];
            }
            // FX18 - Store the VX value into ST
            0xF if b == 0x18 => {
                let x = nibble_l(a) as usize;
                self.ST = self.V[x];
            }
            // FX1E - Set I = I + VX
            0xF if b == 0x1E => {
                let x = nibble_l(a) as usize;
                self.I = self.I.wrapping_add(self.V[x] as u16);
            }
            0xF if b == 0x29 => {
                todo!("store on I the address of sprite of digit on VX")
            }
            // FX33 - Store BCD of VX into I, I+I and I+2
            0xF if b == 0x33 => {
                let x = nibble_l(a) as usize;
                let i = self.I as usize;
                self.memory[i] = self.V[x] / 100;
                self.memory[i + 1] = self.V[x] / 10 % 10;
                self.memory[i + 2] = self.V[x] % 100 % 10;
            }
            0xF if b == 0x55 => {
                todo!("store values of V0 to VX starting into I, ends I at I + X + 1")
            }
            0xF if b == 0x65 => {
                todo!("read values into V0 to VX starting from I, ends I at I + X + 1")
            }
            _ => return Err(Error::InvalidInstruction(a, b, (self.PC - 2) as u16)),
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec_cycles(emu: &mut Emulator, mut cycles: i32) {
        while cycles > 0 {
            emu.execute().unwrap();
            cycles -= 1;
        }
    }

    #[test]
    fn test_nibble() {
        let a = 0x12;
        let b = 0x34;

        assert_eq!(nibble_h(a), 0x1);
        assert_eq!(nibble_l(a), 0x2);
        assert_eq!(nibble_h(b), 0x3);
        assert_eq!(nibble_l(b), 0x4);
        assert_eq!(nnn(a, b), 0x234);
    }

    #[test]
    fn test_load_small_rom() {
        let rom = [0xFFu8; 10];
        let emu = Emulator::load_rom(&rom[..]).unwrap();

        assert_eq!(emu.memory[ADDR_START], 0xFF);
        assert_eq!(emu.memory[ADDR_START + 1], 0xFF);
        assert_eq!(emu.memory[ADDR_START + 2], 0xFF);
        assert_eq!(emu.memory[ADDR_START + 3], 0xFF);
        assert_eq!(emu.memory[ADDR_START + 4], 0xFF);
        assert_eq!(emu.memory[ADDR_START + 5], 0xFF);
        assert_eq!(emu.memory[ADDR_START + 6], 0xFF);
        assert_eq!(emu.memory[ADDR_START + 7], 0xFF);
        assert_eq!(emu.memory[ADDR_START + 8], 0xFF);
        assert_eq!(emu.memory[ADDR_START + 9], 0xFF);
        assert_eq!(emu.memory[ADDR_START + 10], 0x00);
    }

    #[test]
    fn test_load_big_rom_limit() {
        let rom = [0xEE; MAX_ROM_SIZE * 2];
        let emu = Emulator::load_rom(&rom[..]).unwrap();

        assert_eq!(emu.memory[ADDR_START], 0xEE);
        assert_eq!(emu.memory[ADDR_START + 1], 0xEE);
        assert_eq!(emu.memory[ADDR_START + 2], 0xEE);
        assert_eq!(emu.memory[ADDR_END], 0xEE);
        assert_eq!(emu.memory[ADDR_END - 1], 0xEE);
        assert_eq!(emu.memory[ADDR_END - 2], 0xEE);
        assert_eq!(emu.memory[ADDR_END + 1], 0x00);
        assert_eq!(emu.memory[ADDR_END + 2], 0x00);
    }

    #[test]
    fn test_load_rom_exact() {
        let mut rom = [0xFF; MAX_ROM_SIZE];
        rom[0] = 0xAA;
        rom[1] = 0xBB;
        rom[2] = 0xCC;
        rom[MAX_ROM_SIZE - 1] = 0xAA;
        rom[MAX_ROM_SIZE - 2] = 0xBB;
        rom[MAX_ROM_SIZE - 3] = 0xCC;

        let emu = Emulator::load_rom(&rom[..]).unwrap();
        assert_eq!(emu.memory[ADDR_START], 0xAA);
        assert_eq!(emu.memory[ADDR_START + 1], 0xBB);
        assert_eq!(emu.memory[ADDR_START + 2], 0xCC);
        assert_eq!(emu.memory[ADDR_START + 3], 0xFF);
        assert_eq!(emu.memory[ADDR_END], 0xAA);
        assert_eq!(emu.memory[ADDR_END - 1], 0xBB);
        assert_eq!(emu.memory[ADDR_END - 2], 0xCC);
        assert_eq!(emu.memory[ADDR_END - 3], 0xFF);
        assert_eq!(emu.memory[ADDR_END + 1], 0x00);
    }

    #[test]
    fn test_jump_to_address() {
        let rom: [u8; 2] = [
            0x12, 0x34, // 0x200: JMP 0x234
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        assert_eq!(emu.PC, ADDR_START);

        emu.execute().unwrap();
        assert_eq!(emu.PC, 0x234);
    }

    #[test]
    fn test_store_in_register() {
        let rom: [u8; 32] = [
            0x60, 0x01, // 0x200: SET V0 = 0x01
            0x61, 0x02, // 0x202: SET V1 = 0x02
            0x62, 0x03, // 0x204: SET V2 = 0x03
            0x63, 0x04, // 0x206: SET V3 = 0x04
            0x64, 0x05, // 0x208: SET V4 = 0x05
            0x65, 0x06, // 0x20A: SET V5 = 0x06
            0x66, 0x07, // 0x20C: SET V6 = 0x07
            0x67, 0x08, // 0x20E: SET V7 = 0x08
            0x68, 0x09, // 0x210: SET V8 = 0x09
            0x69, 0x0A, // 0x212: SET V9 = 0x0A
            0x6A, 0x0B, // 0x214: SET VA = 0x0B
            0x6B, 0x0C, // 0x216: SET VB = 0x0C
            0x6C, 0x0D, // 0x218: SET VC = 0x0D
            0x6D, 0x0E, // 0x21A: SET VD = 0x0E
            0x6E, 0x0F, // 0x21C: SET VE = 0x0F
            0x6F, 0x10, // 0x21E: SET VF = 0x10
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        let mut expected = 1u8;
        for i in 0..16 {
            emu.execute().unwrap();
            assert_eq!(emu.V[i], expected);
            expected += 1;
        }
        assert_eq!(emu.PC, 0x220);
    }

    #[test]
    fn test_set_between_registers() {
        let rom: [u8; 4] = [
            0x65, 0xFA, // 0x200: SET V5 = 0xFA
            0x75, 0x14, // 0x202: SET V5 = V5 + 0x14 (overflow)
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        assert_eq!(emu.V[0x5], 0x00);

        emu.execute().unwrap();
        assert_eq!(emu.V[0x5], 0xFA);

        emu.execute().unwrap();
        assert_eq!(emu.V[0x5], 0x0E);
        assert_eq!(emu.PC, 0x204);
    }

    #[test]
    fn test_add_const_to_register() {
        let rom: [u8; 4] = [
            0x60, 0xAA, // 0x200: SET V0 = 0xAA
            0x8A, 0x00, // 0x202: SET VA = V0
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        assert_eq!(emu.V[0x0], 0);
        assert_eq!(emu.V[0xA], 0);

        emu.execute().unwrap();
        assert_eq!(emu.V[0x0], 0xAA);
        assert_eq!(emu.V[0xA], 0);

        emu.execute().unwrap();
        assert_eq!(emu.V[0x0], 0xAA);
        assert_eq!(emu.V[0xA], 0xAA);
        assert_eq!(emu.PC, 0x204);
    }

    #[test]
    fn test_skip_if_eq_value() {
        let rom: [u8; 8] = [
            0x60, 0x01, // 0x200: SET V0 = 0x01
            0x30, 0x01, // 0x202: SKIPEQL V0,0x01
            0x61, 0x02, // 0x204: SET V1 = 0x02 (skipped)
            0x62, 0x03, // 0x206: SET V2 = 0x03
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        assert_eq!(emu.V[0x0], 0);
        assert_eq!(emu.V[0x1], 0);
        assert_eq!(emu.V[0x2], 0);

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0x01);
        assert_eq!(emu.V[0x1], 0x00);
        assert_eq!(emu.V[0x2], 0x03);
        assert_eq!(emu.PC, 0x208);
    }

    #[test]
    fn test_skip_if_neq_value() {
        let rom: [u8; 6] = [
            0x40, 0x01, // 0x200: SKIPNEQ V0,0x01
            0x60, 0x01, // 0x202: SET V0 = 0x01 (skipped)
            0x61, 0x01, // 0x204: SET V1 = 0x01
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        assert_eq!(emu.V[0x0], 0);
        assert_eq!(emu.V[0x1], 0);

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0);
        assert_eq!(emu.V[0x1], 0x01);
        assert_eq!(emu.PC, 0x206);
    }

    #[test]
    fn test_skip_if_eq_register() {
        let rom: [u8; 10] = [
            0x60, 0x01, // 0x200: SET V0 = 0x01
            0x61, 0x01, // 0x202: SET V1 = 0x01
            0x50, 0x10, // 0x204: SKIPEQ V0,V1
            0x62, 0x01, // 0x206: SET V2 = 0x01 (skipped)
            0x63, 0x01, // 0x208: SET V3 = 0x01
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        assert_eq!(emu.V[0x0], 0);
        assert_eq!(emu.V[0x1], 0);
        assert_eq!(emu.V[0x2], 0);
        assert_eq!(emu.V[0x3], 0);

        exec_cycles(&mut emu, 4);
        assert_eq!(emu.V[0x0], 1);
        assert_eq!(emu.V[0x1], 1);
        assert_eq!(emu.V[0x2], 0);
        assert_eq!(emu.V[0x3], 1);
        assert_eq!(emu.PC, 0x20A);
    }

    #[test]
    fn test_skip_if_neq_register() {
        let rom: [u8; 8] = [
            0x60, 0x01, // 0x200: SET V0 = 0x01
            0x90, 0x10, // 0x202: SKIPNEQ V0,V1
            0x61, 0x01, // 0x204: SET V1 = 0x01 (skipped)
            0x62, 0x01, // 0x206: SET V2 = 0x01
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        assert_eq!(emu.V[0x0], 0);
        assert_eq!(emu.V[0x1], 0);
        assert_eq!(emu.V[0x2], 0);

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 1);
        assert_eq!(emu.V[0x1], 0);
        assert_eq!(emu.V[0x2], 1);
        assert_eq!(emu.PC, 0x208);
    }

    #[test]
    fn test_bitwise_or() {
        let rom: [u8; 6] = [
            0x60, 0xBB, // 0x200: SET V0 = 0xBB
            0x61, 0x5A, // 0x202: SET V1 = 0x5A
            0x80, 0x11, // 0x204: SET V0 = V0 | V1
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0xFB);
        assert_eq!(emu.PC, 0x206);
    }

    #[test]
    fn test_bitwise_and() {
        let rom: [u8; 6] = [
            0x60, 0xBB, // 0x200: SET V0 = 0xBB
            0x61, 0x5A, // 0x202: SET V1 = 0x5A
            0x80, 0x12, // 0x204: SET V0 = V0 & V1
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0x1A);
        assert_eq!(emu.PC, 0x206);
    }

    #[test]
    fn test_bitwise_xor() {
        let rom: [u8; 6] = [
            0x60, 0xBB, // 0x200: SET V0 = 0xBB
            0x61, 0x5A, // 0x202: SET V1 = 0x5A
            0x80, 0x13, // 0x204: SET V0 = V0 ^ V1
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0xE1);
        assert_eq!(emu.PC, 0x206);
    }

    #[test]
    fn test_plus_register() {
        let rom: [u8; 10] = [
            0x60, 0x8A, // 0x200: Set V0 = 0x8A
            0x61, 0x22, // 0x202: Set V1 = 0x22
            0x80, 0x14, // 0x204: Set V0 = V0 + V1 (normal) - AC
            0x61, 0xE0, // 0x206: Set V1 = 0xE0
            0x80, 0x14, // 0x208: Set V0 = V0 + V1 (overflow) - 8C
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0xAC);
        assert_eq!(emu.V[0x1], 0x22);
        assert_eq!(emu.V[0xF], 0x00);

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0x8C);
        assert_eq!(emu.V[0x1], 0xE0);
        assert_eq!(emu.V[0xF], 0x01);
        assert_eq!(emu.PC, 0x20A);
    }

    #[test]
    fn test_minus_register_xy() {
        let rom: [u8; 10] = [
            0x60, 0x8A, // 0x200: Set V0 = 0x8A
            0x61, 0x22, // 0x202: Set V1 = 0x22
            0x80, 0x15, // 0x204: Set V0 = V0 - V1 (normal) - 68
            0x61, 0xE0, // 0x206: Set V1 = 0xE0
            0x80, 0x15, // 0x208: Set V0 = V0 - V1 (overflow) - 88
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0x68);
        assert_eq!(emu.V[0x1], 0x22);
        assert_eq!(emu.V[0xF], 0x01);

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0x88);
        assert_eq!(emu.V[0x1], 0xE0);
        assert_eq!(emu.V[0xF], 0x00);
        assert_eq!(emu.PC, 0x20A);
    }

    #[test]
    fn test_minus_register_yx() {
        let rom: [u8; 10] = [
            0x60, 0x8A, // 0x200: Set V0 = 0x8A
            0x61, 0x22, // 0x202: Set V1 = 0x22
            0x80, 0x17, // 0x204: Set V0 = V1 - V0 (overflow) - 98
            0x61, 0xE0, // 0x206: Set V1 = 0xE0
            0x80, 0x17, // 0x208: Set V0 = V1 - V0 (normal) - 48
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0x98);
        assert_eq!(emu.V[0x1], 0x22);
        assert_eq!(emu.V[0xF], 0x00);

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0x48);
        assert_eq!(emu.V[0x1], 0xE0);
        assert_eq!(emu.V[0xF], 0x01);
        assert_eq!(emu.PC, 0x20A);
    }

    #[test]
    fn test_store_into_addr_register() {
        let rom = [0xA1u8, 0x23];
        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        assert_eq!(emu.I, 0x0);

        emu.execute().unwrap();
        assert_eq!(emu.I, 0x123);
        assert_eq!(emu.PC, 0x202);
    }

    #[test]
    fn test_jump_addr_v0() {
        let rom: [u8; 12] = [
            0x60, 0x02, // 0x200: SET V0 = 0x02
            0xB2, 0x04, // 0x202: JP 0x204 + 0x02 = 0x206
            0x00, 0x00, // 0x204: filler
            0x61, 0x01, // 0x206: SET V1 = 0x01
            0x60, 0xFF, // 0x208: SET V0 = 0xFF
            0xBF, 0xFF, // 0x20A: jump outside of memory bounds (error)
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0x02);
        assert_eq!(emu.V[0x1], 0x01);
        assert_eq!(emu.PC, 0x208);

        emu.execute().unwrap();
        assert_eq!(emu.V[0x0], 0xFF);

        assert!(matches!(
            emu.execute(),
            Err(Error::InvalidJump(0xBF, 0xFF, 0x20A))
        ));
        assert_eq!(emu.PC, 0x20A);
    }

    #[test]
    fn test_store_register_into_dt() {
        let rom: [u8; 4] = [
            0x61, 0xAE, // 0x200: SET V1 = 0xAE
            0xF1, 0x15, // 0x202: SET DT = V1
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x1], 0xAE);
        assert_eq!(emu.DT, 0xAE);
        assert_eq!(emu.PC, 0x204);
    }

    #[test]
    fn test_store_dt_in_register() {
        let rom: [u8; 6] = [
            0x60, 0xAF, // 0x200: SET V0 = 0xAF
            0xF0, 0x15, // 0x202: SET DT = V0
            0xF1, 0x07, // 0x204: SET V1 = DT
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0xAF);
        assert_eq!(emu.V[0x1], 0xAF);
        assert_eq!(emu.DT, 0xAF);
        assert_eq!(emu.PC, 0x206);
    }

    #[test]
    fn test_store_register_into_st() {
        let rom: [u8; 4] = [
            0x61, 0xAA, // 0x200: SET V1 = 0xAA
            0xF1, 0x18, // 0x202: SET ST = V1
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x1], 0xAA);
        assert_eq!(emu.ST, 0xAA);
        assert_eq!(emu.PC, 0x204);
    }

    #[test]
    fn test_sum_register_addr() {
        let rom: [u8; 6] = [
            0x60, 0x11, // 0x200: SET V0 = 0x11
            0xF0, 0x1E, // 0x202: SET I = I + V0
            0xF0, 0x1E, // 0x204: SET I = I + V0
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0x11);
        assert_eq!(emu.I, 0x22);
        assert_eq!(emu.PC, 0x206);
    }

    #[test]
    fn test_shift_right() {
        let rom: [u8; 8] = [
            0x60, 0xF0, // 0x200: SET V0 = 0xF0
            0x81, 0x06, // 0x202: SET V1 = V0 >> 1 (0x78, VF=0)
            0x62, 0x0F, // 0x204: SET V2 = 0x0F
            0x83, 0x26, // 0x206: SET V3 = V3 >> 1 (0x07, VF=1)
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0xF0);
        assert_eq!(emu.V[0x1], 0x78);
        assert_eq!(emu.V[0xF], 0x0);

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x2], 0x0F);
        assert_eq!(emu.V[0x3], 0x07);
        assert_eq!(emu.V[0xF], 0x1);
        assert_eq!(emu.PC, 0x208);
    }

    #[test]
    fn test_shift_left() {
        let rom: [u8; 8] = [
            0x60, 0xF0, // 0x200: SET V0 = 0xF0
            0x81, 0x0E, // 0x202: SET V1 = V0 << 1 (0xE0, VF=1)
            0x62, 0x0F, // 0x204: SET V2 = 0x0F
            0x83, 0x2E, // 0x206: SET V3 = V3 << 1 (0x1E, VF=0)
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0xF0);
        assert_eq!(emu.V[0x1], 0xE0);
        assert_eq!(emu.V[0xF], 0x1);

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x2], 0x0F);
        assert_eq!(emu.V[0x3], 0x1E);
        assert_eq!(emu.V[0xF], 0x0);
        assert_eq!(emu.PC, 0x208);
    }

    #[test]
    fn test_store_bcd() {
        let rom: [u8; 10] = [
            0xA2, 0x34, // 0x200: SET I = 0x234
            0x60, 0x9A, // 0x202: SET V0 = 0x9A (154 decimal)
            0xF0, 0x33, // 0x204: Convert V0 to BCD
            0x61, 0x32, // 0x206: SET V1 = 0x32 (50 decimal)
            0xF1, 0x33, // 0x208: Convert V1 to BCD
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.I, 0x234);
        assert_eq!(emu.V[0x0], 0x9A);
        assert_eq!(emu.memory[emu.I as usize], 0x01);
        assert_eq!(emu.memory[(emu.I + 1) as usize], 0x05);
        assert_eq!(emu.memory[(emu.I + 2) as usize], 0x04);

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.I, 0x234);
        assert_eq!(emu.V[0x1], 0x32);
        assert_eq!(emu.memory[emu.I as usize], 0x00);
        assert_eq!(emu.memory[(emu.I + 1) as usize], 0x05);
        assert_eq!(emu.memory[(emu.I + 2) as usize], 0x00);
    }
}
