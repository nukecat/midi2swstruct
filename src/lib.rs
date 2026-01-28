use std::collections::{HashMap};
use std::fmt::Write;
use midly::{Smf, TrackEventKind, MidiMessage, Timing, MetaMessage};
use sw_structure_io::structs::{Root, Block, Building, Metadata, TypeSettings};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("formatting error")]
    Format(#[from] std::fmt::Error),
    #[error("int conversion error")]
    FromInt(#[from] std::num::TryFromIntError),
    #[error("unsupported timing")]
    UnsupportedTimingSMPTE
}

type Result<T> = std::result::Result<T, Error>;

fn pitch_to_freq(midi: u8) -> f32 {
    440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0)
}

fn data_to_functions(data_changes: &Vec<(u32, u32)>, max_events_per_func: usize) -> Vec<String> {
    let mut functions: Vec<String> = Vec::new();

    let mut i = 0;
    let mut prev_data = 0;
    let mut diffs: HashMap<i64, Vec<u32>> = HashMap::new();

    for (n, &(time, data)) in data_changes.iter().enumerate() {
        i += 1;
        if i > max_events_per_func || n + 1 == data_changes.len() {
            let mut function = String::new();

            for (j, (&diff, time_keys)) in diffs.iter().enumerate() {
                if j > 0 { function.push('+'); }

                let _ = write!(function, "{}*(", diff);

                for (k, t) in time_keys.iter().enumerate() {
                    if k > 0 { function.push('+'); }
                    let _ = write!(function, "step({},x)", t);
                }

                function.push(')');
            }

            let _ = write!(function, "-{}*step({},x)", prev_data, time);

            functions.push(function);

            i = 0;
            prev_data = 0;
            diffs = HashMap::new();
        }

        let diff = data as i64 - prev_data as i64;

        diffs
        .entry(diff)
        .and_modify(|e| e.push(time))
        .or_insert(vec![time]);

        prev_data = data;
    }

    functions
}

pub fn generate_music_player(
    smf: Smf,
    notes_per_value: usize,
    min_pitch: u8,
    max_pitch: u8,
    min_velocity: u8,
    repeat: bool,
    max_events_per_func: usize
) -> Result<Building> {
    // Special positions for blocks.
    const SWITCH_POSITION   : [f32; 3] = [ 0.0 , 0.015625 ,  0.25 ];
    const TONE_GEN_POSITION : [f32; 3] = [ 0.0 , 0.0      , -0.25 ];

    let ppq = match smf.header.timing {
        Timing::Metrical(t) => t.as_int() as u32,
        Timing::Timecode(_, _) => return Err(Error::UnsupportedTimingSMPTE)
    };

    // Collecting events from all tracks in midi.
    let mut note_events: Vec<(u32, u8, bool)> = Vec::new();
    let mut used_keys = [false; 128];
    let mut total_len = 0;

    let mut tempo_events: Vec<(u32, u32)> = Vec::new();

    tempo_events.push((0, 120));
    for track in &smf.tracks {
        let mut abs_time = 0;
        for event in track {
            abs_time += event.delta.as_int();

            match event.kind {
                TrackEventKind::Midi { message, .. } => {
                    let (key, state) = match message {
                        MidiMessage::NoteOn  { key, vel } => (key.as_int(), vel.as_int() > min_velocity),
                        MidiMessage::NoteOff { key, ..  } => (key.as_int(), false),
                        _ => continue
                    };
                    if key < min_pitch || key > max_pitch { continue; }
                    note_events.push((abs_time, key, state));
                    used_keys[key as usize] = true;
                },
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => {
                    tempo_events.push((abs_time, t.as_int()));
                },
                _ => {}
            }
        }
        total_len = total_len.max(abs_time);
    }

    // Sorting events (because we collected them from different tracks and instruments)
    note_events.sort_by_key(|e| e.0);
    tempo_events.sort_by_key(|e| e.0);

    tempo_events.push((u32::MAX, 120));

    // Creating hash map for mapping used keys to indices.
    let mut key_mapping: HashMap<u8, usize> = HashMap::new();
    let mut index_to_key: Vec<u8> = Vec::new();

    let mut i = 0;

    for (key, &is_used) in used_keys.iter().enumerate() {
        if !is_used { continue; }
        key_mapping.insert(key as u8, i);
        index_to_key.push(key as u8);
        i += 1;
    }

    let used_keys_count = key_mapping.len();
    let channels_count = ((used_keys_count.checked_sub(1).unwrap_or(0)) / notes_per_value) + 1;

    // Encoding note changes into bits of values.
    let mut note_counters = vec![0u8; used_keys_count];
    let mut data_changes: Vec<Vec<(u32, u32)>> = vec![Vec::new(); channels_count];

    let mut i = 0;

    while i < note_events.len() {
        let current_time = note_events[i].0;

        let mut j = i;
        while j < note_events.len() && note_events[j].0 == current_time {
            j += 1;
        }

        let events = &note_events[i..j];

        for &(_, key, state) in events {
            let mapped = key_mapping[&key];
            // !todo: needs check for overflow
            if state == true {
                note_counters[mapped] = note_counters[mapped]
                .checked_add(1)
                .unwrap_or(u8::MAX);
            } else {
                note_counters[mapped] = note_counters[mapped]
                .checked_sub(1)
                .unwrap_or(0);
            }
        }

        for c in 0..channels_count {
            let prev_data = data_changes[c].last().unwrap_or(&(0, 0)).1;
            let mut data = 0;
            for bit in 0..notes_per_value {
                let index = c * notes_per_value + bit;
                if !(index < used_keys_count) || note_counters[index] < 1 { continue; }
                data = data | 1 << bit;
            }
            if data != prev_data {
                data_changes[c].push((current_time, data));
            }
        }

        i = j;
    }

    // Decoder function
    let mut decoder_func = String::new();
    for (p, ind) in (1..=notes_per_value).rev().enumerate() {
        write!(decoder_func, "ind({})=step(0.5,(A%1/2^{})*2^{})+step(1,A);", ind, p, p)?;
    }
    decoder_func.push('0');

    // Initializing array with blocks that are always present.
    let mut blocks: Vec<Block> = vec![
        Block { // 0
            id: 129, // Math block
            metadata: Some(Metadata {
                type_settings: TypeSettings::MathBlock {
                    function: {
                        let mut f = String::new();
                        write!(
                            f,
                            "s=0.0001;tempo_us=max(C*{},1);dt_sec=1/50;dt_ticks=dt_sec*{}*1000000/tempo_us;B=1-B;A*(Lval+dt_ticks/{}){}",
                               2u32.pow(24),
                               ppq,
                               total_len,
                               if repeat { "%1" } else { "" }
                        )?;
                        f
                    },
                    incoming_connections_order: Vec::new(),
                           slots: Vec::new()
                },
                ..Default::default()
            }),
            connections: vec![2, 4],
            ..Default::default()
        },
        Block { // 1: A
            id: 9, // Switch
            position: SWITCH_POSITION,
            connections: vec![0],
            ..Default::default()
        },
        Block { // 2 - Forces math block to update: B
            id: 78, // OR
            connections: vec![0],
            name: "2".into(),
            ..Default::default()
        },
        Block { // 3 - Main math block input (for tempo): C
            id: 78, // OR
            connections: vec![0],
            name: "3".into(),
            ..Default::default()
        },
        Block { // 4 - Main math block output: D
            id: 78, // OR
            name: "4".into(),
            ..Default::default()
        }
    ];

    // Generating funcs and creating blocks
    for c in 0..channels_count {
        blocks.push(Block {
            id: 129,
            metadata: Some(Metadata {
                type_settings: TypeSettings::MathBlock {
                    function: decoder_func.clone(),
                           incoming_connections_order: Vec::new(),
                           slots: Vec::new()
                },
                ..Default::default()
            }),
            ..Default::default()
        });
        let decoder_index: u16 = (blocks.len() - 1).try_into()?;

        blocks.push(Block {
            id: 78,
            connections: vec![decoder_index],
            ..Default::default()
        });
        let decoder_input_index: u16 = (blocks.len() - 1).try_into()?;

        for n in 0..notes_per_value {
            let index = c * notes_per_value + n;
            if !(index < used_keys_count) { continue; }
            let pitch = index_to_key[index];
            let freq = pitch_to_freq(pitch);

            blocks.push(Block {
                id: 125,
                position: TONE_GEN_POSITION,
                metadata: Some(Metadata {
                    values: vec![freq, 100.0],
                    ..Default::default()
                }),
                ..Default::default()
            });
            let tone_gen_index: u16 = (blocks.len() - 1).try_into()?;

            if let Some(decoder) = blocks.get_mut(decoder_index as usize) {
                decoder.connections.push(tone_gen_index);
            }
        }

        let functions = data_to_functions(&data_changes[c], max_events_per_func);

        for f in functions {
            blocks.push(Block {
                id: 129,
                metadata: Some(Metadata {
                    type_settings: TypeSettings::MathBlock {
                        function: format!("x=A*{};n={};n/{}", total_len, f, 2u32.pow(notes_per_value as u32)),
                               incoming_connections_order: Vec::new(),
                               slots: Vec::new()
                    },
                    ..Default::default()
                }),
                connections: vec![decoder_input_index],
                ..Default::default()
            });

            let data_block_index: u16 = (blocks.len() - 1).try_into()?;

            if let Some(main_output) = blocks.get_mut(4) {
                main_output.connections.push(data_block_index);
            }
        }
    }

    // Generating blocks with tempo data
    let functions = data_to_functions(&tempo_events, max_events_per_func);
    for f in functions {
        blocks.push(Block {
            id: 129,
            metadata: Some(Metadata {
                type_settings: TypeSettings::MathBlock {
                    function: format!("x=A*{};n={};n/{}", total_len, f, 2u32.pow(24)),
                           incoming_connections_order: Vec::new(),
                           slots: Vec::new()
                },
                ..Default::default()
            }),
            connections: vec![3],
            ..Default::default()
        });

        let data_block_index: u16 = (blocks.len() - 1).try_into()?;

        if let Some(main_output) = blocks.get_mut(4) {
            main_output.connections.push(data_block_index);
        }
    }

    Ok(Building {
        roots: vec![Root::default()],
       blocks
    })
}
