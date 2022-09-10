use std::time::Instant;

use anyhow::Context;
use sdl2::{audio::AudioSpecDesired, event::Event, pixels::Color, rect::Rect};
use thiserror::Error;

use super::{
    beep::Beep,
    emulator::{Emulator, DISPLAY_HEIGHT, DISPLAY_WIDTH},
    keymap::{Action, Keymap},
};

const CYCLE_DELAY: u128 = 1_000_000 / 540;
const TIMER_DELAY: u128 = 1_000_000 / 60;
const VBLANK_DELAY: u128 = 1_000_000 / 60;
const ZOOM: usize = 10;

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

enum AppState {
    Running,
    Quit,
}

/// Main application loop
pub fn run(mut emu: Emulator) -> Result<(), anyhow::Error> {
    // initialize SDL context and subsystems
    let sdl_context = sdl2::init()
        .map_err(AppError::from)
        .context("failed to initialize SDL context")?;
    let sdl_video = sdl_context
        .video()
        .map_err(AppError::from)
        .context("failed to initialize video subsystem")?;
    let sdl_audio = sdl_context
        .audio()
        .map_err(AppError::from)
        .context("failed to initialize audio subsystem")?;

    // build the window
    let window = sdl_video
        .window(
            "RC8",
            (DISPLAY_WIDTH * ZOOM) as u32,
            (DISPLAY_HEIGHT * ZOOM) as u32,
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

    // desired audio spec
    let desired_spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1),
        samples: None,
    };

    // get sound device
    let audio_device = sdl_audio
        .open_playback(None, &desired_spec, Beep::from)
        .map_err(AppError::from)
        .context("error opening audio device")?;

    let mut state = AppState::Running;
    let keymap = Keymap::Chip8;
    let mut previous = Instant::now();
    let mut timer_delta = 0;
    let mut cpu_delta = 0;
    let mut vblank_delta = 0;

    loop {
        let now = Instant::now();
        let elapsed = previous.elapsed().as_micros();
        previous = now;

        // process input events
        for event in event_pump.poll_iter() {
            match keymap.translate_action(&event) {
                Some(Action::EmulateKeyState(key, state)) => emu.set_key(key, state),
                Some(Action::Quit) => state = AppState::Quit,
                None => {
                    if let Event::Quit { .. } = event {
                        state = AppState::Quit
                    }
                }
            }
        }

        match state {
            // Only update the simulation when it is running
            AppState::Running => {
                timer_delta += elapsed;
                cpu_delta += elapsed;
                vblank_delta += elapsed;

                // vblank signal - just one trigger is enough
                if vblank_delta >= VBLANK_DELAY {
                    emu.vblank();
                    vblank_delta -= VBLANK_DELAY;
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

                // on COSMAC VIP, the sound is not played if ST is less than 2
                // this is a hardware quirk.
                if emu.ST > 1 {
                    audio_device.resume()
                } else {
                    audio_device.pause()
                }
            }
            // singnal to get out of the routine
            AppState::Quit => break,
        }

        // draw a frame
        // this will always happens, regardless of the simulation state
        canvas.set_draw_color(Color::RGB(0x00, 0x00, 0x00));
        canvas.clear();

        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        for x in 0..DISPLAY_WIDTH {
            for y in 0..DISPLAY_HEIGHT {
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

    audio_device.pause();
    Ok(())
}
