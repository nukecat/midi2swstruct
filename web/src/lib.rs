use wasm_bindgen::prelude::*;
use sw_structure_io::io::WriteBuilding;

#[wasm_bindgen]
pub fn generate_sw_from_midi(midi_bytes: &[u8], structure_version: u8) -> Result<Box<[u8]>, JsValue> {
    let smf = midly::Smf::parse(midi_bytes).map_err(|e| e.to_string())?;
    let building = midi2swstruct::generate_music_player(smf, 24, 27, 111, 1, false, 1024)
    .map_err(|e| e.to_string())?;

    let mut buffer: Vec<u8> = Vec::new();
    buffer.write_building(&building, structure_version)
    .map_err(|e| e.to_string())?;

    Ok(buffer.into_boxed_slice())
}

