//! `ug2` — read Need for Speed: Underground 2 car assets from the command line.
//!
//! One entry point over the whole parser: summarise a car, list the parts a configuration
//! selects, print the raw chunk tree of any asset file, and export geometry + textures to
//! formats other tools read (OBJ/MTL + PNG).
//!
//! It ships no game data and reads only what you point it at — you must own your copy of the
//! game. Build it with `--features tools`:
//!
//! ```text
//! cargo run -p gizmo-nfs --features tools --bin ug2 -- info  "$NFSU2_ROOT/CARS/240SX"
//! cargo run -p gizmo-nfs --features tools --bin ug2 -- export "$NFSU2_ROOT/CARS/240SX" -o out/
//! ```

/// Print a line to stdout, treating a closed pipe as "the reader has seen enough" rather than a
/// panic. `ug2 parts CARS/240SX | head` is ordinary use, and Rust's `println!` panics on EPIPE.
///
/// Declared before the modules below so they all see it (macros are textually scoped).
macro_rules! outln {
    () => { outln!("") };
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        if writeln!(std::io::stdout(), $($arg)*).is_err() {
            std::process::exit(0);
        }
    }};
}

mod diff;
mod dump;
mod export;
mod fields;
mod globalb;
mod info;
mod parts;
mod paths;
mod probe;
mod textures;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

/// Read Need for Speed: Underground 2 car assets — inspect them, or export them to OBJ/PNG.
#[derive(Parser)]
#[command(name = "ug2", version, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// A car configuration, shared by the commands that assemble one.
#[derive(clap::Args, Clone, Copy)]
struct ConfigArgs {
    /// Body kit `KIT##` (front + rear bumper + side skirt); 0 = stock.
    #[arg(long, default_value_t = 0, value_name = "N")]
    kit: u8,
    /// Hood design `STYLE##`; 0 = stock.
    #[arg(long, default_value_t = 0, value_name = "N")]
    hood: u8,
    /// Head/tail-light design `STYLE##`; 0 = stock.
    #[arg(long, default_value_t = 0, value_name = "N")]
    light: u8,
    /// Widebody kit `KITW##` (body + doors); 0 = stock.
    #[arg(long, default_value_t = 0, value_name = "N")]
    wide: u8,
}

impl From<ConfigArgs> for gizmo_nfs::CarConfig {
    fn from(a: ConfigArgs) -> Self {
        Self { body_kit: a.kit, hood_style: a.hood, light_style: a.light, widebody: a.wide }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Summarise a car: its parts, the variants it ships, its dimensions and wheel mounts.
    Info {
        /// A car directory (`CARS/240SX`) or its `GEOMETRY.BIN`.
        car: PathBuf,
        #[command(flatten)]
        config: ConfigArgs,
    },
    /// List a car's parts, grouped by customization namespace.
    Parts {
        /// A car directory or its `GEOMETRY.BIN`.
        car: PathBuf,
        /// Show only the parts the given configuration selects, with their material group.
        #[arg(long)]
        selected: bool,
        #[command(flatten)]
        config: ConfigArgs,
    },
    /// Read a chunk back as labelled fields — the same model STRUKT's inspector shows.
    Fields {
        /// A car directory or its `GEOMETRY.BIN`.
        car: PathBuf,
        /// A single chunk, by the hex offset of its header (`--at 0x1b8`).
        #[arg(long, value_name = "OFFSET")]
        at: Option<String>,
        /// Otherwise: every chunk whose type name or id contains this.
        #[arg(long, value_name = "SUBSTR")]
        filter: Option<String>,
    },
    /// Print the chunk tree of any asset file (or list a BIGF/VIV archive).
    Dump {
        /// The file to inspect; any codec is detected and decompressed first.
        file: PathBuf,
        /// How deep to print.
        #[arg(long, default_value_t = 64, value_name = "N")]
        max_depth: u32,
        /// Leading payload bytes to show as hex per leaf.
        #[arg(long, default_value_t = 16, value_name = "N")]
        hex: usize,
    },
    /// List a car's textures and how its material runs map onto them.
    Textures {
        /// A car directory or its `GEOMETRY.BIN`.
        car: PathBuf,
        /// Only show parts whose name contains this.
        #[arg(long, value_name = "SUBSTR")]
        filter: Option<String>,
    },
    /// Print the wheel mounts, radius and mass recorded in `GLOBALB.BUN`.
    Globalb {
        /// `GLOBAL/GLOBALB.BUN`, or a car directory / game root to find it from.
        path: PathBuf,
        /// Only show cars whose name contains this.
        #[arg(long, value_name = "SUBSTR")]
        filter: Option<String>,
    },
    /// Probe a car's raw solids: vertex/triangle counts, buffer sizes, mesh-header words.
    ///
    /// The reverse-engineering view — this is what tells a car whose vertex layout the parser
    /// does not decode from one it silently skips.
    Probe {
        /// A car directory or its `GEOMETRY.BIN`.
        car: PathBuf,
        /// Only show solids whose name contains this.
        #[arg(long, value_name = "SUBSTR")]
        filter: Option<String>,
        /// Also classify each part's local matrix (placement / baked pose / reflection).
        #[arg(long)]
        matrices: bool,
    },
    /// Compare two asset files chunk by chunk.
    Diff {
        /// The file to read as the "before".
        left: PathBuf,
        /// The file to read as the "after".
        right: PathBuf,
        /// Also list the chunks that are the same.
        #[arg(long)]
        all: bool,
        /// Stop after this many lines.
        #[arg(long, value_name = "N", default_value_t = 200)]
        max: usize,
    },
    /// Export a car to glTF (`.glb`) and OBJ + MTL, with its textures as PNG.
    Export {
        /// A car directory or its `GEOMETRY.BIN`.
        car: PathBuf,
        /// Directory to write into (created if missing).
        #[arg(short, long, value_name = "DIR")]
        out: PathBuf,
        #[command(flatten)]
        config: ConfigArgs,
        /// Export every part in the file, not just the selected configuration.
        #[arg(long)]
        all: bool,
        /// Skip textures (geometry only).
        #[arg(long)]
        no_textures: bool,
        /// Write only one of the two formats.
        #[arg(long, value_name = "FMT", value_parser = ["glb", "obj", "both"], default_value = "both")]
        format: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Info { car, config } => info::run(&car, config.into()),
        Command::Parts { car, selected, config } => parts::run(&car, selected, config.into()),
        Command::Fields { car, at, filter } => fields::run(&car, at, filter.as_deref()),
        Command::Dump { file, max_depth, hex } => dump::run(&file, max_depth, hex),
        Command::Textures { car, filter } => textures::run(&car, filter.as_deref()),
        Command::Globalb { path, filter } => globalb::run(&path, filter.as_deref()),
        Command::Probe { car, filter, matrices } => probe::run(&car, filter.as_deref(), matrices),
        Command::Diff { left, right, all, max } => diff::run(&left, &right, all, max),
        Command::Export { car, out, config, all, no_textures, format } => {
            export::run(&car, &out, config.into(), all, !no_textures, &format)
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ug2: {e}");
            ExitCode::FAILURE
        }
    }
}
