use sw_structure_io::structs::*;
use sw_structure_io::io::WriteBuilding;
use clap::Parser;
use std::fs::File;
use std::io::{self, Read};
use std::collections::{BTreeMap, HashMap};
use midly::{Smf, TrackEventKind, MidiMessage};
use std::fmt::Write;
use std::path::PathBuf;

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
    input: PathBuf,

    /// Optional output path.
    #[arg(short, long)]
    output: Option<PathBuf>,

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

#[derive(Clone, Debug)]
struct NoteEvent {
    time: u32,
    pitch: u8,
    state: bool
}

#[derive(Clone, Debug)]
struct PackedNotesEvent {
    time: u32,
    data: u32
}

fn collect_note_events(smf: Smf, min_velocity: u8) -> Vec<NoteEvent> {
    let mut note_events: Vec<NoteEvent> = Vec::new();

    for track in &smf.tracks {
        let mut abs_time = 0;
        for event in track {
            abs_time += event.delta.as_int();

            if let TrackEventKind::Midi { message, .. } = &event.kind {
                match message {
                    MidiMessage::NoteOn { key, vel } => {
                        if vel.as_int() < min_velocity { continue; }
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

    note_events
}

fn collect_into_packed_channels(mut note_events: Vec<NoteEvent>, pitches_per_channel: usize) -> (Vec<Vec<PackedNotesEvent>>, Vec<u8>, u32) {
    // Finding used pitches
    let mut keys = [false; 128];
    let mut max_ticks = 0u32;
    for e in &note_events {
        max_ticks = max_ticks.max(e.time);
        keys[e.pitch as usize] = true;
    }

    let used_pitches: Vec<u8> = keys
    .iter()
    .enumerate()
    .filter_map(|(pitch, &used)| if used { Some(pitch as u8) } else { None })
    .collect();

    // Map for mapping pitch to index
    let mut pitch_to_index: HashMap<u8, usize> = HashMap::new();
    for (i, &p) in used_pitches.iter().enumerate() {
        pitch_to_index.insert(p, i);
    }

    let num_channels = (used_pitches.len() + pitches_per_channel - 1) / pitches_per_channel;

    let mut packed_channels: Vec<Vec<PackedNotesEvent>> = vec![Vec::new(); num_channels];
    let mut channel_states: Vec<u32> = vec![0; num_channels];
    let mut pitch_counters: Vec<u32> = vec![0; used_pitches.len()];

    note_events.sort_by_key(|e| e.time);

    let mut i = 0;
    while i < note_events.len() {
        let current_time = note_events[i].time;

        // Process all events at the same time
        let mut j = i;
        while j < note_events.len() && note_events[j].time == current_time {
            let e = &note_events[j];

            if let Some(&index) = pitch_to_index.get(&e.pitch) {
                let channel = index / pitches_per_channel;
                let bit_pos = index % pitches_per_channel;

                // Update counter
                if e.state {
                    // !todo
                    // needs check for overflow
                    pitch_counters[index] += 1;
                    if pitch_counters[index] == 1 {
                        // First note on → set bit
                        channel_states[channel] |= 1 << bit_pos;
                    }
                } else {
                    if pitch_counters[index] > 0 {
                        pitch_counters[index] -= 1;
                        if pitch_counters[index] == 0 {
                            // Last note off → clear bit
                            channel_states[channel] &= !(1 << bit_pos);
                        }
                    }
                }
            }
            j += 1;
        }

        // Push one PackedNotesEvent per channel for this time
        for channel in 0..num_channels {
            packed_channels[channel].push(PackedNotesEvent {
                time: current_time,
                data: channel_states[channel],
            });
        }

        i = j; // move to next group of events
    }

    (packed_channels, used_pitches, max_ticks)
}

pub fn pitch_to_freq(pitch: u8) -> f32 {
    440.0 * 2.0_f32.powf((pitch as f32 - 69.0) / 12.0)
}

fn generate_music_player(smf: Smf, min_velocity: u8, notes_per_channel: usize) -> std::result::Result<Building, Box<dyn std::error::Error>> {
    const SWITCH_POSITION         : [f32; 3] = [ 0.0,  0.03125, -0.25 ];
    const TONE_GENERATOR_POSITION : [f32; 3] = [ 0.0,  0.0,      0.25 ];

    let root = Root {
        position: [ 0.0, 0.0, 0.0 ],
        rotation: [ 0.0, 0.0, 0.0 ]
    };

    let (packed_channels, used_pitches, max_ticks) = collect_into_packed_channels(collect_note_events(smf, min_velocity), notes_per_channel);

    fn math_block(function: String) -> Block {
        Block {
            id: 129,
            metadata: Some(Metadata {
                type_settings: TypeSettings::MathBlock {
                    function: function,
                    incoming_connections_order: Vec::new(),
                    slots: Vec::new()
                },
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    let mut blocks: Vec<Block> = vec![
        Block { // 0
            id: 129, // Math block
            metadata: Some(Metadata {
                type_settings: TypeSettings::MathBlock {
                    function: "s=0.001;A*(Lval+s)%1".into(),
                    incoming_connections_order: Vec::new(),
                        slots: Vec::new()
                },
                ..Default::default()
            }),
            connections: vec![2],
            ..Default::default()
        },
        Block { // 1
            id: 9, // Switch
            position: SWITCH_POSITION,
            connections: vec![0, 4],
            ..Default::default()
        },
        Block { // 2
            id: 78, // OR
            ..Default::default()
        },
        Block { // 3
            id: 78, // OR
            connections: vec![0],
            ..Default::default()
        },
        Block { // 4
            id: 84, // Timer
            metadata: Some(Metadata {
                values: vec![0.0, 0.0],
                ..Default::default()
            }),
            connections: vec![0],
            ..Default::default()
        }
    ];

    for (c, channel) in packed_channels.iter().enumerate() {
        let mut prev_data: u32 = 0;
        let mut i: usize = 0;
        let mut changes_keys_grouped: HashMap<i64, Vec<u32>> = HashMap::new();

        let mut converter_func = String::new();

        for (k, k2) in (1..=notes_per_channel).rev().enumerate() {
            write!(converter_func, "ind({})=step(0.5,(A%1/2^{})*2^{})+step(1,A);", k2,k,k)?;
        }

        converter_func.push('0');

        blocks.push(Block {
            id: 129,
            metadata: Some(Metadata {
                type_settings: TypeSettings::MathBlock {
                    function: converter_func,
                    incoming_connections_order: Vec::new(),
                    slots: Vec::new()
                },
                ..Default::default()
            }),
            ..Default::default()
        });
        let converter_block_index: u16 = (blocks.len() - 1).try_into()?;

        blocks.push(Block {
            id: 78,
            connections: vec![converter_block_index],
            ..Default::default()
        });
        let output_block_index: u16 = (blocks.len() - 1).try_into()?;

        for k in 0..notes_per_channel {
            let index = c * notes_per_channel + k;
            let pitch = if let Some(&pitch) = used_pitches.get(index) {
                pitch
            } else { continue; };

            println!("Channel {}, Bit {} -> Pitch {} -> Frequency {}", c, k, pitch, pitch_to_freq(pitch));

            blocks.push(Block {
                id: 125,
                position: TONE_GENERATOR_POSITION,
                metadata: Some(Metadata {
                    values: vec![pitch_to_freq(pitch), 50.0],
                    ..Default::default()
                }),
                ..Default::default()
            });

            let tone_gen_index: u16 = (blocks.len() - 1).try_into()?;
            blocks[converter_block_index as usize].connections.push(tone_gen_index);
        }

        for (e, event) in channel.iter().enumerate() {
            if i > 1024 || e + 1 >= channel.len() {
                let mut function = String::new();
                write!(function, "x=A*{};n=", max_ticks)?;
                for (x, change) in changes_keys_grouped.iter().enumerate() {
                    if x > 0 {
                        function.push('+');
                    }

                    write!(function, "{}*(", change.0)?;

                    for (n, time) in change.1.iter().enumerate() {
                        if n > 0 {
                            function.push('+');
                        }
                        write!(function, "step({},x)", time)?;
                    }

                    function.push(')');
                }
                write!(function, "-{}*step({},x);", prev_data, event.time)?;
                write!(function, "n/{}", 2usize.pow(notes_per_channel as u32))?;

                blocks.push(Block {
                    id: 129,
                    metadata: Some(Metadata {
                        type_settings: TypeSettings::MathBlock {
                            function,
                            incoming_connections_order: Vec::new(),
                            slots: Vec::new()
                        },
                        ..Default::default()
                    }),
                    connections: vec![output_block_index],
                    ..Default::default()
                });

                let index: u16 = (blocks.len() - 1).try_into()?;
                if let Some(b) = blocks.get_mut(2) {
                    b.connections.push(index);
                }

                i = 0;
                changes_keys_grouped = HashMap::new();
            }

            i += 1;
            let change = event.data as i64 - prev_data as i64;
            prev_data = event.data;

            changes_keys_grouped
            .entry(change)
            .and_modify(|v| v.push(event.time))
            .or_insert(vec![event.time]);
        }
    }

    Ok(Building {
        roots: vec![root],
        blocks
    })
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut file = File::open(&args.input)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let smf = Smf::parse(&buffer)?;

    let building = generate_music_player(smf, 1, 24)?;

    let output_path = match args.output {
        Some(p) => p,
        None => {
            // generate default name in current dir
            let mut default_name = args
            .input
            .file_stem()
            .unwrap_or_else(|| std::ffi::OsStr::new("output"))
            .to_os_string();
            default_name.push(".structure"); // or whatever extension
            std::env::current_dir().unwrap().join(default_name)
        }
    };

    println!("Output: {:?}", output_path);

    let mut output_file = File::create(output_path)?;
    output_file.write_building(&building, 0)?;

    Ok(())
}
