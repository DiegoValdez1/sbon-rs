#![allow(unused)]

use sbon::formats::SbvjRead;
use tinyjson::JsonValue;
use std::{
    fs::{File, write},
    io::{Cursor, Seek, SeekFrom, Write},
};

fn main() {
    let save = File::open("testing/test.player")
        .unwrap()
        .read_sb_sbvj01()
        .unwrap();

    dbg!(&save.name);

    let json: JsonValue = save.data.into();

    let b = json.stringify().unwrap();

    write("testing/out", b).unwrap();
}
