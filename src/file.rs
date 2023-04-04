use std::fmt::Display;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::{BufReader, Read};
use bitflags::bitflags;

use crate::message::Message;

#[derive(Debug)]
pub enum CompassFileError {
    IOError(PathBuf, std::io::Error),
    WavesError(PathBuf)
}

impl Display for CompassFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IOError(name, error) => write!(f, "File {} ran into an error: {}", name.display(), error),
            Self::WavesError(name) => write!(f, "File {} was detected to be in waves mode!", name.display())
        }
    }
}

impl std::error::Error for CompassFileError {

}

bitflags! {
    #[derive(Debug, Clone)]
    pub struct CompassDataType: u16 {
        const ENERGY = 0x0001;
        const ENERGY_SHORT = 0x0004;
        const ENERGY_CALIBRATED = 0x0002;
        const WAVES = 0x0008;
        const ALL = Self::ENERGY.bits() | Self::ENERGY_SHORT.bits() | Self::ENERGY_CALIBRATED.bits() | Self::WAVES.bits();
        const NONE = 0x0000;
    }
}

/*
    Simple representation of a CoMPASS binary data file. 
 */

#[derive(Debug)]
pub struct CompassFile {
    filepath: PathBuf,
    handle: BufReader<File>,
    data_type: u16,
    hit_size: usize
}

impl CompassFile {
    pub fn new(path: &Path) -> Result<CompassFile, CompassFileError> {
        let mut file: File = match File::open(path) {
            Ok(f) => f,
            Err(e) => return Err(CompassFileError::IOError(path.to_path_buf(), e))
        };

        //Read the header
        let mut header:[u8; 2] = [0; 2];
        match file.read_exact(&mut header) {
            Ok(_) => {},
            Err(e) => return Err(CompassFileError::IOError(path.to_path_buf(), e))
        };
        let header_word = u16::from_le_bytes(header);

        let mut datatype = CompassDataType::NONE;
        let mut datasize: usize = 16; //minimum 16 bytes for board, channel, timestamp, flags

        //determine the header type and data size.
        if header_word & CompassDataType::ENERGY.bits() != 0 {
            datatype |= CompassDataType::ENERGY;
            datasize += 2;
        }
        if header_word & CompassDataType::ENERGY_SHORT.bits() != 0 {
            datatype |= CompassDataType::ENERGY_SHORT;
            datasize += 2;
        }
        if header_word & CompassDataType::ENERGY_CALIBRATED.bits() != 0 {
            datatype |= CompassDataType::ENERGY_CALIBRATED;
            datasize += 8;
        }
        if header_word & CompassDataType::WAVES.bits() != 0 {
            return Err(CompassFileError::WavesError(path.to_path_buf()));
        }
        

        return Ok(CompassFile {
            filepath: path.to_path_buf(),
            handle: BufReader::new(file),
            data_type: header_word,
            hit_size: datasize,
        });

    }

    //Read data from the file and make a Message
    pub fn read_data(&mut self) -> Result<Message, CompassFileError> {
        let mut message = Message::default();
        message.data_type = self.data_type;
        message.hit_size = self.hit_size as u64;
        match self.handle.read_to_end(&mut message.data) {
            Ok(size) => message.size += size as u64,
            Err(e) => return Err(CompassFileError::IOError(self.filepath.clone(), e))
        };
        return Ok(message);
    }
}