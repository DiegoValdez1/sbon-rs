pub mod sbasset;

use byteorder::{BigEndian, ReadBytesExt};
use std::{
    collections::HashMap,
    fmt::Display,
    io::{Read, Seek, SeekFrom},
};
use thiserror::Error;

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

// impl Display for Dynamic {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::Null => write!(f, "null")?,
//             Self::Double(v) => write!(f, "{v}")?,
//             Self::Bool(v) => write!(f, "{v}")?,
//             Self::SignedVarint(v) => write!(f, "{v}")?,
//             Self::String(v) => write!(f, "{v}")?,
//             Self::List(v) => write!(f, "{:?}", v.iter().map(|x| x.to_string()).collect::<Vec<_>>())?,
//             Self::Map(val) => {
//                 let mut output = String::from("{");
//                 let mut iter = 0;

//                 for (i, v) in val {
//                     output.push_str(i);
//                     output.push_str(": ");
//                     output.push_str(&v.to_string());
//                     if iter > val.len()-1 {
//                         output.push_str(", ");
//                         iter += 1;
//                     }
//                 }
//                 output.push_str("}");

//                 write!(f, "{output}")?;
//             }
//         }
//         Ok(())
//     }
// }

// impl ToString for Dynamic {
//     fn to_string(&self) -> String {
//         // I don't want to hear it
//         match self {
//             Self::Nil => "null".to_string(),
//             Self::Double(x) => x.to_string(),
//             Self::Bool(x) => x.to_string(),
//             Self::SignedVarint(x) => x.to_string(),
//             Self::String(x) => x.to_string(),
//             Self::List(x) => {
//                 format!("{:?}", x.iter().map(|val| val.to_string()).collect::<Vec<_>>())
//             }
//             _ => todo!()
//         }
//     }
// }

// impl ToString for Dynamic {
//     // there should be a better way to do this but im too lazy to find out
//     fn to_string(&self) -> String {
//         match self {
//             Dynamic::Nil => "null".to_string(),
// Dynamic::Double(x) => x.to_string(),
// Dynamic::Bool(x) => x.to_string(),
// Dynamic::SignedVarint(x) => x.to_string(),
// Dynamic::String(x) => x.clone(),
// Dynamic::List(x) => {
//                 let mut s = String::from("[");
//                 for i in 0..x.len() {
//                     let val = x.get(i).unwrap();
//                     if val.quoted() {
//                         s.push_str(&format!("\"{}\"", val.to_string()))
//                     } else {
//                         s.push_str(&val.to_string())
//                     }
//                     s.push_str(&x.get(i).unwrap().to_string());
//                     if i <= x.len()-1 {
//                         s.push_str(", ")
//                     }
//                 }
//                 s.push_str("]");
//                 s
//             },
//             Dynamic::Map(x) => {
//                 let mut s = String::from("{");
//                 let mut iter = 0;

//                 for (k, v) in x {
//                     iter += 1;
//                     s.push_str(&format!("\"{}\": {}", k, if v.quoted() {format!("\"{}\"", v.to_string())} else {v.to_string()}));
//                     if iter <= x.len()-1 {
//                         s.push_str(", ")
//                     }
//                 }
//                 s.push_str("}");
//                 s
//             }
//         }
//     }
// }

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
