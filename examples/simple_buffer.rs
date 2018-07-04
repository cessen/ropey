#![allow(dead_code)]

extern crate ropey;

use std::fs::File;
use std::io;

use ropey::iter::{Bytes, Chars, Chunks, Lines};
use ropey::{Rope, RopeSlice};

struct TextBuffer {
    text: Rope,
    path: String,
    dirty: bool,
}

impl TextBuffer {
    fn from_path(path: &str) -> io::Result<TextBuffer> {
        let text = Rope::from_reader(&mut io::BufReader::new(File::open(&path)?))?;
        Ok(TextBuffer {
            text: text,
            path: path.to_string(),
            dirty: false,
        })
    }

    fn get_line<'a>(&'a self, idx: usize) -> RopeSlice<'a> {
        self.text.line(idx)
    }

    fn bytes<'a>(&'a self) -> Bytes<'a> {
        self.text.bytes()
    }

    fn chars<'a>(&'a self) -> Chars<'a> {
        self.text.chars()
    }

    fn lines<'a>(&'a self) -> Lines<'a> {
        self.text.lines()
    }

    fn chunks<'a>(&'a self) -> Chunks<'a> {
        self.text.chunks()
    }

    fn edit(&mut self, start: usize, end: usize, text: &str) {
        if start != end {
            self.text.remove(start..end);
        }
        if text.len() > 0 {
            self.text.insert(start, text);
        }
        self.dirty = true;
    }
}

fn main() {
    // Get filepath from commandline
    let filepath = if std::env::args().count() > 1 {
        std::env::args().nth(1).unwrap()
    } else {
        println!(
            "You must pass a filepath!  Only recieved {} arguments.",
            std::env::args().count()
        );
        panic!()
    };

    let mut buf = TextBuffer::from_path(&filepath).unwrap();

    buf.edit(3, 5, "Hello!");
    println!("{}", buf.get_line(2));
}
