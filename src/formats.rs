use crate::{Dynamic, Map, SbonRead};
use anyhow::{bail, Result};
use byteorder::{BigEndian, ReadBytesExt};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::PathBuf,
};

/// A struct representing a starbound asset file called 'SBAsset6'. Most commonly seen as mods.
pub struct Asset {
    /// Metadata about the asset.
    pub meta: Metadata,

    /// Every file entry found in this asset.
    pub files: Vec<AssetEntry>,
}

impl Asset {
    /// Parses the given reader as a starbound asset file.
    pub fn read_from<R: Read + Seek>(mut reader: R) -> Result<Self> {
        // Magic Number validation. Hex number is 'SBAsset6' in hex to easily read it in one line.
        if reader.read_u64::<BigEndian>()? != 0x53_42_41_73_73_65_74_36 {
            bail!("Invalid magic number for starbound asset.")
        }

        // Reads index. Adds 5 to offset to skip the string 'INDEX'.
        let offset = reader.read_u64::<BigEndian>()?;
        reader.seek(SeekFrom::Start(offset + 5));
        let raw_meta = reader.read_map()?;
        let num_files = reader.read_vlqu()?;

        // Reads file locations from index
        let mut file_locations = vec![];

        for _ in 0..num_files {
            file_locations.push((
                reader.read_string()?,           // Path
                reader.read_u64::<BigEndian>()?, // Offset
                reader.read_u64::<BigEndian>()?, // Length
            ))
        }

        // Actually seeks to and reads the file bytes from the asset
        let mut files = vec![];
        for location in file_locations {
            reader.seek(SeekFrom::Start(location.1))?;

            let mut buf = vec![0u8; location.2 as usize];
            reader.read_exact(&mut buf)?;

            files.push(AssetEntry {
                path: location.0,
                data: buf,
            });
        }

        Ok(Self {
            meta: raw_meta.into(),
            files,
        })
    }

    /// Opens and parses the given file path into a starbound asset file. Uses a BufReader.
    pub fn open<P: Into<PathBuf>>(path: P) -> Result<Self> {
        let mut file = File::open(path.into())?;
        Self::read_from(BufReader::new(file))
    }
}

/// Contains information about a starbound asset file. Nothing here is required therefore they are all options.
#[derive(Debug)]
pub struct Metadata {
    /// Internal starbound mod id.
    pub internal_name: Option<Dynamic>,

    /// Public mod display name.
    pub friendly_name: Option<Dynamic>,
}

impl From<HashMap<String, Dynamic>> for Metadata {
    fn from(mut value: HashMap<String, Dynamic>) -> Self {
        Self {
            internal_name: value.remove("name"),
            friendly_name: value.remove("friendlyName"),
        }
    }
}

/// A raw file entry in a starbound asset.
pub struct AssetEntry {
    /// Path of the file in the asset.
    pub path: String,

    /// Bytes of the file.
    pub data: Vec<u8>,
}

/// Starbound versioned json. Usually used with player files.
pub struct VJson {
    name: String,
    version: Option<i32>,
    data: Dynamic,
}

impl VJson {
    /// Reads a versioned json file from the reader to a dynamic map.
    pub fn read_vjson<R: Read>(mut reader: R) -> Result<Self> {
        let name = reader.read_string()?;
        let mut version = None;
        if reader.read_u8()? == 1 {
            version = Some(reader.read_i32::<BigEndian>()?)
        }
        let data = reader.read_dynamic()?;

        Ok(Self {
            name,
            version,
            data,
        })
    }

    /// Opens a versioned json file and parses it into a dynamic map.
    pub fn open_vjson<P: Into<PathBuf>>(path: P) -> Result<Self> {
        let f = File::open(path.into())?;
        Self::read_vjson(f)
    }
}
