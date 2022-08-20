use std::time::Instant;

use anyhow::Context;
use sdl2::{
    event::Event,
    keyboard::Keycode,
    pixels::Color,
    rect::{Point, Rect},
};
use thiserror::Error;

mod emulator;

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
        .window("RC8", 640, 320)
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

    while running {
        let now = Instant::now();
        let elapsed = previous.elapsed().as_micros();
        timer_delta += elapsed;
        cpu_delta += elapsed;
        previous = now;

        // process input
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => running = false,
                _ => (),
            }
        }

        // run cpu
        while cpu_delta >= CYCLE_DELAY {
            println!("cpu!");
            cpu_delta -= CYCLE_DELAY;
        }

        // update timers
        while timer_delta >= TIMER_DELAY {
            println!("timer!");
            timer_delta -= TIMER_DELAY;
        }

        // draw a frame
        canvas.set_draw_color(Color::RGB(0x00, 0x00, 0x00));
        canvas.clear();

        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        canvas
            .fill_rect(Rect::from_center(Point::new(320, 160), 10, 10))
            .unwrap();
        canvas.present();
    }

    Ok(())
}
