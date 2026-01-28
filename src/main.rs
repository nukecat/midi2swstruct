use sw_structure_io::io::WriteBuilding;
use clap::Parser;
use std::fs::File;
use std::io::Read;
use midly::Smf;
use std::path::PathBuf;
use anyhow::{Result, Context};

use midi2swstruct::generate_music_player;

#[derive(Parser, Debug)]
#[command(name = "midi2swstruct")]
#[command(
    version,
    about       = "Converts MIDI-file to SW building",
    long_about  = "Converts MIDI-file to Sandbox World structure file with music player, that contains data from MIDI-file."
)]
struct Args {
    /// Input MIDI-file.
    #[arg(value_name = "INPUT", required = true)]
    input: PathBuf,

    /// Optional output path.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Write output to stdout instead of file.
    #[arg(long, default_value_t = false)]
    stdout: bool,

    /// Minimal note pitch.
    #[arg(long, default_value = "27")]
    min_pitch: u8,

    /// Maximal note pitch.
    #[arg(long, default_value = "111")]
    max_pitch: u8,

    /// Structure version.
    #[arg(short, long, default_value = "0")]
    structure_version: u8,

    /// Maximal amount of events per function.
    #[arg(long, default_value = "1024")]
    max_events_per_func: usize,

    /// Minimal velocity for note to be flagged as active.
    #[arg(long, default_value = "1")]
    min_velocity: u8,

    /// If true, music will repeat.
    #[arg(short, long, default_value = "false")]
    repeat: bool,

    /// How many note changes can be encoded in one value.
    #[arg(short, long, default_value = "24")]
    notes_per_value: usize
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Read MIDI file (or from stdin)
    let mut buffer = Vec::new();
    if args.input == PathBuf::from("-") {
        std::io::stdin().read_to_end(&mut buffer)
        .with_context(|| format!("Failed to read input file {:?}", args.input))?;
    } else {
        File::open(&args.input)
        .with_context(|| format!("Failed to open input file {:?}", args.input))?
        .read_to_end(&mut buffer)
        .with_context(|| format!("Failed to read input file {:?}", args.input))?;
    }

    let smf = Smf::parse(&buffer)
    .with_context(|| format!("Failed to parse MIDI file {:?}", args.input))?;

    // Generate building
    let building = generate_music_player(
        smf,
        args.notes_per_value,
        args.min_pitch,
        args.max_pitch,
        args.min_velocity,
        args.repeat,
        args.max_events_per_func,
    ).with_context(|| format!("Failed to generate building"))?;

    // Write output
    if args.stdout {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        handle.write_building(&building, args.structure_version).with_context(|| format!("Failed to serialize building"))?;
    } else {
        let output_path = match args.output {
            Some(p) => p,
            None => {
                let mut default_name = args
                .input
                .file_stem()
                .unwrap_or_else(|| std::ffi::OsStr::new("output"))
                .to_os_string();
                default_name.push(".structure");
                std::env::current_dir()?.join(default_name)
            }
        };
        let mut output_file = File::create(&output_path)
        .with_context(|| format!("Failed to create output file {:?}", output_path))?;

        output_file
        .write_building(&building, args.structure_version).with_context(|| format!("Failed to serialize building"))?;
        println!("Wrote structure file to {:?}", output_path);
    }

    Ok(())
}
