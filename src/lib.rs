#![allow(unused)]

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::{
    collections::HashMap,
    io::{Read, Write},
};

pub mod formats;

macro_rules! impl_try_from_dynamic {
    ($($dyn_type:ident = $wanted_type:ty),*) => {
        $(
            impl TryFrom<Dynamic> for $wanted_type {
                type Error = TryFromDynamicError;

                fn try_from(value: Dynamic) -> Result<Self, Self::Error> {
                    match value {
                        Dynamic::$dyn_type(val) => Ok(val),
                        _ => Err(TryFromDynamicError::Invalid)
                    }
                }
            }
        )*
    };
}

pub type List = Vec<Dynamic>;
pub type Map = HashMap<String, Dynamic>;

#[derive(Debug, thiserror::Error)]
pub enum TryFromDynamicError {
    #[error("The provided dynamic cannot be unwrapped to the wanted type.")]
    Invalid
}

/// The starbound dynamic type.
#[derive(Debug)]
pub enum Dynamic {
    Nil,
    Double(f64),
    Boolean(bool),
    VlqI(i64),
    String(String),
    List(List),
    Map(Map),
}

impl Dynamic {
    /// Attempts to unwrap this dynamic to its inner value. Since each enum variant is of a different type,
    /// this fn returns a Option of the type specified. If the dynamic does not match they type specified,
    /// it returns None.
    pub fn cast<T: TryFrom<Dynamic>>(self) -> Option<T> {
        T::try_from(self).ok()
    }
}

impl_try_from_dynamic! {
    Double = f64,
    Boolean = bool,
    VlqI = i64,
    String = String,
    List = List,
    Map = Map
}

#[derive(Debug, thiserror::Error)]
pub enum SbonError {
    #[error("IO error while reading/writing SBON")]
    Io(#[from] std::io::Error),

    #[error("Invalid dynamic type byte. Expected 1.=7, got '{0}'")]
    DynamicTypeError(u8),

    #[error("Invalid UTF-8 string encountered while reading SBON")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Max bytes read for VLQU is 10, while the VLQU wants to read over 10.")]
    OversizedVLQ
}

/// Functions to read starbound encoded types from `self`, and returning their rust value.
pub trait SbonRead: Read {
    /// Reads starbound encoded unsigned 'VLQ' from the reader.
    fn read_sb_vlqu(&mut self) -> Result<u64, SbonError> {
        let mut val: u64 = 0;
        for _ in 0..10 {
            let byte: u8 = self.read_u8()?;
            val = val << 7 | (byte & 0b0111_1111) as u64;
            if byte & 0b1000_0000 == 0 {
                return Ok(val);
            }
        }
        return Err(SbonError::OversizedVLQ)
    }

    /// Reads starbound encoded signed 'VLQ' from the reader.
    fn read_sb_vlqi(&mut self) -> Result<i64, SbonError> {
        let val = self.read_sb_vlqu()?;

        if val & 0b1 != 0 {
            return Ok((-((val >> 1) as i64)  - 1))
        }

        Ok((val >> 1) as i64)
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

/// Functions to write starbound primitive values to anything that implements write.
pub trait SbonWrite: Write {
    /// Writes a starbound encoded unsigned 'VLQ' to the writer.
    fn write_sb_vlqu(&mut self, mut val: u64) -> Result<(), SbonError> {
        let mut buf = Vec::new();

        buf.push((val & 0b1111111) as u8);
        val >>= 7;

        while val != 0 {
            buf.push((val & 0b1111111 | 0b10000000) as u8);
            val >>= 7;
        }

        buf.reverse();
        self.write_all(&buf)?;
        Ok(())
    }

    /// Writes a starbound encoded signed 'VLQ' to the writer.
    fn write_sb_vlqi(&mut self, val: i64) -> Result<(), SbonError> {
        self.write_sb_vlqu(match val {
            x if x < 0 => ((x + 1).abs() << 1 | 1) as u64,
            x => (x << 1) as u64,
        })
    }

    /// Writes starbound encoded 'bytes' to the writer.
    fn write_sb_bytes(&mut self, val: &[u8]) -> Result<(), SbonError> {
        self.write_sb_vlqu(val.len() as u64)?;
        self.write_all(val)?;
        Ok(())
    }

    /// Writes a starbound encoded 'string' to the writer.
    fn write_sb_string(&mut self, val: &str) -> Result<(), SbonError> {
        self.write_sb_bytes(val.as_bytes())
    }

    /// Writes a starbound encoded 'list' to the writer.
    fn write_sb_list(&mut self, val: &List) -> Result<(), SbonError> {
        self.write_sb_vlqu(val.len() as u64)?;

        for d in val {
            self.write_sb_dynamic(d)?;
        }

        Ok(())
    }

    /// Writes a starbound encoded 'map' to the writer.
    fn write_sb_map(&mut self, val: &Map) -> Result<(), SbonError> {
        self.write_sb_vlqu(val.len() as u64)?;

        for (key, dynamic) in val {
            self.write_sb_string(&key)?;
            self.write_sb_dynamic(&dynamic)?;
        }

        Ok(())
    }

    /// Writes a starbound encoded 'dynamic' to the writer.
    fn write_sb_dynamic(&mut self, val: &Dynamic) -> Result<(), SbonError> {
        match val {
            Dynamic::Nil => self.write_u8(1)?,
            Dynamic::Double(x) => {
                self.write_u8(2)?;
                self.write_f64::<BigEndian>(*x)?;
            }
            Dynamic::Boolean(x) => {
                self.write_u8(3)?;
                self.write_u8((*x) as u8)?;
            }
            Dynamic::VlqI(x) => {
                self.write_u8(4)?;
                self.write_sb_vlqi(*x)?;
            }
            Dynamic::String(x) => {
                self.write_u8(5)?;
                self.write_sb_string(x)?;
            }
            Dynamic::List(x) => {
                self.write_u8(6)?;
                self.write_sb_list(x)?;
            }
            Dynamic::Map(x) => {
                self.write_u8(7)?;
                self.write_sb_map(x)?;
            }
        }

        Ok(())
    }
}

impl<W: Write> SbonWrite for W {}
