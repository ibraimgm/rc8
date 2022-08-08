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
            0x1 => {
                todo!("jump to address")
            }
            0x2 => {
                todo!("execute subroutine")
            }
            0x3 => {
                todo!("Skip if VX == NN")
            }
            0x4 => {
                todo!("Skip id VX != NN")
            }
            0x5 if nibble_l(b) == 0x0 => {
                todo!("Skip if VX == VY")
            }
            0x6 => {
                todo!("set VX = NN")
            }
            0x7 => {
                todo!("set VX = VX + NN")
            }
            0x8 if nibble_l(b) == 0x0 => {
                todo!("set VX = VY")
            }
            0x8 if nibble_l(b) == 0x1 => {
                todo!("set VX = VX | VY")
            }
            0x8 if nibble_l(b) == 0x2 => {
                todo!("set VX = VX & VY")
            }
            0x8 if nibble_l(b) == 0x3 => {
                todo!("set VX = VX ^ VY")
            }
            0x8 if nibble_l(b) == 0x4 => {
                todo!("set VX = VX + VY; set or clear carry")
            }
            0x8 if nibble_l(b) == 0x5 => {
                todo!("set VX = VX - VY; set or clear borrow")
            }
            0x8 if nibble_l(b) == 0x6 => {
                todo!("set VX = VY >> 1; set VF to shifted bit")
            }
            0x8 if nibble_l(b) == 0x7 => {
                todo!("set VX = VY - VX; set or clear borrow")
            }
            0x8 if nibble_l(b) == 0xE => {
                todo!("set VX = VY << 1; set VF to shifted bit")
            }
            0x9 if nibble_l(b) == 0x0 => {
                todo!("skip if VX != VY")
            }
            0xA => {
                todo!("store NNN into I")
            }
            0xB => {
                todo!("jump to NNN + V0")
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
            0xF if b == 0x07 => {
                todo!("set VX = DT")
            }
            0xF if b == 0x0A => {
                todo!("wait for key and store in VX")
            }
            0xF if b == 0x15 => {
                todo!("set DT = VX")
            }
            0xF if b == 0x18 => {
                todo!("set ST = VX")
            }
            0xF if b == 0x1E => {
                todo!("sey I = I + VX")
            }
            0xF if b == 0x29 => {
                todo!("store on I the address of sprite of digit on VX")
            }
            0xF if b == 0x33 => {
                todo!("store BCD of VX in I, I+1 and I+2")
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
}
