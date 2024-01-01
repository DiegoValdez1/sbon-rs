use crate::{Dynamic, SbonRead};
use byteorder::{BigEndian, ReadBytesExt};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
};

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

pub struct AssetFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

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
        let raw_meta = inner.read_sb_map()?;
        let get = |tag: &str| -> Option<String> {
            if let Some(d) = raw_meta.get(&tag.to_string()) {
                if let Dynamic::String(s) = d {
                    return Some(s.clone());
                }
            }

            None
        };
        let get_list = |tag: &str| -> Option<Vec<String>> {
            if let Some(lst_dyn) = raw_meta.get(&tag.to_string()) {
                if let Dynamic::List(lst) = lst_dyn {
                    return Some(
                        lst.into_iter()
                            .flat_map(|d| match d {
                                Dynamic::String(s) => Some(s),
                                _ => None,
                            })
                            .cloned()
                            .collect(),
                    );
                }
            }

            None
        };
        let meta = Metadata {
            internal_name: get("name"),
            friendly_name: get("friendlyName"),
            author: get("author"),
            description: get("description"),
            steam_id: get("steamContentId"),
            tags: get("tags"),
            version: get("version"),
            link: get("link"),
            includes: get_list("includes"),
            requires: get_list("requires"),
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
}

impl<R: Read + Seek> IntoIterator for AssetReader<R> {
    type Item = AssetResult<AssetFile>;
    type IntoIter = AssetIter<R>;

    fn into_iter(self) -> Self::IntoIter {
        let AssetReader { inner, files, .. } = self;

        AssetIter {
            inner,
            files: files.into_iter(),
        }
    }
}

/// An iterator which reads the asset file's path and bytes from the inner readable.
pub struct AssetIter<R: Read + Seek> {
    inner: R,
    files: std::collections::hash_map::IntoIter<String, (u64, u64)>,
}

impl<R: Read + Seek> Iterator for AssetIter<R> {
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
