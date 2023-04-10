pub trait TwoWayIterator: Iterator {
    fn prev(&mut self) -> Option<Self::Item>;

    fn two_way_peekable(self) -> TwoWayPeekable<Self>
    where
        Self: Sized,
        Self::Item: Copy,
    {
        TwoWayPeekable {
            itr: self,
            peeked: Peeked::None,
        }
    }
}

#[derive(Debug)]
enum Peeked<T> {
    None,
    Prev(Option<T>), // remember peeked value even if it was none
    Next(Option<T>),
}

pub struct TwoWayPeekable<I>
where
    I: TwoWayIterator,
    I::Item: Copy,
{
    itr: I,
    peeked: Peeked<I::Item>,
}

impl<I> Iterator for TwoWayPeekable<I>
where
    I: TwoWayIterator,
    I::Item: Copy,
{
    type Item = I::Item;

    /// Advances the iterator forward and returns the next value.
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.peeked {
            Peeked::None => self.itr.next(),
            Peeked::Next(next) => {
                self.peeked = Peeked::None;
                next
            }
            Peeked::Prev(_) => {
                self.peeked = Peeked::None;
                self.itr.next(); // compensate for prev peeked one
                self.itr.next()
            }
        }
    }
}

impl<I> TwoWayIterator for TwoWayPeekable<I>
where
    I: TwoWayIterator,
    I::Item: Copy,
{
    /// Advances the iterator backwards and returns the previous value.
    #[inline]
    fn prev(&mut self) -> Option<Self::Item> {
        match self.peeked {
            Peeked::None => self.itr.prev(),
            Peeked::Prev(prev) => {
                self.peeked = Peeked::None;
                prev
            }
            Peeked::Next(_) => {
                self.peeked = Peeked::None;
                self.itr.prev(); // compensate for prev peeked one
                self.itr.prev()
            }
        }
    }
}

impl<I> TwoWayPeekable<I>
where
    I: TwoWayIterator,
    I::Item: Copy,
{
    /// Return the next value witout advancing the iterator.
    #[inline]
    pub fn peek_next(&mut self) -> Option<I::Item> {
        match self.peeked {
            Peeked::Next(next) => next,
            _ => {
                if let Peeked::Prev(Some(_)) = self.peeked {
                    // compensate for prev peeked one
                    self.itr.next();
                }

                let next = self.itr.next();
                self.peeked = Peeked::Next(next);
                next
            }
        }
    }

    /// Return the previous value witout advancing the iterator.
    #[inline]
    pub fn peek_prev(&mut self) -> Option<I::Item> {
        match self.peeked {
            Peeked::Prev(prev) => prev,
            _ => {
                if let Peeked::Next(Some(_)) = self.peeked {
                    // compensate for next peeked one
                    self.itr.prev();
                }

                let prev = self.itr.prev();
                self.peeked = Peeked::Prev(prev);
                prev
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rope;

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_01() {
        let r = Rope::from_str("a");
        let mut i = r.chars().two_way_peekable();

        assert_eq!(None, i.prev());
        assert_eq!(Some('a'), i.peek_next());
        assert_eq!(Some('a'), i.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_02() {
        let r = Rope::from_str("a");
        let mut i = r.chars().two_way_peekable();

        assert_eq!(Some('a'), i.next());
        assert_eq!(None, i.next());
        assert_eq!(Some('a'), i.peek_prev());
        assert_eq!(Some('a'), i.prev());
        assert_eq!(None, i.peek_prev());
        assert_eq!(Some('a'), i.peek_next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_03() {
        let r = Rope::from_str("ab");
        let mut i = r.chars().two_way_peekable();

        assert_eq!(Some('a'), i.next());
        assert_eq!(Some('b'), i.peek_next());
        assert_eq!(Some('a'), i.peek_prev());
        assert_eq!(Some('b'), i.next());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_04() {
        let r = Rope::from_str("ab");
        let mut i = r.chars().two_way_peekable();

        assert_eq!(Some('a'), i.next());
        assert_eq!(Some('b'), i.next());
        assert_eq!(None, i.peek_next());
        assert_eq!(Some('b'), i.peek_prev());
        assert_eq!(None, i.next());
        assert_eq!(Some('b'), i.peek_prev());
        assert_eq!(Some('b'), i.prev());
        assert_eq!(Some('a'), i.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn chars_reversed() {
        let r = Rope::from_str("ab");
        let mut i = r.chars().reversed().two_way_peekable();

        assert_eq!(None, i.next());
        assert_eq!(Some('a'), i.prev());
        assert_eq!(Some('b'), i.peek_prev());
        assert_eq!(Some('a'), i.peek_next());
        assert_eq!(Some('b'), i.prev());
        assert_eq!(None, i.prev());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn lines() {
        let r = Rope::from_str(
            "\
Roses are red
Violets are blue
Ropes are brown
or yellow? idk",
        );
        let mut i = r.lines().two_way_peekable();

        assert_eq!(Some("Roses are red\n".into()), i.next());
        assert_eq!(Some("Violets are blue\n".into()), i.next());
        assert_eq!(Some("Ropes are brown\n".into()), i.next());
        assert_eq!(Some("or yellow? idk".into()), i.peek_next());
        assert_eq!(Some("Ropes are brown\n".into()), i.peek_prev());
        assert_eq!(Some("or yellow? idk".into()), i.next());
    }
}
