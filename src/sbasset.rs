use crate::{Dynamic, SbonRead};
use byteorder::{BigEndian, ReadBytesExt};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
};

macro_rules! meta {
    (s: $rm:ident, $name:literal) => {
        if let Some(val) = $rm.remove($name) {
            if let Dynamic::String(x) = val {
                Some(x)
            } else {
                None
            }
        } else {
            None
        }
    };
    (l: $rm:ident, $name:literal) => {
        if let Some(Dynamic::List(lst)) = $rm.remove($name) {
            let out: Vec<_> = lst
                .into_iter()
                .flat_map(|d| {
                    if let Dynamic::String(s) = d {
                        Some(s)
                    } else {
                        None
                    }
                })
                .collect();
            Some(out)
        } else {
            None
        }
    };
}

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

#[derive(Debug)]
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

/// An asset reader. Reads index then dynamically reads the files inside as they are requested.
pub struct AssetReader<'a, R: Read + Seek> {
    inner: &'a mut R,

    /// HashMap< path, ( offset, length ) >
    files: HashMap<String, (u64, u64)>,

    pub meta: Metadata,
}

impl<'a, R: Read + Seek> AssetReader<'a, R> {
    /// Creates an asset reader from the supplied reader. Reads are made on creaction.
    pub fn new(inner: &'a mut R) -> AssetResult<AssetReader<'a, R>> {
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
            internal_name: meta!(s: rm, "name"),
            friendly_name: meta!(s: rm, "friendlyName"),
            author: meta!(s: rm, "author"),
            description: meta!(s: rm, "description"),
            steam_id: meta!(s: rm, "steamContentId"),
            tags: meta!(s: rm, "tags"),
            version: meta!(s: rm, "version"),
            link: meta!(s: rm, "link"),
            includes: meta!(l: rm, "includes"),
            requires: meta!(l: rm, "requires"),
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

    /// Gets a file in the asset by its path as defined by starbound.
    pub fn get_file(&mut self, path: impl ToString) -> Option<AssetResult<Vec<u8>>> {
        let path = path.to_string();

        if !self.files.contains_key(&path) {
            return None;
        }

        let info = self.files.get(&path).unwrap().clone();
        Some(self.read_file(&info))
    }

    /// Gets all the starbound defined paths of files in this asset.
    pub fn get_paths(&self) -> Vec<&String> {
        self.files.keys().collect()
    }

    /// Tests if this asset has a file present by its path.
    pub fn has_file(&self, path: impl ToString) -> bool {
        self.files.contains_key(&path.to_string())
    }
}

impl<'a, R: Read + Seek> IntoIterator for AssetReader<'a, R> {
    type Item = (String, Vec<u8>);
    type IntoIter = AssetIter<'a, R>;

    fn into_iter(self) -> Self::IntoIter {
        let AssetReader { inner, files, .. } = self;

        AssetIter {
            inner,
            files: files.into_iter(),
        }
    }
}

/// An iterator which reads the asset file's path and bytes from the inner readable.
pub struct AssetIter<'a, R: Read + Seek> {
    inner: &'a mut R,
    files: std::collections::hash_map::IntoIter<String, (u64, u64)>,
}

impl<'a, R: Read + Seek> Iterator for AssetIter<'a, R> {
    type Item = (String, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        let (path, (offset, length)) = self.files.next()?;

        let mut buf = vec![0u8; length as usize];
        self.inner.seek(SeekFrom::Start(offset)).ok()?;
        self.inner.read_exact(&mut buf).ok()?;

        Some((path, buf))
    }
}
