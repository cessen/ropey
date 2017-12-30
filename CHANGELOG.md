# 0.5.4 (2016-12-28)

Bug fixes:

* Rope::remove() didn't always merge graphemes between chunks properly.

Performance and memory:

* Inserting large texts into a rope now degrades in performance more gracefully as the insertion text becomes larger, rather than hitting a sudden performance cliff.
* `Rope::remove()` got a nice speed boost.
* Memory overhead has been reduced across the board.  Freshly loaded files now only have ~17% overhead, and the worst-case (built up from lots of small random-location inserts) is now ~60% overhead.

Misc:

* 100% unit test coverage of public APIs.
* Added randomized testing via [QuickCheck](https://crates.io/crates/quickcheck).
* Added benchmarks to the library.


# 0.5.3 (2016-12-28)

Performance and memory:

* Massive speed boost for small insertions: between %40 - %50 faster.
* `Rope::from_str()` now only uses stack memory for strings smaller than ~3MB. (Aside from the resulting Rope itself, of course.)

Misc:

* Better unit test coverage of public APIs.  Still not 100%, but getting there!

# 0.5.2 (2016-12-25)

Bug fixes:

* There were ocassionally unnecessary heap allocations that took up a small amount of extra space in the rope.

Misc:

* Memory overhead has been significantly reduced for ropes built up by many small coherent insertions.


# 0.5.1 (2017-12-24)

Bug fixes:

* Calling `Rope::line_to_char()` with a line index one-past-the-end would panic.  This wasn't consistent with other indexing, and has been fixed and now returns the one-past-the-end char index.
* Had accidentally left some asserts in the `Rope::remove()` code that were put in during debugging.  They were causing significant slow downs for removes.

Misc:

* Added a changelog file.
