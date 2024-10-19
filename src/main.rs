mod lib;
use lib::character_encoding::CHAR_ENCODING_US;
use std::{
    error::Error,
    fs::{self, File},
    io,
};

const SAVESIZE: usize = 131072;
const SECTIONS: usize = 14;
const SIGNATURE: u32 = 0x08012025;
const PLAYERNAMELEN: usize = 7;

struct FileStructure {
    gamesave: [Vec<Section>; 2],
}

impl FileStructure {
    fn new() -> Self {
        FileStructure {
            gamesave: [Vec::new(), Vec::new()],
        }
    }
}

#[derive(Debug)]
struct Section {
    data: SectionData,
    sectionid: u16,
    checksum: u16,
    signature: u32,
    saveindex: u32,
}

impl Section {
    fn new(data: SectionData) -> Self {
        Section {
            data,
            sectionid: 0,
            checksum: 0,
            signature: 0,
            saveindex: 0,
        }
    }
}

#[derive(Debug)]
enum SectionData {
    TRAINERINFO(TrainerInfo),
}

#[derive(Debug)]
struct TrainerInfo {
    playername: String,
    playergender: bool,
    trainerid: u32,
    timeplayed: u32,
    timeframes: u8,
    options: u32,
    gamecode: u32,
    securitykey: u32,
}

impl TrainerInfo {
    fn new(data: Vec<u8>) -> Self {
        let mut playername = String::new();

        // Player name
        for i in 0..PLAYERNAMELEN {
            playername.push(CHAR_ENCODING_US[data[i] as usize]);
        }

        // true: female, false: male
        let playergender = if data[0x08] == 0x00 { false } else { true };

        // lower 16 bits is public ID, top 16 bits is secret ID
        let trainerid =
            u32::from_le_bytes([data[0x0A], data[0x0A + 1], data[0x0A + 2], data[0x0A + 3]]);

        let timeplayed =
            u32::from_le_bytes([data[0x0E], data[0x0E + 1], data[0x0E + 2], data[0x0E + 3]]);

        let timeframes = data[0x12];

        // MSB is unused
        let options = u32::from_le_bytes([data[0x13], data[0x13 + 1], data[0x13 + 2], 0x00 as u8]);

        let gamecode =
            u32::from_le_bytes([data[0xAC], data[0xAC + 1], data[0xAC + 2], data[0xAC + 3]]);

        let securitykey = u32::from_le_bytes([
            data[0xAF8],
            data[0xAF8 + 1],
            data[0xAF8 + 2],
            data[0xAF8 + 3],
        ]);

        TrainerInfo {
            playername,
            playergender,
            trainerid,
            timeplayed,
            timeframes,
            options,
            gamecode,
            securitykey,
        }
    }

    fn default() -> Self {
        TrainerInfo {
            playername: String::from(""),
            playergender: false,
            trainerid: 0,
            timeplayed: 0,
            timeframes: 0,
            options: 0,
            gamecode: 0,
            securitykey: 0,
        }
    }
}




fn get_save_from_data(data: &Vec<u8>) -> Result<FileStructure, String> {
    if data.len() != SAVESIZE {
        return Err(format!(
            "Unexpected size: {}. Expected: {}.",
            data.len(),
            SAVESIZE
        ));
    }

    let mut savefile = FileStructure::new();
    let mut offset: usize = 0;

    for gamesave in savefile.gamesave.iter_mut() {
        for i in 0..SECTIONS {
            let section_id = u16::from_le_bytes([data[offset + 0x0FF4], data[offset + 0x0FF5]]);

            match section_id {
                0 => {
                    println!("Trainer info section {section_id}");

                    gamesave.push(Section::new(SectionData::TRAINERINFO(TrainerInfo::new(
                        data[offset..offset + 0x0F2C].to_vec(),
                    ))));
                }
                _ => {
                    gamesave.push(Section::new(SectionData::TRAINERINFO(
                        TrainerInfo::default(),
                    )));
                    println!("Another section {section_id}");
                }
            }

            gamesave[i].signature = u32::from_le_bytes([
                data[offset + 0x0FF8],
                data[offset + 0x0FF9],
                data[offset + 0x0FFA],
                data[offset + 0x0FFB],
            ]);

            if gamesave[i].signature != SIGNATURE {
                return Err(format!(
                    "Signature mismatch! Section block {} invalid.\nExpected: 0x{:x} - Result: 0x{:x}",
                    gamesave[i].sectionid, SIGNATURE, gamesave[i].signature
                ));
            }

            gamesave[i].sectionid = section_id;

            gamesave[i].saveindex = u32::from_le_bytes([
                data[offset + 0x0FFC],
                data[offset + 0x0FFD],
                data[offset + 0x0FFE],
                data[offset + 0x0FFF],
            ]);

            // Now get the checksum and calculate from data to check for
            // invalid section blocks.
            gamesave[i].checksum =
                u16::from_le_bytes([data[offset + 0x0FF6], data[offset + 0x0FF7]]);

            match calculate_checksum(
                &data[offset..offset + 0x0F80].to_vec(),
                gamesave[i].checksum,
            ) {
                Ok(_) => {}
                Err(e) => {
                    return Err(format!("{} for section {}", e, section_id));
                }
            }

            offset += 0x1000;
        }
        println!();
    }

    return Ok(savefile);
}

fn calculate_checksum(section_raw_data: &[u8], expected_checksum: u16) -> Result<bool, String> {
    let mut checksum: u32 = 0;

    // Read 4 bytes at a time and add them as a 32-bit word (little-endian)
    for chunk in section_raw_data.chunks(4) {
        let mut word: u32 = 0;
        for (i, &byte) in chunk.iter().enumerate() {
            word |= (byte as u32) << (i * 8);
        }
        checksum = checksum.wrapping_add(word);
    }

    // Fold the upper 16 bits into the lower 16 bits
    let upper = (checksum >> 16) as u16;
    let lower = (checksum & 0xFFFF) as u16;

    let result = lower.wrapping_add(upper);

    if result == expected_checksum {
        Ok(true)
    } else {
        Err(format!(
            "Checksum calculation failed! Expected: 0x{:x} - Result: 0x{:x}",
            expected_checksum, result
        ))
    }
}

fn main() {
    let data = fs::read("D:/Roms/GBA/Pokemon_FireRed.sav").expect("Unable to read file");

    if data.len() != SAVESIZE {
        println!("Unexpected size: {}. Expected: {}.", data.len(), SAVESIZE);
        return;
    }

    println!("Save data length: {} bytes\n", data.len());

    let mut savefile = match get_save_from_data(&data) {
        Ok(result) => result,
        Err(e) => {
            panic!("**{}**", e)
        }
    };
}
