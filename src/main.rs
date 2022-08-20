use std::time::Instant;

use sdl2::{
    event::Event,
    keyboard::Keycode,
    pixels::Color,
    rect::{Point, Rect},
};

mod emulator;

fn main() {
    const CYCLE_DELAY: u128 = 1_000_000 / 540;
    const TIMER_DELAY: u128 = 1_000_000 / 60;

    let sdl_context = sdl2::init().unwrap();
    let sdl_video = sdl_context.video().unwrap();
    let window = sdl_video
        .window("RC8", 640, 320)
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

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
}
