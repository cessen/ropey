extern crate ropey;

use ropey::Rope;

const TEXT1: &str = include_str!("non_ascii.txt");

#[test]
#[allow(clippy::cmp_owned)]
#[cfg_attr(miri, ignore)]
fn non_ascii_eq() {
    // Build rope from file contents
    let rope1 = Rope::from_str(TEXT1);

    let mut rope2 = Rope::from_str(TEXT1);
    rope2.remove(1467..1827);
    for line1 in rope1.lines() {
        for line2 in rope2.lines() {
            println!("lines1: {line1} line2: {line2}");
            println!("{}", line1.to_string() == line2);
            println!("{}", line1 == line2);
        }
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn non_ascii_ord() {
    // Build rope from file contents
    let rope1 = Rope::from_str(TEXT1);

    let mut rope2 = Rope::from_str(TEXT1);
    rope2.remove(1467..1827);
    for line1 in rope1.lines() {
        for line2 in rope2.lines() {
            println!("lines1: {line1} line2: {line2}");
            println!("{:?}", line2.partial_cmp(&line1));
        }
    }
}
