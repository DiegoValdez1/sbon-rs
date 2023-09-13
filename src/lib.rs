#![allow(unused)]

use anyhow::{bail, Result};
use byteorder::{BigEndian, ReadBytesExt};
use std::{collections::HashMap, io::Read};

pub mod formats;

type List = Vec<Dynamic>;
type Map = HashMap<String, Dynamic>;

#[derive(Debug)]
pub enum Dynamic {
    Null,
    Double(f64),
    Bool(bool),
    Vlqi(i64),
    String(String),
    List(List),
    Map(Map),
}

pub trait SbonRead: Read {
    fn read_vlqu(&mut self) -> Result<u64> {
        let mut val: u64 = 0;
        loop {
            let byte: u8 = self.read_u8()?;
            val = val << 7 | (byte & 0b0111_1111) as u64;
            if byte & 0b1000_0000 == 0 {
                return Ok(val);
            }
        }
    }

    fn read_vlqi(&mut self) -> Result<i64> {
        let mut val = i64::try_from(self.read_vlqu()?)?;
        if val & 1 != 0 {
            val = -(val >> 1) - 1
        }
        Ok(val)
    }

    fn read_string(&mut self) -> Result<String> {
        let length = usize::try_from(self.read_vlqu()?)?;

        let mut buf = vec![0u8; length];
        self.read_exact(&mut buf)?;

        Ok(String::from_utf8(buf)?)
    }

    fn read_list(&mut self) -> Result<Vec<Dynamic>> {
        let length = self.read_vlqu()?;
        let mut list: Vec<Dynamic> = Vec::new();

        for _ in 0..length {
            list.push(self.read_dynamic()?);
        }

        Ok(list)
    }

    fn read_map(&mut self) -> Result<HashMap<String, Dynamic>> {
        let length = self.read_vlqu()?;
        let mut map: HashMap<String, Dynamic> = HashMap::new();

        for _ in 0..length {
            let key = self.read_string()?;
            map.insert(key, self.read_dynamic()?);
        }

        Ok(map)
    }

    fn read_dynamic(&mut self) -> Result<Dynamic> {
        let type_byte = self.read_u8()?;
        match type_byte {
            1 => Ok(Dynamic::Null),
            2 => Ok(Dynamic::Double(self.read_f64::<BigEndian>()?)),
            3 => Ok(Dynamic::Bool(self.read_u8()? != 0)),
            4 => Ok(Dynamic::Vlqi(self.read_vlqi()?)),
            5 => Ok(Dynamic::String(self.read_string()?)),
            6 => Ok(Dynamic::List(self.read_list()?)),
            7 => Ok(Dynamic::Map(self.read_map()?)),
            _ => bail!("Invalid dynamic type byte"),
        }
    }
}

impl<R: Read> SbonRead for R {}
