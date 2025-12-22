#![feature(adt_const_params)]
#![feature(generic_const_exprs)]

use sw_structure_io::structs::*;
use sw_structure_io::io::WriteBuilding;
use clap::Parser;
use std::fs::File;
use std::io::{self, Write};
use std::collections::{BTreeMap, HashMap};
use crate::timebytesequence::*;

type StateUnit = u16;

const CHUNKS_COUNT: usize = 128usize / StateUnit::BITS as usize;
const CHUNK_SIZE: usize = StateUnit::BITS as usize;

#[derive(Parser, Debug)]
#[command(name = "midi2swstruct")]
#[command(
    version,
    about       = "Converts MIDI-file to SW building",
    long_about  = "Converts MIDI-file to Sandbox World structure file with music player, that contains data from MIDI-file."
)]
struct Args {
    /// Input MIDI-file.
    input: String,

    /// Optional output path ("-" for stdout).
    #[arg(short, long, default_value = "-")]
    output: String,

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
    #[arg(long, default_value = "2048")]
    max_events_per_func: usize,

    /// Minimal velocity for note to be flagged as active.
    #[arg(long, default_value = "31")]
    min_velocity: u8,

    /// If true, music will repeat.
    #[arg(short, long, default_value = "false")]
    repeat: bool,
}

fn open_output(dest: &str) -> io::Result<Box<dyn Write>> {
    if dest == "-" {
        Ok(Box::new(io::stdout().lock()))
    } else {
        Ok(Box::new(File::create(dest)?))
    }
}

#[derive(Debug, Clone)]
struct TempoState {
    time: usize,
    state: u32
}

#[derive(Debug, Clone)]
enum Timing {
    Metrical {
        tpq: usize,
        tempo: Vec<TempoState>
    },
    Timecode {
        tps: usize
    }
}

trait GetSequenceTiming {
    fn get_sequence_timing(&self) -> TimeByteMultiSequence<>
}

trait IntoTimeByteMultiSequence {
    fn into_time_byte_multi_sequence(&self, min_vel: u8) -> TimeByteMultiSequence<16>;
}

impl<'a> IntoTimeByteMultiSequence for midly::Smf<'a> {
    fn into_time_byte_multi_sequence(&self, min_vel: u8) -> TimeByteMultiSequence<16> {
        use midly::{TrackEventKind, MidiMessage};
        use std::array::from_fn;

        let mut merged_multi_sequence = TimeByteMultiSequence::<16>::new();

        for track in &self.tracks {
            let mut sequences: [TimeByteSequence; 16] = from_fn(|_| TimeByteSequence::new());
            let mut time: usize = 0;
            for event in track {
                time = time.checked_add(event.delta.as_int() as usize).unwrap();
                if let TrackEventKind::Midi { channel: _, message } = event.kind {
                    match message {
                        MidiMessage::NoteOn { key, vel } => {
                            let vel_val = vel.as_int();
                            let on = vel_val != 0 && vel_val > min_vel;
                            let note = key.as_int();
                            let group = (note / 8) as usize;
                            if group < 16 {
                                let bit = (note % 8) as usize;
                                let mask = 1u8.checked_shl(bit as u32).unwrap_or(0);
                                if on {
                                    *sequences[group].get_value_or_default_mut(time) |= mask;
                                } else {
                                    *sequences[group].get_value_or_default_mut(time) &= !mask;
                                }
                            }
                        }
                        MidiMessage::NoteOff { key, .. } => {
                            let note = key.as_int();
                            let group = (note / 8) as usize;
                            if group < 16 {
                                let bit = (note % 8) as usize;
                                let mask = 1u8.checked_shl(bit as u32).unwrap_or(0);
                                *sequences[group].get_value_or_default_mut(time) &= !mask;
                            }
                        }
                        _ => continue
                    }
                }
            }
            merged_multi_sequence |= &Into::<TimeByteMultiSequence<16>>::into(sequences);
        }

        merged_multi_sequence.optimize();

        merged_multi_sequence
    }
}

fn gen_expr_from_chang_map(state_changes: &HashMap<isize, Vec<u64>>) -> String {
    let expr = state_changes
        .iter()
        .map(|(change_value, time_keys)| {
            let mut expr = format!("{change_value}");

            expr.push('*');

            expr.push('(');
            expr.push_str(
                &time_keys
                    .iter()
                    .map(|time| format!("step({time},x)"))
                    .collect::<Vec<_>>()
                    .join("+")
            );
            expr.push(')');

            expr
        })
        .collect::<Vec<_>>()
        .join("+");

    expr
}

fn chunk_to_funcs(chunk: &Vec<NotePackedStates>, max_events_per_func: usize) -> Vec<String> {
    let mut funcs: Vec<String> = Vec::new();

    let mut event_counter: usize = 0;
    let mut last_state: isize = 0;
    let mut changes_map: HashMap<isize, Vec<u64>> = HashMap::new();
    for event in chunk {
        if event_counter >= max_events_per_func {
            println!("d");
            funcs.push(format!("{}+(-1/0)*step({},x)", gen_expr_from_chang_map(&changes_map), event.time));
            changes_map.clear();
            event_counter = 0;
            last_state = 0;
        }

        let change = event.state as isize - last_state;
        if change != 0 {
            changes_map
                .entry(change)
                .or_default()
                .push(event.time);
        }
        last_state = event.state as isize;

        event_counter += 1;
    }

    if event_counter != 0 {
        funcs.push(gen_expr_from_chang_map(&changes_map));
    }

    funcs
}

fn tempo_to_func(tempo_changes: Vec<TempoState>) -> String {
    let mut last_state: isize = 0;
    let mut changes_map: HashMap<isize, Vec<u64>> = HashMap::new();
    for tempo in tempo_changes {
        let change = tempo.state as isize - last_state;
        if change != 0 {
            changes_map
                .entry(change)
                .or_default()
                .push(tempo.time);
        }
        last_state = tempo.state as isize;
    }

    gen_expr_from_chang_map(&changes_map)
}

fn math_block_with_func(func: String) -> Block {
    Block {
        id: 129,
        metadata: Some(Metadata {
            type_settings: TypeSettings::MathBlock {
                function: func,
                incoming_connections_order: Vec::new(),
                slots: Vec::new()
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn midi_to_freq(midi_pitch: u8) -> f32 {
    const A4_FREQ: f32 = 440.0;
    let exp = (midi_pitch as f32 - 69.0) / 12.0;
    A4_FREQ * 2.0f32.powf(exp)
}

fn generate_music_player(chunk_funcs: Vec<Vec<String>>, tempo_func: String, length: u64, chunk_size: u8, min_pitch: u8, timing: midly::Timing, repeat: bool) -> Building {
    let mut building = Building::default();
    building.roots.push(Root::default());

    const SWITCH_POSITION: [f32; 3] = [0.0, 1.0 / 256.0, 0.25];
    const TONE_GEN_POSITION: [f32; 3] = [0.0, 0.0, -0.25];

    building.blocks = vec![
        Block {
            id: 9,
            position: SWITCH_POSITION,
            ..Default::default()
        }, // 0 - Switch
        Block { id: 129, ..Default::default() }, // 1 - Math block
        Block { id: 78,  ..Default::default() }  // 2 - OR gate
    ];

    building.blocks[0].connections.push(1);

    let mut main_mblock_func = format!("x=Lval*{length};");
    main_mblock_func.push_str(&match timing {
        Timing::Metrical(tpq) => {
            format!("t={};(A*(x+({}*1000000/t)/50)/{})", tempo_func, tpq.as_int(), length)
        },
        Timing::Timecode(fps, ticks_per_frame) => {
            let fps = match fps {
                Fps::Fps24 => 24.0,
                Fps::Fps25 => 25.0,
                Fps::Fps29 => 29.97,
                Fps::Fps30 => 30.0,
            };
            let ticks_per_second = fps * ticks_per_frame as f32;
            format!("(A*(x+{}/50)/{})", ticks_per_second, length)
        }
    });
    if repeat {
        main_mblock_func.push_str("%1");
    }

    building.blocks[1] = math_block_with_func(main_mblock_func);
    building.blocks[1].connections.push(2);
    building.blocks[2].connections.push(1);

    let mut decoder_func = format!("x=ind(0)*2^{chunk_size};");
    for i in 0..chunk_size {
        let index = i + 1;
        decoder_func.push_str(&format!("ind({index})=floor((x/2^{i})%2);"));
    }
    decoder_func.push('0');
    let decoder_math_block = math_block_with_func(decoder_func);

    let mut current_pitch = min_pitch;
    for chunk in chunk_funcs.into_iter() {
        building.blocks.push(Block { id: 78, ..Default::default() });
        let or_gate_i = building.blocks.len() - 1;

        for func in chunk.into_iter() {
            building.blocks.push(math_block_with_func(format!(
                "x=A*{};n={};n/(2^{})", length, func, chunk_size
            )));
            let index = building.blocks.len() - 1;

            building.blocks[1].connections.push(index as u16);
            building.blocks[index].connections.push(or_gate_i as u16);
        }

        building.blocks.push(decoder_math_block.clone());
        let decoder_i = building.blocks.len() - 1;
        building.blocks[or_gate_i].connections.push(decoder_i as u16);

        for _ in 0..chunk_size {
            building.blocks.push(Block {
                id: 125,
                position: TONE_GEN_POSITION,
                metadata: Some(Metadata {
                    values: vec![midi_to_freq(current_pitch), 100.0],
                    ..Default::default()
                }),
                ..Default::default()
            });
            let tone_gen_i = building.blocks.len() - 1;
            building.blocks[decoder_i].connections.push(tone_gen_i as u16);
            current_pitch += 1;
        }
    }

    building
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let midi_data = std::fs::read(args.input)?;
    let track = read_midi_filtered_packed(
        &midi_data,
        args.min_pitch,
        args.max_pitch,
        args.min_velocity,
        args.chunk_size
    )?;

    let mut chunk_funcs: Vec<Vec<String>> = Vec::new();
    for chunk in track.note_states.iter() {
        chunk_funcs.push(chunk_to_funcs(chunk, args.max_events_per_func));
    }

    let tempo_func = tempo_to_func(track.tempo_changes);

    let building = generate_music_player(chunk_funcs, tempo_func, track.length, args.chunk_size, args.min_pitch, track.timing, args.repeat);

    let mut out = open_output(&args.output)?;

    out.write_building(&building, args.structure_version)?;

    Ok(())
}
