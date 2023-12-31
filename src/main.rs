#![allow(unused)]

use std::fs::File;
use sbon::sbasset::AssetReader;

fn main() {
    let mut f = File::open("testing/contents.pak").unwrap();
    let mut a = AssetReader::new(&mut f).unwrap();

    dbg!(a.meta);
}
