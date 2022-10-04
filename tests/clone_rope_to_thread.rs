extern crate ropey;

use std::sync::mpsc;
use std::thread;

use std::iter::Iterator;

use ropey::Rope;

const TEXT: &str = include_str!("test_text.txt");

#[test]
#[cfg_attr(miri, ignore)]
fn clone_rope_to_thread() {
    let mut rope1 = Rope::from_str(TEXT);
    let rope2 = rope1.clone();

    // Spawn a thread for modifying the clone
    let (tx1, rx1) = mpsc::channel::<Rope>();
    let (tx2, rx2) = mpsc::channel::<Rope>();
    thread::spawn(move || {
        // Modify rope2
        let mut rope = rx1.recv().unwrap();
        rope.insert(432, "Hello ");
        rope.insert(2345, "world! ");
        rope.insert(5256, "How are ");
        rope.insert(53, "you ");
        rope.insert(768, "doing?\r\n");

        // Send it back
        tx2.send(rope).unwrap();

        // Modify it again
        let mut rope = rx1.recv().unwrap();
        rope.insert(3891, "I'm doing fine, thanks!");
        tx2.send(rope).unwrap();
    });

    // Send the clone to the other thread for modification
    tx1.send(rope2).unwrap();

    // Make identical modifications to rope1 as are being made
    // to rope2 in the other thread.
    rope1.insert(432, "Hello ");
    rope1.insert(2345, "world! ");
    rope1.insert(5256, "How are ");
    rope1.insert(53, "you ");
    rope1.insert(768, "doing?\r\n");

    // Get rope2 back and make sure they match
    let rope2 = rx2.recv().unwrap();
    let matches = Iterator::zip(rope1.chars(), rope2.chars())
        .map(|(a, b)| a == b)
        .all(|n| n);
    assert!(matches);

    // Send rope2 to the other thread again for more modifications.
    tx1.send(rope2).unwrap();

    // Get rope2 back again and make sure they don't match now.
    let rope2 = rx2.recv().unwrap();
    let matches = Iterator::zip(rope1.chars(), rope2.chars())
        .map(|(a, b)| a == b)
        .all(|n| n);
    assert!(!matches);
}
