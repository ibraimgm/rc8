use std::time::Instant;

use anyhow::Context;
use sdl2::{
    audio::AudioSpecDesired,
    event::Event,
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    render::{BlendMode, Texture, TextureCreator, TextureValueError},
    surface::Surface,
    ttf::Font,
};
use thiserror::Error;

use super::{
    beep::Beep,
    emulator::{Emulator, DISPLAY_HEIGHT, DISPLAY_WIDTH},
    keymap::{Action, Keymap},
};

pub const PIXEL_SIZE: usize = 10;

const CYCLE_DELAY: u128 = 1_000_000 / 540;
const TIMER_DELAY: u128 = 1_000_000 / 60;
const VBLANK_DELAY: u128 = 1_000_000 / 60;

#[derive(Error, Debug)]
enum AppError {
    #[error("SDL error: {0}")]
    Sdl(String),

    #[error("SDL TTF error: {0}")]
    TTFInit(#[from] sdl2::ttf::InitError),

    #[error("SDL font error: {0}")]
    Font(#[from] sdl2::ttf::FontError),

    #[error("SDL texture error: {0}")]
    Texture(#[from] TextureValueError),
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Sdl(s)
    }
}

pub struct Options {
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub bgcolor: u32,
    pub fgcolor: u32,
}

#[derive(PartialEq)]
enum AppState {
    Running,
    Paused,
    Quit,
}

/// Main application loop
pub fn run(mut emu: Emulator, options: Options) -> Result<(), anyhow::Error> {
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

    // initialize SDL_ttf
    let ttf_context = sdl2::ttf::init()
        .map_err(AppError::from)
        .context("failed to initialize SDL_ttf context")?;

    // load TTF font
    let font_bytes = include_bytes!("computer-speak-v0.3.ttf");
    let font_rwops = sdl2::rwops::RWops::from_bytes(font_bytes).map_err(AppError::from)?;
    let font = ttf_context
        .load_font_from_rwops(font_rwops, 64)
        .map_err(AppError::from)?;

    // build the window
    let mut window = sdl_video.window("RC8", options.width, options.height);

    if options.fullscreen {
        window.fullscreen_desktop();
    } else {
        window.position_centered();
    }

    let window = window.build().context("error creating window")?;

    // get the drawing canvas
    let mut canvas = window
        .into_canvas()
        .build()
        .context("error creating window canvas")?;

    canvas
        .set_logical_size(
            (DISPLAY_WIDTH * PIXEL_SIZE) as u32,
            (DISPLAY_HEIGHT * PIXEL_SIZE) as u32,
        )
        .context("failed to set logical resolution")?;

    // build a texture creator
    let texture_creator = canvas.texture_creator();

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

    // convert color values
    let bgcolor = options.bgcolor.to_be_bytes();
    let bgcolor = Color::RGBA(bgcolor[0], bgcolor[1], bgcolor[2], 0xff);
    let fgcolor = options.fgcolor.to_be_bytes();
    let fgcolor = Color::RGBA(fgcolor[0], fgcolor[1], fgcolor[2], 0xff);

    let mut state = AppState::Running;
    let keymap = Keymap::Chip8;
    let mut previous = Instant::now();
    let mut timer_delta = 0;
    let mut cpu_delta = 0;
    let mut vblank_delta = 0;
    let mut emulator_texture = None;
    let mut pause_texture = None;

    loop {
        let now = Instant::now();
        let elapsed = previous.elapsed().as_micros();
        previous = now;

        // process input events
        for event in event_pump.poll_iter() {
            match keymap.translate_action(&event) {
                Some(Action::EmulateKeyState(key, state)) => emu.set_key(key, state),
                Some(Action::Quit) => state = AppState::Quit,
                Some(Action::TogglePause) => {
                    state = if state == AppState::Running {
                        AppState::Paused
                    } else {
                        AppState::Running
                    }
                }
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

            // do nothing if paused, except stopping the buzzer
            // it will be resumed in the running logic, if needed
            AppState::Paused => {
                audio_device.pause();
            }

            // signal to get out of the routine
            AppState::Quit => break,
        }

        // draw a frame - this will always happens, regardless of the simulation state
        // first, we cache the screen state
        if emu.screen_changed() || emulator_texture.is_none() {
            let texture = draw_emulator_screen(&emu, bgcolor, fgcolor, &texture_creator)
                .context("error computing emulator state")?;
            emulator_texture = Some(texture);
        }

        // then, we do the real drawing
        {
            let texture = emulator_texture.as_ref().unwrap();
            canvas
                .copy(texture, None, None)
                .map_err(AppError::from)
                .context("error drawing emulator screen")?;
        }

        // when paused, we add an extra overlay
        if state == AppState::Paused {
            if pause_texture.is_none() {
                let texture = draw_pause_screen(&font, &texture_creator)
                    .map_err(AppError::from)
                    .context("error creating pause screen")?;
                pause_texture = Some(texture);
            }

            let texture = pause_texture.as_ref().unwrap();

            canvas
                .copy(texture, None, None)
                .map_err(AppError::from)
                .context("error drawing pause screen")?;
        }

        // update the screen
        canvas.present();
    }

    // pause_texture = None;
    audio_device.pause();
    Ok(())
}

fn draw_emulator_screen<'a, T>(
    emu: &Emulator,
    bgcolor: Color,
    fgcolor: Color,
    texture_creator: &'a TextureCreator<T>,
) -> Result<Texture<'a>, AppError> {
    // create the screen surface
    let mut surface = Surface::new(
        (DISPLAY_WIDTH * PIXEL_SIZE) as u32,
        (DISPLAY_HEIGHT * PIXEL_SIZE) as u32,
        PixelFormatEnum::RGBA8888,
    )?;

    // clear the background
    surface.fill_rect(None, bgcolor)?;

    // draw the squares
    for x in 0..DISPLAY_WIDTH {
        for y in 0..DISPLAY_HEIGHT {
            if emu.get_pixel(x, y) {
                let rect = Rect::new(
                    (x * PIXEL_SIZE) as i32,
                    (y * PIXEL_SIZE) as i32,
                    PIXEL_SIZE as u32,
                    PIXEL_SIZE as u32,
                );
                surface.fill_rect(rect, fgcolor)?;
            }
        }
    }

    Ok(texture_creator.create_texture_from_surface(surface)?)
}

fn draw_pause_screen<'a, T>(
    font: &Font,
    texture_creator: &'a TextureCreator<T>,
) -> Result<Texture<'a>, AppError> {
    const TEXT: &str = "-- PAUSE --";
    const BG_COLOR: Color = Color::RGBA(0x80, 0x80, 0x80, 240);
    const FG_COLOR: Color = Color::BLACK;

    // create the text surface and compute the rect size for
    // the center of the screen
    let text = font.render(TEXT).solid(FG_COLOR)?;
    let text_rect = {
        let (w, h) = font.size_of(TEXT)?;
        let center_w = DISPLAY_WIDTH * PIXEL_SIZE / 2;
        let center_h = DISPLAY_HEIGHT * PIXEL_SIZE / 2;
        let x = (center_w as u32) - (w / 2);
        let y = (center_h as u32) - (h / 2);

        Rect::new(x as i32, y as i32, w, h)
    };

    // create a surface to paint the screen
    let mut surface = Surface::new(
        (DISPLAY_WIDTH * PIXEL_SIZE) as u32,
        (DISPLAY_HEIGHT * PIXEL_SIZE) as u32,
        PixelFormatEnum::RGBA8888,
    )?;
    surface.set_blend_mode(BlendMode::Blend)?;

    // semi-transparent background
    surface.fill_rect(None, BG_COLOR)?;

    // text
    text.blit(None, &mut surface, text_rect)?;

    // return the texture
    Ok(texture_creator.create_texture_from_surface(&surface)?)
}
