use anyhow::Context;
use clap::{ArgGroup, CommandFactory, ErrorKind, Parser};

mod app;
mod beep;
mod emulator;
mod keymap;

use app::{Options, PIXEL_SIZE};
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

    /// Set the background color
    #[clap(long)]
    bg: Option<String>,

    /// Set the foreground color
    #[clap(long)]
    fg: Option<String>,
}

impl TryFrom<&Cli> for Options {
    type Error = (ErrorKind, String);

    fn try_from(cli: &Cli) -> Result<Self, Self::Error> {
        // screen size
        let (width, height) = match &cli.window_size {
            Some(spec) => {
                let mut splitted: Vec<&str> = spec.split('x').collect();
                if splitted.len() != 2 {
                    return Err((
                        ErrorKind::Format,
                        "WINDOW-SIZE must be in the format (width)x(height)".to_owned(),
                    ));
                }

                let width = validate_resolution(splitted.remove(0), "WIDTH", MIN_SCREEN_WIDTH)?;
                let height = validate_resolution(splitted.remove(0), "HEIGHT", MIN_SCREEN_HEIGHT)?;

                (width, height)
            }
            None => (MIN_SCREEN_WIDTH, MIN_SCREEN_HEIGHT),
        };

        // colors
        let (bgcolor, fgcolor) = match (&cli.bg, &cli.fg) {
            (Some(bgcolor), Some(fgcolor)) => {
                let bgcolor = validate_rgb(bgcolor)?;
                let fgcolor = validate_rgb(fgcolor)?;
                (bgcolor, fgcolor)
            }
            (Some(bgcolor), None) => {
                let bgcolor = validate_rgb(bgcolor)?;
                (bgcolor, 0xffffff00 - bgcolor)
            }
            (None, Some(fgcolor)) => {
                let fgcolor = validate_rgb(fgcolor)?;
                (0xffffff00 - fgcolor, fgcolor)
            }
            (None, None) => (0x00000000, 0xffffff00),
        };

        Ok(Options {
            width,
            height,
            fullscreen: cli.fullscreen,
            bgcolor,
            fgcolor,
        })
    }
}

fn main() -> Result<(), anyhow::Error> {
    // parse command-line arguments
    let cli = Cli::parse();

    // convert to app options
    let options = match Options::try_from(&cli) {
        Ok(options) => options,
        Err((kind, msg)) => {
            Cli::command().error(kind, msg).exit();
        }
    };

    // load the rom and build the emulator
    let rom = std::fs::File::open(&cli.filename)
        .with_context(|| format!("error opening rom file: {}", &cli.filename))?;

    // load the rom
    let emu = emulator::Emulator::load_rom(rom).context("error loading rom")?;

    // run
    app::run(emu, options)?;
    Ok(())
}

fn validate_resolution(input: &str, field: &str, min: u32) -> Result<u32, (ErrorKind, String)> {
    let value = input.parse::<u32>().ok().unwrap_or_default();

    if value < min {
        return Err((
            ErrorKind::Format,
            format!(
                "{} on WINDOW-SIZE must be greater than or equal {}",
                field, min
            ),
        ));
    }

    Ok(value)
}

fn validate_rgb(input: &str) -> Result<u32, (ErrorKind, String)> {
    let stripped = input.strip_prefix('#').unwrap_or(input);

    if stripped.len() != 6 {
        return Err((
            ErrorKind::Format,
            format!("wrong color size (expected: 6, got {})", stripped.len()),
        ));
    }

    let value = match u32::from_str_radix(stripped, 16) {
        Ok(value) => value,
        Err(err) => {
            return Err((
                ErrorKind::Format,
                format!("error parsing color value: {:?}", err),
            ))
        }
    };

    let value = value << 8;
    Ok(value)
}
