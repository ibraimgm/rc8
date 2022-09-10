use anyhow::Context;
use clap::{ArgGroup, CommandFactory, ErrorKind, Parser};

mod app;
mod beep;
mod emulator;
mod keymap;

use app::PIXEL_SIZE;
use emulator::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

const MIN_SCREEN_WIDTH: u32 = (DISPLAY_WIDTH * PIXEL_SIZE) as u32;
const MIN_SCREEN_HEIGHT: u32 = (DISPLAY_HEIGHT * PIXEL_SIZE) as u32;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(group(
    ArgGroup::new("ssize")
        .args(&["window-size", "fullscreen"])
))]
struct Cli {
    /// ROM file to load
    #[clap(value_parser)]
    filename: String,

    /// Size of the window (WxH)
    #[clap(short, long)]
    window_size: Option<String>,

    /// Enable fullscreen
    #[clap(short, long)]
    fullscreen: bool,
}

fn main() -> Result<(), anyhow::Error> {
    // parse command-line arguments
    let cli = Cli::parse();

    // get the desired resolution, or use the default
    let (width, height) = match cli.window_size {
        Some(spec) => {
            let mut splitted: Vec<&str> = spec.split('x').collect();
            if splitted.len() != 2 {
                Cli::command()
                    .error(
                        clap::ErrorKind::Format,
                        "WINDOW-SIZE must be in the format (width)x(height)",
                    )
                    .exit();
            }

            let width = validate_resolution(splitted.remove(0), "WIDTH", MIN_SCREEN_WIDTH);
            let height = validate_resolution(splitted.remove(0), "HEIGHT", MIN_SCREEN_HEIGHT);

            (width, height)
        }
        None => (MIN_SCREEN_WIDTH, MIN_SCREEN_HEIGHT),
    };

    // load the rom and build the emulator
    let rom = std::fs::File::open(&cli.filename)
        .with_context(|| format!("error opening rom file: {}", &cli.filename))?;

    // load the rom
    let emu = emulator::Emulator::load_rom(rom).context("error loading rom")?;

    // run
    app::run(emu, width, height, cli.fullscreen)?;
    Ok(())
}

fn validate_resolution(input: &str, field: &str, min: u32) -> u32 {
    let value = input.parse::<u32>().ok().unwrap_or_default();

    if value < min {
        Cli::command()
            .error(
                ErrorKind::InvalidValue,
                format!(
                    "{} on WINDOW-SIZE must be greater than or equal {}",
                    field, min
                ),
            )
            .exit();
    }

    value
}
