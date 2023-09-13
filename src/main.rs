#![allow(unused)]

use sbon::formats::Asset;

fn main() {
    let fu = Asset::open("dev/fu.pak").unwrap();

    dbg!(fu.meta);
}
