use crate::{Dynamic, SbReadable, SbonError};
use byteorder::{BigEndian, ReadBytesExt};
use globset::Glob;
use serde::Serialize;
use std::{
    collections::HashMap,
    io::{Cursor, Read, Seek, SeekFrom}, fmt::Display,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("IO error while reading from file")]
    IoError {
        #[from]
        source: std::io::Error,
    },

    #[error("Errored while reading a SB type")]
    SbonError {
        #[from]
        source: SbonError,
    },

    #[error("Error in glob")]
    GlobError {
        #[from]
        source: globset::Error,
    },

    #[error("The information map (metadata) in the asset is invalid.")]
    InvalidInfoMap,
}

pub struct AssetFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default, Serialize)]
pub struct Info {
    pub internal_name: Option<Dynamic>,
    pub friendly_name: Option<Dynamic>,
    pub description: Option<Dynamic>,
    pub author: Option<Dynamic>,
    pub version: Option<Dynamic>,
    pub link: Option<Dynamic>,
    pub steam_id: Option<Dynamic>,
    pub tags: Option<Dynamic>,
    pub includes: Option<Dynamic>,
    pub requires: Option<Dynamic>,
    pub priority: Option<Dynamic>,
}

#[derive(Debug)]
pub struct SbAsset {
    pub info: Info,
    file_index: HashMap<String, (u64, u64)>,
    cursor: Cursor<Vec<u8>>,
}

impl SbAsset {
    pub fn new(bytes: Vec<u8>) -> Result<Self, AssetError> {
        let mut cursor = Cursor::new(bytes);
        cursor.seek(SeekFrom::Start(8))?; // skips 'SbAsset6' text at beginning

        let index_offset = cursor.read_u64::<BigEndian>()?;
        cursor.seek(SeekFrom::Start(index_offset + 5))?; // plus 5 to skip the 'INDEX' text

        let info_map = cursor.read_sb_map()?;
        let g = |name: &str| info_map.get(name).map(|x| x.to_owned());
        let info = Info {
            internal_name: g("name"),
            friendly_name: g("friendlyName"),
            description: g("description"),
            author: g("author"),
            version: g("version"),
            link: g("link"),
            steam_id: g("steamContentId"),
            tags: g("tags"),
            includes: g("includes"),
            requires: g("requires"),
            priority: g("priority"),
        };

        let mut file_index = HashMap::new();
        let index_length = cursor.read_sb_varint()?;

        for _ in 0..index_length {
            let path = cursor.read_sb_string()?;
            let offset = cursor.read_u64::<BigEndian>()?;
            let length = cursor.read_u64::<BigEndian>()?;

            file_index.insert(path, (offset, length));
        }

        Ok(SbAsset {
            info,
            file_index,
            cursor,
        })
    }

    pub fn get_file(&mut self, path: &str) -> Result<Option<AssetFile>, AssetError> {
        if let Some(info) = self.file_index.get(path) {
            self.cursor.seek(SeekFrom::Start(info.0))?;

            let mut buf = vec![0u8; usize::try_from(info.1).unwrap_or(0)]; // im too lazy sue me
            self.cursor.read_exact(&mut buf)?;

            return Ok(Some(AssetFile {
                path: path.to_string(),
                bytes: buf,
            }));
        }
        Ok(None)
    }

    /// Returns a vec of paths in this assetfile that match the given glob
    ///
    /// This should be faster than glob_read() since it doesn't have to read from the internal cursor. It also only requires an immutable reference to self.
    ///
    /// Errors if constructing the Glob struct using the given glob errors.
    pub fn glob(self, glob: &str) -> Result<Vec<String>, globset::Error> {
        let mut output = vec![];
        let matcher = Glob::new(glob)?.compile_matcher();

        for file_path in self.file_index.keys() {
            if matcher.is_match(file_path) {
                output.push(file_path.to_owned())
            }
        }

        Ok(output)
    }

    /// Returns all AssetFile containing the path and bytes which match the given glob.
    pub fn glob_read(&mut self, glob: &str) -> Result<Vec<AssetFile>, AssetError> {
        let mut output = vec![];
        let matcher = Glob::new(glob)?.compile_matcher();

        for (file_path, info) in self.file_index.iter() {
            if matcher.is_match(file_path.clone()) {
                let mut buf = vec![0u8; usize::try_from(info.1).unwrap_or(0)];
                self.cursor.read_exact(&mut buf)?;
                output.push(AssetFile {
                    path: file_path.clone(),
                    bytes: buf,
                })
            }
        }

        Ok(output)
    }
}
