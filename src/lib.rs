#![allow(unused)]

use byteorder::{BigEndian, ReadBytesExt};
use std::{
    collections::HashMap,
    io::{Read, Write},
};

pub mod sbasset;

#[derive(Debug, thiserror::Error)]
pub enum SbonError {
    #[error("IO error while reading/writing SBON")]
    Io(#[from] std::io::Error),

    #[error("Invalid dynamic type byte. Expected 1.=7, got '{0}'")]
    DynamicTypeError(u8),

    #[error("Invalid UTF-8 string encountered while reading SBON")]
    Utf8(#[from] std::string::FromUtf8Error),
}

pub type List = Vec<Dynamic>;
pub type Map = HashMap<String, Dynamic>;

pub enum Dynamic {
    Nil,
    Double(f64),
    Boolean(bool),
    VlqI(i64),
    String(String),
    List(List),
    Map(Map),
}

pub trait SbonRead: Read {
    /// Reads starbound encoded un-signed 'VLQ' from the reader.
    fn read_sb_vlqu(&mut self) -> Result<u64, SbonError> {
        let mut val: u64 = 0;
        loop {
            let byte: u8 = self.read_u8()?;
            val = val << 7 | (byte & 0b0111_1111) as u64;
            if byte & 0b1000_0000 == 0 {
                return Ok(val);
            }
        }
    }

    /// Reads starbound encoded signed 'VLQ' from the reader.
    fn read_sb_vlqi(&mut self) -> Result<i64, SbonError> {
        let mut val = i64::try_from(self.read_sb_vlqu()?).unwrap();
        if val & 1 != 0 {
            val = -(val >> 1) - 1
        }
        Ok(val)
    }

    /// Reads starbound encoded 'bytes' from the reader.
    fn read_sb_bytes(&mut self) -> Result<Vec<u8>, SbonError> {
        let length = self.read_sb_vlqu()?;
        let mut buf = vec![0u8; length as usize];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Reads a starbound encoded 'string' from the reader.
    fn read_sb_string(&mut self) -> Result<String, SbonError> {
        let bytes = self.read_sb_bytes()?;
        Ok(String::from_utf8(bytes)?)
    }

    /// Reads a starbound encoded 'list' from the reader.
    fn read_sb_list(&mut self) -> Result<List, SbonError> {
        let mut out = Vec::new();
        let count = self.read_sb_vlqu()?;

        for _ in 0..count {
            out.push(self.read_sb_dynamic()?)
        }

        Ok(out)
    }

    /// Reads a starbound encoded 'map' from the reader.
    fn read_sb_map(&mut self) -> Result<Map, SbonError> {
        let mut out = HashMap::new();
        let count = self.read_sb_vlqu()?;

        for _ in 0..count {
            out.insert(self.read_sb_string()?, self.read_sb_dynamic()?);
        }

        Ok(out)
    }

    /// Reads a starbound encoded 'dynamic' from the reader.
    fn read_sb_dynamic(&mut self) -> Result<Dynamic, SbonError> {
        Ok(match self.read_u8()? {
            1 => Dynamic::Nil,
            2 => Dynamic::Double(self.read_f64::<BigEndian>()?),
            3 => Dynamic::Boolean(self.read_u8()? != 0),
            4 => Dynamic::VlqI(self.read_sb_vlqi()?),
            5 => Dynamic::String(self.read_sb_string()?),
            6 => Dynamic::List(self.read_sb_list()?),
            7 => Dynamic::Map(self.read_sb_map()?),
            invalid => return Err(SbonError::DynamicTypeError(invalid)),
        })
    }
}

impl<R: Read> SbonRead for R {}

pub trait SbonWrite: Write {
    fn write_sb_vlqu(&mut self, val: u64) -> Result<(), SbonError> {
        todo!()
    }

    fn write_sb_vlqi(&mut self, val: i64) -> Result<(), SbonError> {
        todo!()
    }

    fn write_sb_bytes(&mut self, val: &[u8]) -> Result<(), SbonError> {
        todo!()
    }

    fn write_sb_string(&mut self, val: &str) -> Result<(), SbonError> {
        todo!()
    }

    fn write_sb_list(&mut self, val: List) -> Result<(), SbonError> {
        todo!()
    }

    fn write_sb_map(&mut self, val: Map) -> Result<(), SbonError> {
        todo!()
    }

    fn write_sb_dynamic(&mut self, val: Dynamic) -> Result<(), SbonError> {
        todo!()
    }
}

impl<W: Write> SbonWrite for W {}
