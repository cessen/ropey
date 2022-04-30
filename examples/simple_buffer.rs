#![allow(clippy::redundant_field_names)]
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

    fn get_line(&self, idx: usize) -> RopeSlice {
        self.text.line(idx)
    }

    fn bytes(&self) -> Bytes {
        self.text.bytes()
    }

    fn chars(&self) -> Chars {
        self.text.chars()
    }

    fn lines(&self) -> Lines {
        self.text.lines()
    }

    fn chunks(&self) -> Chunks {
        self.text.chunks()
    }

    fn edit(&mut self, start: usize, end: usize, text: &str) {
        if start != end {
            self.text.remove(start..end);
        }
        if !text.is_empty() {
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
