use std::time::Duration;

use sdl2::{
    event::Event,
    keyboard::Keycode,
    pixels::Color,
    rect::{Point, Rect},
};

mod emulator;

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let ttf_context = sdl2::ttf::init().unwrap();

    // TODO: need to find a proper font for distribution
    let font = ttf_context
        .load_font("source-code-pro.regular.ttf", 16)
        .unwrap();

    const TEXT: &str = "This is a test";
    let text_to_draw = font.render(TEXT).solid(Color::RGB(0xFF, 0, 0)).unwrap();
    let text_area = {
        let (width, height) = font.size_of(TEXT).unwrap();
        Rect::new(10, 10, width, height)
    };

    let sdl_video = sdl_context.video().unwrap();
    let window = sdl_video
        .window("RC8", 800, 600)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let rect = Rect::from_center(Point::new(400, 300), 100, 100);
    let mut color = Color::RGB(0, 0, 0);
    let mut increase = true;

    let texture_creator = canvas.texture_creator();
    let text_to_draw = texture_creator
        .create_texture_from_surface(text_to_draw)
        .unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    'sdl_loop: loop {
        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        canvas.clear();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'sdl_loop,
                _ => (),
            }
        }

        if increase {
            if color.r < 200 {
                color.r += 5
            } else if color.g < 200 {
                color.g += 5
            } else if color.b < 200 {
                color.b += 5
            } else {
                increase = false;
            }
        } else if color.r > 0 {
            color.r -= 5;
        } else if color.g > 0 {
            color.g -= 5;
        } else if color.b > 0 {
            color.b -= 5;
        } else {
            increase = true;
        }

        canvas.copy(&text_to_draw, None, text_area).unwrap();

        canvas.set_draw_color(color);
        canvas.fill_rect(rect).unwrap();
        canvas.present();

        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    println!("OK!");
}
