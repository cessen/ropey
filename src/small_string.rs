use std;
use std::str;
use std::ptr;
use std::ffi::OsStr;
use std::ops::Deref;
use std::borrow::Borrow;
use std::iter::{FromIterator, IntoIterator};
use smallvec;
use smallvec::{Array, SmallVec};

// TODO: FromIterator without having to allocate a String
#[derive(Clone, Default)]
pub struct SmallString<B: Array<Item = u8>> {
    buffer: SmallVec<B>,
}

impl<B: Array<Item = u8>> SmallString<B> {
    pub fn new() -> Self {
        SmallString { buffer: SmallVec::new() }
    }

    pub fn from_str(s: &str) -> Self {
        SmallString { buffer: s.as_bytes().into_iter().cloned().collect() }
    }

    unsafe fn insert_bytes(&mut self, idx: usize, bytes: &[u8]) {
        let len = self.len();
        let amt = bytes.len();
        self.buffer.reserve(amt);

        ptr::copy(
            self.buffer.as_ptr().offset(idx as isize),
            self.buffer.as_mut_ptr().offset((idx + amt) as isize),
            len - idx,
        );
        ptr::copy(
            bytes.as_ptr(),
            self.buffer.as_mut_ptr().offset(idx as isize),
            amt,
        );
        self.buffer.set_len(len + amt);
    }

    #[inline]
    pub fn insert_str(&mut self, idx: usize, string: &str) {
        assert!(self.is_char_boundary(idx));

        unsafe {
            self.insert_bytes(idx, string.as_bytes());
        }
    }

    #[inline]
    pub unsafe fn as_mut_smallvec(&mut self) -> &mut SmallVec<B> {
        &mut self.buffer
    }

    #[inline]
    pub fn split_off(&mut self, at: usize) -> SmallString<B> {
        assert!(self.is_char_boundary(at));
        let len = self.len();
        let mut other = SmallString::new();
        unsafe {
            ptr::copy_nonoverlapping(
                self.buffer.as_ptr().offset(at as isize),
                other.buffer.as_mut_ptr().offset(0),
                len - at,
            );
            self.buffer.set_len(at);
            other.buffer.set_len(len - at);
        }
        other
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.buffer.shrink_to_fit();
    }
}



impl<B: Array<Item = u8>> std::hash::Hash for SmallString<B> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let s: &str = self;
        s.hash(state)
    }
}

impl<B: Array<Item = u8>> std::cmp::PartialEq for SmallString<B> {
    fn eq(&self, other: &Self) -> bool {
        let (s1, s2): (&str, &str) = (self, other);
        s1 == s2
    }
}

impl<B: Array<Item = u8>> std::cmp::Eq for SmallString<B> {}

impl<'a, B: Array<Item = u8>> PartialEq<SmallString<B>> for &'a str {
    fn eq(&self, other: &SmallString<B>) -> bool {
        *self == (other as &str)
    }
}

impl<B: Array<Item = u8>> std::fmt::Display for SmallString<B> {
    fn fmt(&self, fm: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let s: &str = SmallString::deref(self);
        s.fmt(fm)
    }
}

impl<B: Array<Item = u8>> std::fmt::Debug for SmallString<B> {
    fn fmt(&self, fm: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s: &str = SmallString::deref(self);
        s.fmt(fm)
    }
}

impl<'a, B: Array<Item = u8>> From<&'a str> for SmallString<B> {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl<B: Array<Item = u8>> Deref for SmallString<B> {
    type Target = str;

    fn deref(&self) -> &str {
        // We only allow `buffer` to be created from an existing valid string,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}

impl<B: Array<Item = u8>> AsRef<str> for SmallString<B> {
    fn as_ref(&self) -> &str {
        // We only allow `buffer` to be created from an existing valid string,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}

struct Utf8Iterator<I>(I, Option<smallvec::IntoIter<[u8; 4]>>);

impl<I: Iterator<Item = char>> Utf8Iterator<I> {
    pub fn new<In: IntoIterator<IntoIter = I, Item = char>>(into: In) -> Self {
        Utf8Iterator(into.into_iter(), None)
    }
}

impl<I: Iterator<Item = char>> Iterator for Utf8Iterator<I> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut into) = self.1.take() {
            if let Some(n) = into.next() {
                self.1 = Some(into);
                return Some(n);
            }
        }

        let out = self.0.next();

        out.and_then(|chr| {
            let mut dest = [0u8; 4];
            let outstr = chr.encode_utf8(&mut dest);

            self.1 = Some(
                outstr
                    .as_bytes()
                    .into_iter()
                    .cloned()
                    .collect::<SmallVec<[u8; 4]>>()
                    .into_iter(),
            );

            self.1.as_mut().and_then(|i| i.next())
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.0.size_hint();

        (hint.0, hint.1.map(|x| x * 4))
    }
}

impl<B: Array<Item = u8>> FromIterator<char> for SmallString<B> {
    fn from_iter<T: IntoIterator<Item = char>>(into_iter: T) -> Self {
        // We're a shell so we mostly work with ASCII data - optimise for this
        // case since we have to optimise for _some_ fixed size of char.
        let utf8 = Utf8Iterator::new(into_iter);

        SmallString { buffer: utf8.collect() }
    }
}

#[cfg(feature = "as-mut")]
impl<B: Array<Item = u8>> AsMut<str> for SmallString<B> {
    fn as_mut(&mut self) -> &mut str {
        // We only allow `buffer` to be created from an existing valid string,
        // so this is safe.
        unsafe { str::from_utf8_unchecked_mut(self.buffer.as_mut()) }
    }
}

impl<B: Array<Item = u8>> AsRef<OsStr> for SmallString<B> {
    fn as_ref(&self) -> &OsStr {
        let s: &str = self.as_ref();
        s.as_ref()
    }
}

impl<B: Array<Item = u8>> Borrow<str> for SmallString<B> {
    fn borrow(&self) -> &str {
        // We only allow `buffer` to be created from an existing valid string,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}

impl<B: Array<Item = u8>> From<String> for SmallString<B> {
    fn from(s: String) -> SmallString<B> {
        SmallString { buffer: SmallVec::from_vec(s.into_bytes()) }
    }
}

impl<B: Array<Item = u8>> From<SmallString<B>> for String {
    fn from(s: SmallString<B>) -> String {
        unsafe { String::from_utf8_unchecked(s.buffer.into_vec()) }
    }
}
