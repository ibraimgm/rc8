use anyhow::Context;
use clap::Parser;

mod app;
mod emulator;
mod keymap;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// ROM file to load
    #[clap(value_parser)]
    filename: String,
}

fn main() -> Result<(), anyhow::Error> {
    // parse command-line arguments
    let cli = Cli::parse();

    // lod the rom and build the emulator
    let rom = std::fs::File::open(&cli.filename)
        .with_context(|| format!("error opening rom file: {}", &cli.filename))?;

    // load the rom
    let emu = emulator::Emulator::load_rom(rom).context("error loading rom")?;

    // run
    app::run(emu)?;
    Ok(())
}
