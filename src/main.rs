// based on info found in https://bulbapedia.bulbagarden.net/wiki/Save_data_structure_(Generation_III)

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

// todo: pokemon data structure
// todo: generic saveblock data
// todo: teamanditems implementation
// todo: gamestate implementation
// todo: miscdata implementation
// todo: rivalinfo implementation
// todo: pcbuffer implementation
// todo: hall of fame name with proprietary

struct FileStructure {
    game_save: [Vec<Section>; 2],
}

impl FileStructure {
    fn new() -> Self {
        FileStructure {
            game_save: [Vec::new(), Vec::new()],
        }
    }
}

#[derive(Debug)]
struct Section {
    data: SectionData,
    section_id: u16,
    checksum: u16,
    save_count: u32,
}

impl Section {
    fn new(data: SectionData) -> Self {
        Section {
            data,
            section_id: 0,
            checksum: 0,
            save_count: 0,
        }
    }

    fn to_index(&self) -> u8 {
        match self.data {
            SectionData::TRAINERINFO(_) => 0,
            _ => 255,
        }
    }
}

#[derive(Debug)]
enum SectionData {
    TRAINERINFO(TrainerInfo),
    OTHER,
}

#[derive(Debug)]
struct TrainerInfo {
    player_name: String,
    player_gender: PlayerGender,
    public_id: u16,
    secret_id: u16,
    hours_played: u16,
    minutes_played: u8,
    seconds_played: u8,
    time_frame: u8,
    options: u32,
    game_version: GameVersion,
    security_key: u32,
}

#[derive(Debug)]
enum PlayerGender {
    BOY,
    GIRL,
}

impl PlayerGender {
    fn from_u8(value: u8) -> PlayerGender {
        match value {
            0 => PlayerGender::BOY,
            _ => PlayerGender::GIRL,
        }
    }

    fn to_u8(&self) -> u8 {
        match self {
            PlayerGender::BOY => 0,
            PlayerGender::GIRL => 1,
        }
    }
}

#[derive(Debug)]
enum GameVersion {
    RUBYSAPPHIRE,
    FRLG,
    EMERALD,
}

impl GameVersion {
    fn from_u32(value: u32) -> GameVersion {
        match value {
            0 => GameVersion::RUBYSAPPHIRE,
            1 => GameVersion::FRLG,
            _ => GameVersion::EMERALD,
        }
    }

    fn to_u32(&self) -> u32 {
        match self {
            GameVersion::RUBYSAPPHIRE => 0,
            GameVersion::FRLG => 1,
            GameVersion::EMERALD => 0xFFFFFFFF,
        }
    }
}

// TODO: options individual parsing
impl TrainerInfo {
    fn new(data: Vec<u8>) -> Self {
        let mut playername = String::new();

        // Player name
        for i in 0..PLAYERNAMELEN {
            playername.push(CHAR_ENCODING_US[data[i] as usize]);
        }

        // 0: boy, else: girl
        let playergender = PlayerGender::from_u8(data[0x08]);

        // Both part of the Trainer ID
        let publicid = u16::from_le_bytes([data[0x0A], data[0x0A + 1]]);
        let secretid = u16::from_le_bytes([data[0x0C], data[0x0C + 1]]);

        let hoursplayed = u16::from_le_bytes([data[0x0E], data[0x0E + 1]]);

        let minutesplayed = data[0x10];

        let secondsplayed = data[0x11];

        let timeframes = data[0x12];

        // MSB is unused
        let options = u32::from_le_bytes([data[0x13], data[0x13 + 1], data[0x13 + 2], 0x00 as u8]);

        let gameversion = GameVersion::from_u32(u32::from_le_bytes([
            data[0xAC],
            data[0xAC + 1],
            data[0xAC + 2],
            data[0xAC + 3],
        ]));

        let securitykey = u32::from_le_bytes([
            data[0xAF8],
            data[0xAF8 + 1],
            data[0xAF8 + 2],
            data[0xAF8 + 3],
        ]);

        TrainerInfo {
            player_name: playername,
            player_gender: playergender,
            public_id: publicid,
            secret_id: secretid,
            hours_played: hoursplayed,
            minutes_played: minutesplayed,
            seconds_played: secondsplayed,
            time_frame: timeframes,
            options,
            game_version: gameversion,
            security_key: securitykey,
        }
    }

    fn default() -> Self {
        TrainerInfo {
            player_name: String::from(""),
            player_gender: PlayerGender::BOY,
            public_id: 0,
            secret_id: 0,
            hours_played: 0,
            minutes_played: 0,
            seconds_played: 0,
            time_frame: 0,
            options: 0,
            game_version: GameVersion::RUBYSAPPHIRE,
            security_key: 0,
        }
    }

    const INDEX: u16 = 0;
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

    for gamesave in savefile.game_save.iter_mut() {
        for i in 0..SECTIONS {
            let section_id = u16::from_le_bytes([data[offset + 0x0FF4], data[offset + 0x0FF5]]);

            match section_id {
                TrainerInfo::INDEX => {
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

            if section_id == TrainerInfo::INDEX {
                let signature = u32::from_le_bytes([
                    data[offset + 0x0FF8],
                    data[offset + 0x0FF9],
                    data[offset + 0x0FFA],
                    data[offset + 0x0FFB],
                ]);

                if signature != SIGNATURE {
                    return Err(format!(
                    "Signature mismatch! Section block {} invalid.\nExpected: 0x{:x} - Result: 0x{:x}",
                    gamesave[i].section_id, SIGNATURE, signature
                ));
                }

                gamesave[i].section_id = section_id;

                gamesave[i].save_count = u32::from_le_bytes([
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

                println!("{:?}", gamesave[i]);
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
