use byteorder::{BigEndian, ReadBytesExt};
use serde::{Serialize, ser::SerializeSeq};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
};
use thiserror::Error;

pub mod sbasset;

#[derive(Debug, Error)]
pub enum SbonError {
    #[error("IO error while reading from file")]
    IoError {
        #[from]
        source: std::io::Error,
    },

    #[error("Overflowed value for a signed varint.")]
    InvalidSignedVarint,

    #[error("Invalid String Size")]
    InvalidStringSize,

    #[error("Invalid String Contents")]
    InvalidString {
        #[from]
        source: std::string::FromUtf8Error,
    },

    #[error("Invalid dynamic type byte: {0}. Expected 1-7 (inclusive).")]
    InvalidDynamicType(u8),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Dynamic {
    Null,
    Double(f64),
    Bool(bool),
    SignedVarint(i64),
    String(String),
    List(Vec<Dynamic>),
    Map(HashMap<String, Dynamic>),
}

impl Serialize for Dynamic {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Dynamic::Null => serializer.serialize_none(),
            Dynamic::Double(x) => serializer.serialize_f64(x.clone()),
            Dynamic::Bool(x) => serializer.serialize_bool(x.clone()),
            Dynamic::SignedVarint(x) => serializer.serialize_i64(x.clone()),
            Dynamic::String(x) => serializer.serialize_str(x),
            Dynamic::List(x) => {
                let mut seq = serializer.serialize_seq(Some(x.len()))?;

                for val in x {
                    seq.serialize_element(val)?;
                }

                seq.end()
            },
            Dynamic::Map(x) => todo!(),
        }
    }
}

impl Dynamic {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("unable to convert dynamic to json")
    }
}

#[derive(Debug)]
pub struct VersionedJson {
    pub name: String,
    pub version: Option<i32>,
    pub data: Dynamic,
}

impl VersionedJson {
    pub fn from_sbvj01<T: Read + Seek>(data: &mut T) -> Result<VersionedJson, SbonError> {
        data.seek(SeekFrom::Current(6))?; // skips magic number
        data.read_sb_vjson()
    }
}

pub trait SbReadable: Read {
    fn read_sb_varint(&mut self) -> Result<u64, SbonError> {
        let mut val: u64 = 0;
        loop {
            let byte: u8 = self.read_u8()?;
            val = val << 7 | (byte & 0b0111_1111) as u64;
            if byte & 0b1000_0000 == 0 {
                return Ok(val);
            }
        }
    }

    fn read_sb_signed_varint(&mut self) -> Result<i64, SbonError> {
        let mut val =
            i64::try_from(self.read_sb_varint()?).map_err(|_| SbonError::InvalidSignedVarint)?;
        if val & 1 != 0 {
            val = -(val >> 1) - 1
        }
        Ok(val)
    }

    fn read_sb_string(&mut self) -> Result<String, SbonError> {
        let length =
            usize::try_from(self.read_sb_varint()?).map_err(|_| SbonError::InvalidStringSize)?;

        let mut buf = vec![0u8; length];
        self.read_exact(&mut buf)?;

        Ok(String::from_utf8(buf)?)
    }

    fn read_sb_list(&mut self) -> Result<Vec<Dynamic>, SbonError> {
        let length = self.read_sb_varint()?;
        let mut list: Vec<Dynamic> = Vec::new();

        for _ in 0..length {
            list.push(self.read_sb_dynamic()?);
        }

        Ok(list)
    }

    fn read_sb_map(&mut self) -> Result<HashMap<String, Dynamic>, SbonError> {
        let length = self.read_sb_varint()?;
        let mut map: HashMap<String, Dynamic> = HashMap::new();

        for _ in 0..length {
            let key = self.read_sb_string()?;
            map.insert(key, self.read_sb_dynamic()?);
        }

        Ok(map)
    }

    fn read_sb_dynamic(&mut self) -> Result<Dynamic, SbonError> {
        let type_byte = self.read_u8()?;
        match type_byte {
            1 => Ok(Dynamic::Null),
            2 => Ok(Dynamic::Double(self.read_f64::<BigEndian>()?)),
            3 => Ok(Dynamic::Bool(self.read_u8()? != 0)),
            4 => Ok(Dynamic::SignedVarint(self.read_sb_signed_varint()?)),
            5 => Ok(Dynamic::String(self.read_sb_string()?)),
            6 => Ok(Dynamic::List(self.read_sb_list()?)),
            7 => Ok(Dynamic::Map(self.read_sb_map()?)),
            _ => Err(SbonError::InvalidDynamicType(type_byte)),
        }
    }

    fn read_sb_vjson(&mut self) -> Result<VersionedJson, SbonError> {
        let name = self.read_sb_string()?;
        let mut version = None;

        if self.read_u8()? != 0 {
            version = Some(self.read_i32::<BigEndian>()?)
        }

        let data = self.read_sb_dynamic()?;

        Ok(VersionedJson {
            name,
            version,
            data,
        })
    }
}

impl<R: Read> SbReadable for R {}
