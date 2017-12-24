# 0.5.1 (2017-12-24)

Bug fixes:

* Calling `Rope::line_to_char()` with a line index one-past-the-end would panic.  This wasn't consistent with other indexing, and has been fixed and now returns the one-past-the-end char index.
* Had accidentally left some asserts in the `Rope::remove()` code that were put in during debugging.  They were causing significant slow downs for removes.

Misc:

* Added a changelog file.
