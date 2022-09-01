use std::time::Instant;

use anyhow::Context;
use clap::Parser;
use sdl2::{event::Event, pixels::Color, rect::Rect};
use thiserror::Error;

mod emulator;
mod keymap;

use keymap::{Action, Keymap};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// ROM file to load
    #[clap(value_parser)]
    filename: String,
}

#[derive(Error, Debug)]
enum AppError {
    #[error("SDL error: {0}")]
    Sdl(String),
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Sdl(s)
    }
}

fn main() -> Result<(), anyhow::Error> {
    const CYCLE_DELAY: u128 = 1_000_000 / 540;
    const TIMER_DELAY: u128 = 1_000_000 / 60;
    const ZOOM: usize = 10;

    // parse command-line arguments
    let cli = Cli::parse();

    // lod the rom and build the emulator
    let rom = std::fs::File::open(&cli.filename)
        .with_context(|| format!("error opening rom file: {}", &cli.filename))?;

    let mut emu = emulator::Emulator::load_rom(rom).context("error loading rom")?;

    // initialize SDL context and subsystems
    let sdl_context = sdl2::init()
        .map_err(AppError::from)
        .context("failed to initialize SDL context")?;
    let sdl_video = sdl_context
        .video()
        .map_err(AppError::from)
        .context("failed to initialize video subsystem")?;

    // build the window
    let window = sdl_video
        .window(
            "RC8",
            (emulator::DISPLAY_WIDTH * ZOOM) as u32,
            (emulator::DISPLAY_HEIGHT * ZOOM) as u32,
        )
        .position_centered()
        .build()
        .context("error creating window")?;

    // get the drawing canvas
    let mut canvas = window
        .into_canvas()
        .build()
        .context("error creating window canvas")?;

    // get the event pump
    let mut event_pump = sdl_context
        .event_pump()
        .map_err(AppError::from)
        .context("error obtaining the event pump")?;

    let mut running = true;
    let mut previous = Instant::now();
    let mut timer_delta = 0;
    let mut cpu_delta = 0;
    let keymap = Keymap::Chip8;

    while running {
        let now = Instant::now();
        let elapsed = previous.elapsed().as_micros();
        timer_delta += elapsed;
        cpu_delta += elapsed;
        previous = now;

        // process input
        for event in event_pump.poll_iter() {
            match keymap.translate_action(&event) {
                Some(Action::EmulateKeyState(key, state)) => emu.set_key(key, state),
                Some(Action::Quit) => running = false,
                None => {
                    if let Event::Quit { .. } = event {
                        running = false;
                    }
                }
            }
        }

        // run cpu
        while cpu_delta >= CYCLE_DELAY {
            emu.execute()?;
            cpu_delta -= CYCLE_DELAY;
        }

        // update timers
        while timer_delta >= TIMER_DELAY {
            emu.decrease_timers();
            timer_delta -= TIMER_DELAY;
        }

        // draw a frame
        canvas.set_draw_color(Color::RGB(0x00, 0x00, 0x00));
        canvas.clear();

        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        for x in 0..emulator::DISPLAY_WIDTH {
            for y in 0..emulator::DISPLAY_HEIGHT {
                if emu.get_pixel(x, y) {
                    let rect = Rect::new(
                        (x * ZOOM) as i32,
                        (y * ZOOM) as i32,
                        ZOOM as u32,
                        ZOOM as u32,
                    );
                    canvas
                        .fill_rect(rect)
                        .map_err(AppError::from)
                        .context("error drawing to canvas")?;
                }
            }
        }
        canvas.present();
    }

    Ok(())
}
