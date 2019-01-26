//! Example of basic search-and-replace functionality implemented on top
//! of Ropey.
//!
//! Usage:
//!     search_and_replace <search_pattern> <replacement_text> <input_filepath>
//!
//! The file contents with the search-and-replace performed on it is sent to
//! stdout.

extern crate ropey;

use std::fs::File;
use std::io;

use ropey::{iter::Chars, Rope, RopeSlice};

fn main() {
    // Get arguments from commandline
    let (search_pattern, replacement_text, filepath) = if std::env::args().count() > 3 {
        (
            std::env::args().nth(1).unwrap(),
            std::env::args().nth(2).unwrap(),
            std::env::args().nth(3).unwrap(),
        )
    } else {
        eprintln!(
            "Usage:\n    search_and_replace <search_pattern> <replacement_text> <input_filepath>"
        );
        return;
    };

    // Load file contents into a rope.
    let mut text = Rope::from_reader(io::BufReader::new(File::open(&filepath).unwrap())).expect("Cannot read file: either it doesn't exist, file permissions don't allow reading, or is not utf8 text.");

    // Do the search-and-replace.
    search_and_replace(&mut text, &search_pattern, &replacement_text);

    // Print the new text to stdout.
    println!("{}", text);
}

/// Searches the rope for `search_pattern` and replaces all matches with
/// `replacement_text`.
///
/// There are several ways this could be done:  
///
/// 1. Clone the rope and then do the search on the original while replacing
///    on the clone.  This isn't as awful as it sounds because the clone
///    operation is constant-time and the two ropes will share most of their
///    storage in typical cases.  However, this probably isn't the best
///    general solution because it will use a lot of additional space if a
///    large percentage of the text is being replaced.
///
/// 2. A two-stage approach: first find and collect all the matches, then
///    do the replacements on the original rope.  This is a good solution
///    when a relatively small number of matches are expected.  However, if
///    there are a large number of matches then the space to store the
///    matches themselves can become large.
///
/// 3. A piece-meal approach: search for the first match, replace it, then
///    restart the search from there, repeat.  This is a good solution for
///    memory-constrained situations.  However, computationally it is likely
///    the most expensive when there are a large number of matches and there
///    are costs associated with repeatedly restarting the search.
///
/// 4. Combine approaches #2 and #3: collect a fixed number of matches and
///    replace them, then collect another batch of matches and replace them,
///    and so on.  This is probably the best general solution, because it
///    combines the best of both #2 and #3: it allows you to collect the
///    matches in a bounded amount of space, and any costs associated with
///    restarting the search are amortized over multiple matches.
///
/// In this implementation we take approach #4 because it seems the
/// all-around best.
fn search_and_replace(rope: &mut Rope, search_pattern: &str, replacement_text: &str) {
    const BATCH_SIZE: usize = 256;
    let replacement_text_len = replacement_text.chars().count();

    let mut head = 0; // Keep track of where we are between searches
    let mut matches = Vec::with_capacity(BATCH_SIZE);
    loop {
        // Collect the next batch of matches.  Note that we don't use
        // `Iterator::collect()` to collect the batch because we want to
        // re-use the same Vec to avoid unnecessary allocations.
        matches.clear();
        for m in SearchIter::from_rope_slice(&rope.slice(head..), &search_pattern).take(BATCH_SIZE)
        {
            matches.push(m);
        }

        // If there are no matches, we're done!
        if matches.len() == 0 {
            break;
        }

        // Replace the collected matches.
        let mut index_diff: isize = 0;
        for &(start, end) in matches.iter() {
            // Get the properly offset indices.
            let start_d = (head as isize + start as isize + index_diff) as usize;
            let end_d = (head as isize + end as isize + index_diff) as usize;

            // Do the replacement.
            rope.remove(start_d..end_d);
            rope.insert(start_d, &replacement_text);

            // Update the index offset.
            let match_len = (end - start) as isize;
            index_diff = index_diff - match_len + replacement_text_len as isize;
        }

        // Update head for next iteration.
        head = (head as isize + index_diff + matches.last().unwrap().1 as isize) as usize;
    }
}

/// An iterator over simple textual matches in a RopeSlice.
///
/// This implementation is somewhat naive, and could be sped up by using a
/// more sophisticated text searching algorithm such as Boyer-Moore or
/// Knuth-Morris-Pratt.
///
/// The important thing, however, is the interface.  For example, a regex
/// implementation providing an equivalent interface could easily be dropped
/// in, and the search-and-replace function above would work with it quite
/// happily.
struct SearchIter<'a> {
    char_iter: Chars<'a>,
    search_pattern: &'a str,
    search_pattern_char_len: usize,
    cur_index: usize, // The current char index of the search head.
    possible_matches: Vec<std::str::Chars<'a>>, // Tracks where we are in the search pattern for the current possible matches.
}

impl<'a> SearchIter<'a> {
    fn from_rope_slice<'b>(slice: &'b RopeSlice, search_pattern: &'b str) -> SearchIter<'b> {
        assert!(
            search_pattern.len() > 0,
            "Can't search using an empty search pattern."
        );
        SearchIter {
            char_iter: slice.chars(),
            search_pattern: search_pattern,
            search_pattern_char_len: search_pattern.chars().count(),
            cur_index: 0,
            possible_matches: Vec::new(),
        }
    }
}

impl<'a> Iterator for SearchIter<'a> {
    type Item = (usize, usize);

    // Return the start/end char indices of the next match.
    fn next(&mut self) -> Option<(usize, usize)> {
        while let Some(next_char) = self.char_iter.next() {
            self.cur_index += 1;

            // Push new potential match, for a possible match starting at the
            // current char.
            self.possible_matches.push(self.search_pattern.chars());

            // Check the rope's char against the next character in each of
            // the potential matches, removing the potential matches that
            // don't match.  We're using indexing instead of iteration here
            // so that we can remove the possible matches as we go.
            let mut i = 0;
            while i < self.possible_matches.len() {
                let pattern_char = self.possible_matches[i].next().unwrap();
                if next_char == pattern_char {
                    if self.possible_matches[i].clone().next() == None {
                        // We have a match!  Reset possible matches and
                        // return the successful match's char indices.
                        let char_match_range = (
                            self.cur_index - self.search_pattern_char_len,
                            self.cur_index,
                        );
                        self.possible_matches.clear();
                        return Some(char_match_range);
                    } else {
                        // Match isn't complete yet, move on to the next.
                        i += 1;
                    }
                } else {
                    // Doesn't match, remove it.
                    self.possible_matches.swap_remove(i);
                }
            }
        }

        return None;
    }
}
