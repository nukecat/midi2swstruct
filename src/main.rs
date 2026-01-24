use sw_structure_io::structs::*;
use sw_structure_io::io::WriteBuilding;
use clap::Parser;
use std::fs::File;
use std::io::{self, Read, Write};
use std::collections::{BTreeMap, HashMap};
use midly::{Smf, TrackEventKind, MidiMessage};

#[derive(Parser, Debug)]
#[command(name = "midi2swstruct")]
#[command(
    version,
    about       = "Converts MIDI-file to SW building",
    long_about  = "Converts MIDI-file to Sandbox World structure file with music player, that contains data from MIDI-file."
)]
struct Args {
    /// Input MIDI-file.
    #[arg(short, long)]
    input: String,

    /// Optional output path ("-" for stdout).
    #[arg(short, long)]
    output: Option<String>,

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
    #[arg(long, default_value = "31")]
    min_velocity: u8,

    /// If true, music will repeat.
    #[arg(short, long, default_value = "false")]
    repeat: bool,
}

struct NoteEvent {
    time: u32,
    pitch: u8,
    state: bool
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut file = File::open(args.input)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let smf = Smf::parse(&buffer)?;

    let mut note_events: Vec<NoteEvent> = Vec::new();

    for track in &smf.tracks {
        let mut abs_time = 0u32;
        for event in track {
            abs_time += event.delta.as_int();

            if let TrackEventKind::Midi { message, .. } = &event.kind {
                match message {
                    MidiMessage::NoteOn { key, vel } => {
                        if vel < &args.min_velocity { continue; }
                        note_events.push(NoteEvent {
                            time: abs_time,
                            pitch: key.as_int(),
                            state: true
                        });
                    }
                    MidiMessage::NoteOff { key, .. } => {
                        note_events.push(NoteEvent {
                            time: abs_time,
                            pitch: key.as_int(),
                            state: false
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    note_events.sort_by_key(|e| e.time);

    Ok(())
}
