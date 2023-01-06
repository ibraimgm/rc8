use std::{cmp::Ordering, io::Read};

use nanorand::{BufferedRng, Rng, WyRand};
use thiserror::Error;

pub const DISPLAY_WIDTH: usize = 64;
pub const DISPLAY_HEIGHT: usize = 32;

// memory size
const MEM_SIZE: usize = 4096;

// start of the sprite data
const SPRITE_DATA_START: usize = 0;

// built-in sprites
const SPRITE_DATA: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

// minimum subroutine stack size (to preallocate)
const MIN_SUB_STACK_SIZE: usize = 12;

// start and end of the free are for user programs
// end address is inclusive
const ADDR_START: usize = 0x200;
const ADDR_END: usize = 0xE8F;

// rom size
const MAX_ROM_SIZE: usize = ADDR_END - ADDR_START + 1;

#[derive(Error, Debug)]
pub enum EmulatorError {
    #[error("invalid return at address {0:#05X}")]
    InvalidReturn(u16),

    #[error("machine subroutine call at address {0:#05X}")]
    MachineSubroutine(u16),

    #[error("invalid jump at address {2:#05X}: {0:02X}{1:02X}")]
    InvalidJump(u8, u8, u16),

    #[error("invalid opcode at address {2:#05X}: {0:02X}{1:02X}")]
    InvalidOpcode(u8, u8, u16),

    #[error("could not load rom")]
    Io(#[from] std::io::Error),
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
pub struct Emulator {
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

    // which keys are pressed
    keys: [bool; 16],

    // random number generator
    rng: BufferedRng<WyRand, 8>,

    // screen - 64x32
    screen: [u64; 32],
    prev_screen: [u64; 32],

    // if a vblank interrupt happened
    // the draw command waits for this, to avoid
    // tearing on the sprites
    vblank_interrupt: bool,

    // last pressed key
    last_pressed_key: Option<u8>,
}

impl Emulator {
    /// Load a chip-8 rom, up to the maximum allowed rom size.
    pub fn load_rom<T>(rom: T) -> Result<Self, EmulatorError>
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
            keys: [false; 16],
            rng: BufferedRng::new(WyRand::new()),
            screen: [0u64; 32],
            prev_screen: [0u64; 32],
            vblank_interrupt: false,
            last_pressed_key: None,
        };

        // load the sprite data
        let sprite_area = &mut emu.memory[SPRITE_DATA_START..SPRITE_DATA_START + SPRITE_DATA.len()];
        sprite_area.copy_from_slice(&SPRITE_DATA[..]);

        // load the rom itself
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

    /// Set the state of a key (pressed/released).
    pub fn set_key(&mut self, key: usize, pressed: bool) {
        if self.keys[key & 0xF] && !pressed {
            self.last_pressed_key = Some(key as u8)
        }
        self.keys[key & 0xF] = pressed;
    }

    // registers that a vblank interrupt happened
    pub fn vblank(&mut self) {
        self.vblank_interrupt = true;
    }

    /// Decrease DT and ST, when the value is geater than 0.
    pub fn decrease_timers(&mut self) {
        self.DT = self.DT.checked_sub(1).unwrap_or(self.DT);
        self.ST = self.ST.checked_sub(1).unwrap_or(self.ST);
    }

    /// Returns wether the pixel at location (x, y) is set
    pub fn get_pixel(&self, x: usize, y: usize) -> bool {
        let x = x % DISPLAY_WIDTH;
        let y = y % DISPLAY_HEIGHT;

        let mask = 1 << (DISPLAY_WIDTH - x - 1);
        (self.screen[y] & mask) > 0
    }

    /// Returns true if the pixels on the screen were changed since the
    /// last call of this  method
    pub fn screen_changed(&mut self) -> bool {
        let changed = self.screen != self.prev_screen;
        self.prev_screen = self.screen;
        changed
    }

    /// Execute a single chip-8 CPU instruction.
    pub fn execute(&mut self) -> Result<(), EmulatorError> {
        // read a command
        let a = self.memory[self.PC];
        let b = self.memory[self.PC + 1];
        self.PC += 2;

        // choose the instruction to run
        match nibble_h(a) {
            // 00E0	- Clear the screen
            0x0 if a == 0x00 && b == 0xE0 => {
                self.screen.fill(0);
            }
            // 00EE	- Return from a subroutine
            0x0 if a == 0x00 && b == 0xEE => {
                if self.sub_stack.is_empty() {
                    return Err(EmulatorError::InvalidReturn((self.PC - 2) as u16));
                }

                self.PC = self.sub_stack.pop().unwrap();
            }
            // 0NNN - Execute machine instruction
            // it is ignored on emulators, here we return an error
            // just to track it
            0x0 => {
                return Err(EmulatorError::MachineSubroutine(self.PC as u16));
            }
            // 1NNN - jump to address NNN
            0x1 => {
                self.PC = nnn(a, b) as usize;
            }
            // 2NNN	- Execute subroutine starting at address NNN
            0x2 => {
                self.sub_stack.push(self.PC);
                self.PC = nnn(a, b) as usize;
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
                self.V[0xF] = 0;
            }
            // 8XY2 - Set VX = VX & VY
            0x8 if nibble_l(b) == 0x2 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                self.V[x] &= self.V[y];
                self.V[0xF] = 0;
            }
            // 8XY3 - Set VX = VX ^ VY
            0x8 if nibble_l(b) == 0x3 => {
                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                self.V[x] ^= self.V[y];
                self.V[0xF] = 0;
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
                let flag = self.V[y] & 1;
                self.V[x] = self.V[y] >> 1;
                self.V[0xF] = flag;
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
                let flag = self.V[y] >> 7;
                self.V[x] = self.V[y] << 1;
                self.V[0xF] = flag;
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
            // 0xBNNN - Jump to address NNN + V0
            0xB => {
                let addr = ((self.V[0x0] as u16) + nnn(a, b)) as usize;
                if addr >= MEM_SIZE {
                    self.PC -= 2;
                    return Err(EmulatorError::InvalidJump(a, b, self.PC as u16));
                }
                self.PC = addr;
            }
            // CXNN - Set VX to a random number with mask NN
            0xC => {
                let x = nibble_l(a) as usize;
                let mut n = [0u8; 1];
                self.rng.fill(&mut n);
                self.V[x] = n[0] & b;
            }
            // DXYN - Draw sprite at address I, on VX,VY and size N
            // set VF to 1 if any pixel is cleared
            0xD => {
                if !self.vblank_interrupt {
                    self.PC -= 2;
                    return Ok(());
                }
                self.vblank_interrupt = false;

                const LIMIT: usize = 64 - 8; // 64 bits minus 1 byte from the sprite

                let x = nibble_l(a) as usize;
                let y = nibble_h(b) as usize;
                let n = nibble_l(b) as usize;

                let x = (self.V[x] % 0x40) as usize;
                let y = (self.V[y] % 0x20) as usize;

                for offset in 0..n {
                    let row = y + offset;
                    if row >= self.screen.len() {
                        break;
                    }

                    let location = (self.I as usize) + offset;
                    let to_draw = self.memory[location] as u64;

                    let to_draw = match x.cmp(&LIMIT) {
                        Ordering::Greater => to_draw >> (x - LIMIT),
                        Ordering::Less => to_draw << (LIMIT - x),
                        Ordering::Equal => to_draw,
                    };

                    let result = self.screen[row] ^ to_draw;
                    if self.screen[row] != (self.screen[row] & result) {
                        self.V[0xF] = 0x01;
                    }
                    self.screen[row] = result
                }
            }
            // EX9E - Skip next if the key on VX value is pressed
            0xE if b == 0x9E => {
                let x = nibble_l(a) as usize;
                let key = (self.V[x] & 0xF) as usize;
                if self.keys[key] {
                    self.PC += 2;
                }
            }
            // EXA1 - Skip next if the key on VX value is NOT pressed
            0xE if b == 0xA1 => {
                let x = nibble_l(a) as usize;
                let key = (self.V[x] & 0xF) as usize;
                if !self.keys[key] {
                    self.PC += 2;
                }
            }
            // FX07 - Store the DT value into VX
            0xF if b == 0x07 => {
                let x = nibble_l(a) as usize;
                self.V[x] = self.DT;
            }
            // FX0A - Wait for a key press and store the digit on VX
            0xF if b == 0x0A => {
                let x = nibble_l(a) as usize;
                if let Some(key) = self.last_pressed_key {
                    self.V[x] = key
                } else {
                    self.PC -= 2
                }
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
            // FX29 - Set the address of the sprite of digit on VX to I
            0xF if b == 0x29 => {
                let x = nibble_l(a) as usize;
                let digit = self.V[x] & 0xF;
                self.I = (digit * 5) as u16;
            }
            // FX33 - Store BCD of VX into I, I+I and I+2
            0xF if b == 0x33 => {
                let x = nibble_l(a) as usize;
                let i = self.I as usize;
                self.memory[i] = self.V[x] / 100;
                self.memory[i + 1] = self.V[x] / 10 % 10;
                self.memory[i + 2] = self.V[x] % 100 % 10;
            }
            // FX55 - Store from V0 to VX, starting on I
            // at the end, I will point to the next byte
            0xF if b == 0x55 => {
                let start_addr = self.I as usize;
                let end = (nibble_l(a) + 1) as usize;
                let slice = &mut self.memory[start_addr..start_addr + end];
                slice.copy_from_slice(&self.V[0..end]);
                self.I += end as u16;
            }
            // FX65 - Load from I into V0 -> VX
            // at the end, I will point to the next byte
            0xF if b == 0x65 => {
                let start_addr = self.I as usize;
                let end = (nibble_l(a) + 1) as usize;
                let slice = &mut self.V[0..end];
                slice.copy_from_slice(&self.memory[start_addr..start_addr + end]);
                self.I += end as u16;
            }
            _ => return Err(EmulatorError::InvalidOpcode(a, b, (self.PC - 2) as u16)),
        }

        self.last_pressed_key = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec_cycles(emu: &mut Emulator, mut cycles: i32) {
        while cycles > 0 {
            emu.vblank();
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
        let rom: [u8; 8] = [
            0x6F, 0xFF, // 0x200: SET VF = 0xFF
            0x60, 0xBB, // 0x202: SET V0 = 0xBB
            0x61, 0x5A, // 0x204: SET V1 = 0x5A
            0x80, 0x11, // 0x206: SET V0 = V0 | V1
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 4);
        assert_eq!(emu.V[0x0], 0xFB);
        assert_eq!(emu.V[0xF], 0);
        assert_eq!(emu.PC, 0x208);
    }

    #[test]
    fn test_bitwise_and() {
        let rom: [u8; 8] = [
            0x6F, 0xFF, // 0x200: SET VF = 0xFF
            0x60, 0xBB, // 0x202: SET V0 = 0xBB
            0x61, 0x5A, // 0x204: SET V1 = 0x5A
            0x80, 0x12, // 0x206: SET V0 = V0 & V1
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 4);
        assert_eq!(emu.V[0x0], 0x1A);
        assert_eq!(emu.V[0xF], 0);
        assert_eq!(emu.PC, 0x208);
    }

    #[test]
    fn test_bitwise_xor() {
        let rom: [u8; 8] = [
            0x6F, 0xFF, // 0x200: SET VF = 0xFF
            0x60, 0xBB, // 0x202: SET V0 = 0xBB
            0x61, 0x5A, // 0x204: SET V1 = 0x5A
            0x80, 0x13, // 0x206: SET V0 = V0 ^ V1
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 4);
        assert_eq!(emu.V[0x0], 0xE1);
        assert_eq!(emu.V[0xF], 0);
        assert_eq!(emu.PC, 0x208);
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
            Err(EmulatorError::InvalidJump(0xBF, 0xFF, 0x20A))
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
    fn test_shift_right_order() {
        let rom: [u8; 10] = [
            0x6F, 0x07, // 0x200: SET VF = 7
            0x8F, 0xF6, // 0x202: SET VF = VF >> 1 (3, but overriden to 1)
            0x80, 0xF0, // 0x204: SET V0 = VF
            0x61, 0x03, // 0x206: SET V1 = 3
            0x81, 0x16, // 0x208: SET V1 = V1 >> 1 (1)
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        exec_cycles(&mut emu, 5);

        assert_eq!(emu.V[0x0], 0x1);
        assert_eq!(emu.V[0x1], 0x1);
        assert_eq!(emu.V[0xF], 0x1);
        assert_eq!(emu.PC, 0x20A);
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
    fn test_shift_left_order() {
        let rom: [u8; 10] = [
            0x6F, 0xC8, // 0x200: SET VF = 0xC8 (200)
            0x8F, 0xFE, // 0x202: SET VF = VF << 1 (normally 400, but overrides to 1)
            0x80, 0xF0, // 0x204: SET V0 = VF (1)
            0x61, 0xC8, // 0x206: SET V1 = 0xC8 (200)
            0x81, 0x1E, // 0x208: SET V1 = V1 << 1 (400 - 0x90)
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        exec_cycles(&mut emu, 5);

        assert_eq!(emu.V[0x0], 0x01);
        assert_eq!(emu.V[0x1], 0x90);
        assert_eq!(emu.V[0xF], 0x01);
        assert_eq!(emu.PC, 0x20A);
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

    #[test]
    fn test_load_sprite_address() {
        let rom: [u8; 16] = [
            0x60, 0x00, // 0x200: SET V0 = 0
            0xF0, 0x29, // 0x202: SET I = sprite address of 0
            0x60, 0x05, // 0x204: SET V0 = 5
            0xF0, 0x29, // 0x206: SET I = sprite address of 5
            0x60, 0x0F, // 0x208: SET V0 = F
            0xF0, 0x29, // 0x20A: SET I = sprite address of F
            0x60, 0x1E, // 0x20C: SET V0 = 1E
            0xF0, 0x29, // 0x20E: SET I = sprite address of E
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0x0);
        assert_eq!(emu.I, 0x000);
        assert_eq!(emu.memory[emu.I as usize], 0xF0);
        assert_eq!(emu.memory[(emu.I + 1) as usize], 0x90);
        assert_eq!(emu.memory[(emu.I + 2) as usize], 0x90);
        assert_eq!(emu.memory[(emu.I + 3) as usize], 0x90);
        assert_eq!(emu.memory[(emu.I + 4) as usize], 0xF0);

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0x5);
        assert_eq!(emu.I, 0x019);
        assert_eq!(emu.memory[emu.I as usize], 0xF0);
        assert_eq!(emu.memory[(emu.I + 1) as usize], 0x80);
        assert_eq!(emu.memory[(emu.I + 2) as usize], 0xF0);
        assert_eq!(emu.memory[(emu.I + 3) as usize], 0x10);
        assert_eq!(emu.memory[(emu.I + 4) as usize], 0xF0);

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0xF);
        assert_eq!(emu.I, 0x04B);
        assert_eq!(emu.memory[emu.I as usize], 0xF0);
        assert_eq!(emu.memory[(emu.I + 1) as usize], 0x80);
        assert_eq!(emu.memory[(emu.I + 2) as usize], 0xF0);
        assert_eq!(emu.memory[(emu.I + 3) as usize], 0x80);
        assert_eq!(emu.memory[(emu.I + 4) as usize], 0x80);

        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0x1E);
        assert_eq!(emu.I, 0x046);
        assert_eq!(emu.memory[emu.I as usize], 0xF0);
        assert_eq!(emu.memory[(emu.I + 1) as usize], 0x80);
        assert_eq!(emu.memory[(emu.I + 2) as usize], 0xF0);
        assert_eq!(emu.memory[(emu.I + 3) as usize], 0x80);
        assert_eq!(emu.memory[(emu.I + 4) as usize], 0xF0);
        assert_eq!(emu.PC, 0x210);
    }

    #[test]
    fn test_skip_key_pressed() {
        let rom: [u8; 18] = [
            0x60, 0x0E, // 0x200: Set V0 = 0x0E
            0xE0, 0x9E, // 0x202: Skip if key on V0 is pressed ("E")
            0x61, 0x01, // 0x204: Set V1 = 0x01 (skipped)
            0x60, 0xEE, // 0x206: Set V0 = 0xEE
            0xE0, 0x9E, // 0x208: Skip if key on V0 is pressed ("E")
            0x62, 0x01, // 0x20A: Set V2 = 0x01 (skipped)
            0x60, 0xFF, // 0x20C: Set V0 = 0xFF
            0xE0, 0x9E, // 0x20E: Skip if key on V0 is pressed ("F")
            0x63, 0x01, // 0x210: Set V3 = 0x01
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        emu.set_key(0xE, true);

        exec_cycles(&mut emu, 7);
        assert_eq!(emu.V[0x0], 0xFF);
        assert_eq!(emu.V[0x1], 0x00);
        assert_eq!(emu.V[0x2], 0x00);
        assert_eq!(emu.V[0x3], 0x01);
        assert_eq!(emu.PC, 0x212);
    }

    #[test]
    fn test_skip_key_not_pressed() {
        let rom: [u8; 20] = [
            0x60, 0x0E, // 0x200: Set V0 = 0x0E
            0xE0, 0xA1, // 0x202: Skip if key on V0 is not pressed ("E")
            0x61, 0x01, // 0x204: Set V1 = 0x01
            0x60, 0xEE, // 0x206: Set V0 = 0xEE
            0xE0, 0xA1, // 0x208: Skip if key on V0 is not pressed ("E")
            0x62, 0x01, // 0x20A: Set V2 = 0x01
            0x60, 0xFF, // 0x20C: Set V0 = 0xFF
            0xE0, 0xA1, // 0x20E: Skip if key on V0 is not pressed ("F")
            0x63, 0x01, // 0x210: Set V3 = 0x01 (skipped)
            0x64, 0x01, // 0x212: Set V4 = 0x01
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        emu.set_key(0xE, true);

        exec_cycles(&mut emu, 9);
        assert_eq!(emu.V[0x0], 0xFF);
        assert_eq!(emu.V[0x1], 0x01);
        assert_eq!(emu.V[0x2], 0x01);
        assert_eq!(emu.V[0x3], 0x00);
        assert_eq!(emu.V[0x4], 0x01);
        assert_eq!(emu.PC, 0x214);
    }

    #[test]
    fn test_wait_for_key_press() {
        let rom: [u8; 4] = [
            0xF0, 0x0A, // 0x200: Set V0 = <pressed key> (wait)
            0x61, 0x01, // 0x202: Set V1 = 0x01
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        // should get stuck, waiting for key
        exec_cycles(&mut emu, 10);
        assert_eq!(emu.PC, 0x200);

        // key is down, but it needs to be released
        // to register the keypress
        emu.set_key(0xA, true);
        exec_cycles(&mut emu, 10);
        assert_eq!(emu.PC, 0x200);

        // release the key, now it should work
        emu.set_key(0xA, false);
        exec_cycles(&mut emu, 2);
        assert_eq!(emu.V[0x0], 0xA);
        assert_eq!(emu.V[0x1], 0x1);
        assert_eq!(emu.PC, 0x204);
    }

    #[test]
    fn test_bulk_save() {
        let rom: [u8; 16] = [
            0x60, 0x01, // 0x200: Set V0 = 0x01
            0x61, 0x02, // 0x202: Set V1 = 0x02
            0x62, 0x03, // 0x204: Set V2 = 0x03
            0x63, 0x04, // 0x206: Set V3 = 0x04
            0x64, 0x05, // 0x208: Set V4 = 0x05
            0x65, 0x06, // 0x20A: Set V5 = 0x06
            0xA2, 0x22, // 0x20C: Set I = 0x222
            0xF5, 0x55, // 0x20E: Store V0->V5 starting at I
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 8);
        assert_eq!(emu.memory[0x222], 0x01);
        assert_eq!(emu.memory[0x223], 0x02);
        assert_eq!(emu.memory[0x224], 0x03);
        assert_eq!(emu.memory[0x225], 0x04);
        assert_eq!(emu.memory[0x226], 0x05);
        assert_eq!(emu.memory[0x227], 0x06);
        assert_eq!(emu.I, 0x228);
        assert_eq!(emu.PC, 0x210);
    }

    #[test]
    fn test_bulk_load() {
        let rom: [u8; 22] = [
            0xA2, 0x04, // 0x200: Set I = 0x204
            0x12, 0x14, // 0x202: JMP 0x214
            0x01, 0x02, // 0x204: DATA
            0x03, 0x04, // 0x206: DATA
            0x05, 0x06, // 0x208: DATA
            0x07, 0x08, // 0x20A: DATA
            0x09, 0x0A, // 0x20C: DATA
            0x0B, 0x0C, // 0x20E: DATA
            0x0D, 0x0E, // 0x210: DATA
            0x0F, 0x10, // 0x212: DATA
            0xFF, 0x65, // 0x214: Load V0 -> VF starting at I
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0x01);
        assert_eq!(emu.V[0x1], 0x02);
        assert_eq!(emu.V[0x2], 0x03);
        assert_eq!(emu.V[0x3], 0x04);
        assert_eq!(emu.V[0x4], 0x05);
        assert_eq!(emu.V[0x5], 0x06);
        assert_eq!(emu.V[0x6], 0x07);
        assert_eq!(emu.V[0x7], 0x08);
        assert_eq!(emu.V[0x8], 0x09);
        assert_eq!(emu.V[0x9], 0x0A);
        assert_eq!(emu.V[0xA], 0x0B);
        assert_eq!(emu.V[0xB], 0x0C);
        assert_eq!(emu.V[0xC], 0x0D);
        assert_eq!(emu.V[0xD], 0x0E);
        assert_eq!(emu.V[0xE], 0x0F);
        assert_eq!(emu.V[0xF], 0x10);
        assert_eq!(emu.I, 0x214);
        assert_eq!(emu.PC, 0x216);
    }

    #[test]
    fn test_random() {
        let rom: [u8; 6] = [
            0xC0, 0x0F, // 0x200: Set V0 = <random> & 0x0F = 8E & 0F = 0E
            0xC1, 0xF0, // 0x202: Set V1 = <random> & 0xF0 = A5 & F0 = A0
            0xC2, 0x3C, // 0x204: Set V2 = <random> & 0x3C = 59 & 3C = 18
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        emu.rng = BufferedRng::new(WyRand::new_seed(0));

        exec_cycles(&mut emu, 3);
        assert_eq!(emu.V[0x0], 0x0E);
        assert_eq!(emu.V[0x1], 0xA0);
        assert_eq!(emu.V[0x2], 0x18);
        assert_eq!(emu.PC, 0x206);
    }

    #[test]
    fn test_draw() {
        let rom: [u8; 40] = [
            0x60, 0x04, // 0x200: Set V0 = 4
            0x61, 0x00, // 0x202: Set V1 = 0
            0x62, 0x0A, // 0x204: Set V2 = 0xA
            0xF2, 0x29, // 0x206: Set I to V2 ("A")
            0xD0, 0x15, // 0x208: Draw[VX, VY] = "A"
            //
            0x60, 0x09, // 0x20A: Set V0 = 9
            0x61, 0x01, // 0x20C: Set V1 = 1
            0x62, 0x0B, // 0x20E: Set V2 = 0xB
            0xF2, 0x29, // 0x210: Set I to V2 ("B")
            0xD0, 0x15, // 0x212: Draw[VX, VY] = "B"
            //
            0x60, 0x3C, // 0x214: Set V0 = 60
            0x61, 0x0A, // 0x216: Set V1 = 10
            0x62, 0x09, // 0x218: Set V2 = 0x9
            0xF2, 0x29, // 0x21A: Set I to V2 ("9")
            0xD0, 0x15, // 0x21C: Draw[VX, VY] = "9"
            //
            0x60, 0x3E, // 0x21E: Set V0 = 62
            0x61, 0x1D, // 0x220: Set V1 = 29
            0x62, 0x0E, // 0x222: Set V2 = 0xE
            0xF2, 0x29, // 0x224: Set I to V2 ("E")
            0xD0, 0x15, // 0x226: Draw[VX, VY] = "E"
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 20);
        assert_eq!(emu.V[0x0], 0x3E);
        assert_eq!(emu.V[0x1], 0x1D);
        assert_eq!(emu.V[0x2], 0x0E);
        assert_eq!(emu.V[0xF], 0x00);
        assert_eq!(emu.PC, 0x228);

        assert_eq!(emu.screen[0], 0xF00000000000000);
        assert_eq!(emu.screen[1], 0x970000000000000);
        assert_eq!(emu.screen[2], 0xF48000000000000);
        assert_eq!(emu.screen[3], 0x970000000000000);
        assert_eq!(emu.screen[4], 0x948000000000000);
        assert_eq!(emu.screen[5], 0x070000000000000);
        assert_eq!(emu.screen[6], 0x000000000000000);
        assert_eq!(emu.screen[7], 0x000000000000000);
        assert_eq!(emu.screen[8], 0x000000000000000);
        assert_eq!(emu.screen[9], 0x000000000000000);
        assert_eq!(emu.screen[10], 0x00000000000000F);
        assert_eq!(emu.screen[11], 0x000000000000009);
        assert_eq!(emu.screen[12], 0x00000000000000F);
        assert_eq!(emu.screen[13], 0x000000000000001);
        assert_eq!(emu.screen[14], 0x00000000000000F);
        assert_eq!(emu.screen[15], 0x000000000000000);
        assert_eq!(emu.screen[16], 0x000000000000000);
        assert_eq!(emu.screen[17], 0x000000000000000);
        assert_eq!(emu.screen[18], 0x000000000000000);
        assert_eq!(emu.screen[19], 0x000000000000000);
        assert_eq!(emu.screen[20], 0x000000000000000);
        assert_eq!(emu.screen[21], 0x000000000000000);
        assert_eq!(emu.screen[22], 0x000000000000000);
        assert_eq!(emu.screen[23], 0x000000000000000);
        assert_eq!(emu.screen[24], 0x000000000000000);
        assert_eq!(emu.screen[25], 0x000000000000000);
        assert_eq!(emu.screen[26], 0x000000000000000);
        assert_eq!(emu.screen[27], 0x000000000000000);
        assert_eq!(emu.screen[28], 0x000000000000000);
        assert_eq!(emu.screen[29], 0x000000000000003);
        assert_eq!(emu.screen[30], 0x000000000000002);
        assert_eq!(emu.screen[31], 0x000000000000003);
    }

    #[test]
    fn test_draw_xor() {
        let rom: [u8; 16] = [
            0x60, 0x0C, // 0x200: Set V0 = 12
            0x61, 0x00, // 0x202: Set V1 = 0
            0x62, 0x09, // 0x204: Set V2 = 0x9
            0xF2, 0x29, // 0x206: Set I to V2 ("9")
            0xD0, 0x15, // 0x208: Draw[V0, V1] = "9"
            //
            0x62, 0x08, // 0x20A: Set V2 = 0x8
            0xF2, 0x29, // 0x20C: Set I to V2 ("8")
            0xD0, 0x15, // 0x20E: Draw[V0, V1] = "8"
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 8);
        assert_eq!(emu.V[0x0], 0x0C);
        assert_eq!(emu.V[0x1], 0x00);
        assert_eq!(emu.V[0x2], 0x08);
        assert_eq!(emu.V[0xF], 0x01);
        assert_eq!(emu.PC, 0x210);

        for (row, value) in emu.screen.iter().enumerate() {
            if row == 3 {
                assert_eq!(*value, 0x8000000000000)
            } else {
                assert_eq!(*value, 0x0)
            }
        }
    }

    #[test]
    fn test_clear_screen() {
        let rom: [u8; 12] = [
            0x60, 0x0C, // 0x200: Set V0 = 12
            0x61, 0x00, // 0x202: Set V1 = 0
            0x62, 0x09, // 0x204: Set V2 = 0x9
            0xF2, 0x29, // 0x206: Set I to V2 ("9")
            0xD0, 0x15, // 0x208: Draw[V0, V1] = "9"
            0x00, 0xE0, // 0x20A: Clear Screen
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 6);
        assert_eq!(emu.V[0x0], 0x0C);
        assert_eq!(emu.V[0x1], 0x00);
        assert_eq!(emu.V[0x2], 0x09);
        assert_eq!(emu.V[0xF], 0x00);
        assert_eq!(emu.PC, 0x20C);

        for value in emu.screen.iter() {
            assert_eq!(*value, 0x0)
        }
    }

    #[test]
    fn test_subroutine() {
        let rom: [u8; 18] = [
            0x12, 0x0A, // 0x200: Jump to 0x20A
            0x70, 0x01, // 0x202: Set V0 = V0 + 1
            0x71, 0x02, // 0x204: Set V1 = V1 + 2
            0x72, 0x03, // 0x206: Set V2 = V2 + 3
            0x00, 0xEE, // 0x208: RETURN
            0x22, 0x02, // 0x20A: CALL 0x202
            0x30, 0x03, // 0x20C: Skip next if VX == 3
            0x12, 0x0A, // 0x20E: Jump to 0x20A
            0x63, 0x01, // 0x210: Set V3 = 1
        ];

        let mut emu = Emulator::load_rom(&rom[..]).unwrap();

        exec_cycles(&mut emu, 22);
        assert_eq!(emu.V[0x0], 0x03);
        assert_eq!(emu.V[0x1], 0x06);
        assert_eq!(emu.V[0x2], 0x09);
        assert_eq!(emu.V[0x3], 0x01);
        assert_eq!(emu.PC, 0x212);
    }

    #[test]
    fn test_bad_return() {
        let rom = [0x00u8, 0xEE];
        let mut emu = Emulator::load_rom(&rom[..]).unwrap();
        assert!(matches!(
            emu.execute(),
            Err(EmulatorError::InvalidReturn(0x200))
        ));
    }
}
