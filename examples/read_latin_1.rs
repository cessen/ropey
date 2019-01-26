//! Example of decoding from another text encoding on-the-fly while reading.
//! In this case, we're decoding from ISO/IEC 8859-1, which conveniently
//! happens to map 1-to-1 to the first 256 unicode scalar values.

extern crate ropey;

use std::fs::File;
use std::io;
use std::io::Read;

use ropey::RopeBuilder;

fn main() {
    // Get filepath from commandline
    let filepath = if std::env::args().count() > 1 {
        std::env::args().nth(1).unwrap()
    } else {
        eprintln!(
            "You must pass a filepath!  Only recieved {} arguments.",
            std::env::args().count()
        );
        panic!()
    };

    // Get everything set up to begin reading and decoding.
    let mut buf = vec![0u8; 1 << 14]; // Buffer for raw bytes.
    let mut buf_str = String::with_capacity(1 << 14); // Buffer for decoded utf8.
    let mut builder = RopeBuilder::new();
    let mut file = io::BufReader::new(File::open(&filepath).unwrap());

    // Read the data in chunks, decoding and appending to the rope builder
    // as we go.
    // (Note: in real code you should handle errors from the reader!)
    while let Ok(n) = file.read(&mut buf) {
        if n == 0 {
            break;
        }

        // Decode and append the chunk to the rope builder.
        buf_str.clear();
        for &byte in &buf[..n] {
            buf_str.push(byte as char);
        }
        builder.append(&buf_str);
    }

    // Build rope.
    let _rope = builder.finish();
}
