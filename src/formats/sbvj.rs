use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use tinyjson::JsonValue;
use std::{io::{Read, Write}, path::Path, fs::File};

use crate::{Dynamic, SbonError, SbonRead, SbonWrite};

/// The string "SBVJ01" in bytes
const SBVJ01: &[u8] = "SBVJ01".as_bytes();

#[derive(Debug, thiserror::Error)]
pub enum SbvjError {
    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("Sbon error")]
    Sbon(#[from] SbonError),

    #[error("Invalid magic number")]
    MagicNumberError,
}

#[derive(Debug)]
/// StarBound Versioned Json.
pub struct VJson {
    pub name: String,
    pub version: Option<i32>,
    pub data: Dynamic,
}

impl From<VJson> for JsonValue {
    fn from(value: VJson) -> Self {
        value.data.into()
    }
}

pub trait SbvjRead: Read + SbonRead {
    /// Reads starbound versioned json from self.
    fn read_sb_vjson(&mut self) -> Result<VJson, SbvjError> {
        let name = self.read_sb_string()?;
        let version = if self.read_u8()? & 1 == 1 {
            Some(self.read_i32::<BigEndian>()?)
        } else {
            None
        };
        let data = self.read_sb_dynamic()?;

        Ok(VJson {
            name,
            version,
            data,
        })
    }

    /// Reads a 'SBVJ01' data from self. This is just a magic number with a `Vjson` right after it. Usually used in standalone files.
    fn read_sb_sbvj01(&mut self) -> Result<VJson, SbvjError> {
        let mut buf = [0u8; 6];
        self.read_exact(&mut buf)?;
        if buf != SBVJ01 {
            return Err(SbvjError::MagicNumberError);
        }

        self.read_sb_vjson()
    }
}

impl<R: Read + SbonRead> SbvjRead for R {}

pub trait SbvjWrite: Write + SbonWrite {
    /// Writes starbound versioned json to self.
    fn write_sb_vjson(&mut self, val: &VJson) -> Result<(), SbvjError> {
        self.write_sb_string(&val.name);

        if let Some(v) = val.version {
            self.write_u8(1)?;
            self.write_i32::<BigEndian>(v)?;
        } else {
            self.write_u8(0)?;
        }

        self.write_sb_dynamic(&val.data);
        Ok(())
    }

    /// Writes `SBVJ01` data to self. This is just a magic number with a `VJson` right after it. Usually used in standalone files.
    fn write_sb_sbvj01(&mut self, val: &VJson) -> Result<(), SbvjError> {
        self.write_all(&SBVJ01)?;
        self.write_sb_vjson(&val)
    }
}

impl<W: Write + SbonWrite> SbvjWrite for W {}

/// Convience for opening a file and reading a `SBVJ01`, which is just a `VJson`.
pub fn read_sbvj01(path: impl AsRef<Path>) -> Result<VJson, SbvjError> {
    File::open(path)?.read_sb_sbvj01()
}