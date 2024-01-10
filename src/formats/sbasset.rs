use crate::{Dynamic, List, Map, SbonRead};
use byteorder::{BigEndian, ReadBytesExt};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

// Utility
// ====================================================================================================================

fn get<T: TryFrom<Dynamic>>(rm: &mut Map, tag: &'static str) -> Option<T> {
    rm.remove(tag)?.cast()
}

fn get_list<T: TryFrom<Dynamic>>(rm: &mut Map, tag: &'static str) -> Option<Vec<T>> {
    rm.remove(tag)?
        .cast::<List>()?
        .into_iter()
        .map(|d| d.cast())
        .collect()
}

// Error
// ====================================================================================================================
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("IO Error while reading from asset")]
    Io(#[from] std::io::Error),

    #[error("Error while reading 'SBON' types.")]
    Sbon(#[from] crate::SbonError),

    #[error("Invalid asset read from reader. Error details: {0}")]
    InvalidAsset(&'static str),
}

type AssetResult<T> = Result<T, AssetError>;

// Pre-Items
// ====================================================================================================================

pub struct SbPath(PathBuf);

impl SbPath {}

/// A struct which contains a file read from a starbound asset.
pub struct AssetFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

impl AssetFile {
    /// Writes this file to the path supplied.
    pub fn export(&self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        File::create(path)?.write_all(&self.bytes)
    }
}

/// Information about an asset
#[derive(Debug, Default, Clone)]
pub struct Metadata {
    pub internal_name: Option<String>,
    pub friendly_name: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub steam_id: Option<String>,
    pub tags: Option<String>,
    pub version: Option<String>,
    pub link: Option<String>,
    pub includes: Option<Vec<String>>,
    pub requires: Option<Vec<String>>,
}

// Main items
// ====================================================================================================================
/// An owned version of an Asset which holds all of the files it contains in memory
#[derive(Default)]
pub struct Asset {
    pub meta: Metadata,
    pub files: Vec<AssetFile>,
}

impl Asset {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self) {}
}

/// An asset reader. Reads index then dynamically reads the files inside as they are requested.
pub struct AssetReader<R: Read + Seek> {
    inner: R,

    /// HashMap< path, ( offset, length ) >
    files: HashMap<String, (u64, u64)>,

    pub meta: Metadata,
}

impl<R: Read + Seek> AssetReader<R> {
    /// Creates an asset reader from the supplied reader. Reads are made on creaction.
    pub fn new(mut inner: R) -> AssetResult<AssetReader<R>> {
        // Ensures the starting 'SBAsset6' string in bytes. That number is that string as hex.
        if inner.read_u64::<BigEndian>()? != 0x5342417373657436 {
            return Err(AssetError::InvalidAsset("Invalid magic number"));
        }

        let index_offset = inner.read_u64::<BigEndian>()?;
        inner.seek(SeekFrom::Start(index_offset))?;

        // Ensures the string 'INDEX' is at start of index
        let mut buf = [0u8; 5];
        inner.read_exact(&mut buf)?;
        if &buf == "".as_bytes() {
            return Err(AssetError::InvalidAsset("Invalid index"));
        }

        // Read and parse the metadata
        let mut rm = inner.read_sb_map()?;
        let meta = Metadata {
            internal_name: get(&mut rm, "name"),
            friendly_name: get(&mut rm, "friendlyName"),
            author: get(&mut rm, "author"),
            description: get(&mut rm, "description"),
            steam_id: get(&mut rm, "steamContentId"),
            tags: get(&mut rm, "tags"),
            version: get(&mut rm, "version"),
            link: get(&mut rm, "link"),
            includes: get_list(&mut rm, "includes"),
            requires: get_list(&mut rm, "requires"),
        };

        // Read the files
        let mut files = HashMap::new();
        for _ in 0..inner.read_sb_vlqu()? {
            files.insert(
                inner.read_sb_string()?, // Path
                (
                    inner.read_u64::<BigEndian>()?, // Offset
                    inner.read_u64::<BigEndian>()?, // Length
                ),
            );
        }

        Ok(AssetReader { inner, files, meta })
    }

    /// Reads a file from the inner reader with the supplied (offset, length).
    fn read_file(&mut self, info: &(u64, u64)) -> AssetResult<Vec<u8>> {
        let mut out = vec![0u8; info.1 as usize];
        self.inner.seek(SeekFrom::Start(info.0))?;
        self.inner.read_exact(&mut out)?;
        Ok(out)
    }

    /// Gets a file's bytes in the asset by its path as defined by starbound.
    pub fn get_file(&mut self, path: impl ToString) -> Option<AssetResult<AssetFile>> {
        let path = path.to_string();
        if !self.files.contains_key(&path) {
            return None;
        }

        let info = self.files.get(&path).unwrap().clone();
        Some(self.read_file(&info).map(|bytes| AssetFile { path, bytes }))
    }

    /// Gets all the starbound defined paths of files in this asset.
    pub fn get_paths(&self) -> Vec<&String> {
        self.files.keys().collect()
    }

    /// Tests if this asset has a file present by its path.
    pub fn has_file(&self, path: &String) -> bool {
        self.files.contains_key(path)
    }

    /// Converts this `AssetReader` to an `Asset`, which owns all of its paths.
    pub fn to_asset(self) -> AssetResult<Asset> {
        let meta = self.meta.clone();
        let mut files = Vec::new();
        for file in self.into_iter() {
            files.push(file?);
        }
        Ok(Asset { meta, files })
    }
}

impl<R: Read + Seek> IntoIterator for AssetReader<R> {
    type Item = AssetResult<AssetFile>;
    type IntoIter = AssetReadIter<R>;

    fn into_iter(self) -> Self::IntoIter {
        let AssetReader { inner, files, .. } = self;

        AssetReadIter {
            inner,
            files: files.into_iter(),
        }
    }
}

/// An iterator which reads the asset file's path and bytes from the inner readable.
pub struct AssetReadIter<R: Read + Seek> {
    inner: R,
    files: std::collections::hash_map::IntoIter<String, (u64, u64)>,
}

impl<R: Read + Seek> Iterator for AssetReadIter<R> {
    type Item = AssetResult<AssetFile>;

    fn next(&mut self) -> Option<Self::Item> {
        let (path, (offset, length)) = self.files.next()?;

        let mut buf = vec![0u8; length as usize];
        if let Err(e) = self
            .inner
            .seek(SeekFrom::Start(offset))
            .map_err(|e| AssetError::from(e))
        {
            return Some(Err(e));
        }
        if let Err(e) = self
            .inner
            .read_exact(&mut buf)
            .map_err(|e| AssetError::from(e))
        {
            return Some(Err(e));
        }

        Some(Ok(AssetFile { path, bytes: buf }))
    }
}
