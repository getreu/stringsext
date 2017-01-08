============
 stringsext
============



-----------------------------------------------------
search for multi-byte encoded strings in binary data.
-----------------------------------------------------

.. Previous versions
   :Date:   2016-11-25
   :Version: 1.1.0

   :Date: 2017-01-03
   :Version: 1.2.0

   :Date: 2017-01-04
   :Version: 1.2.1

   :Date: 2017-01-05
   :Version: 1.2.2

   :Date: 2017-01-07
   :Version: 1.3.0

:Author: Jens Getreu
:Date: 2017-01-08
:Version: 1.3.1
:Copyright: Apache License, Version 2.0 (for details see COPYING section)
:Manual section: 1
:Manual group: Forensic Tools







SYNOPSIS
========

::

    stringsext [options] [-e ENC...] [--] [FILE...]
    stringsext [options] [-e ENC...] [--] [-]

DESCRIPTION
===========

**stringsext** is a Unicode enhancement of the *GNU strings* tool with
additional functionalities: **stringsext** recognizes Cyrillic, CJKV
characters and other scripts in all supported multi-byte-encodings,
while *GNU strings* fails in finding any of these scripts in UTF-16 and
many other encodings.

**stringsext** is mainly useful for determining the Unicode content of
non-text files: It prints all graphic character sequences in *FILE* or
*stdin* that are at least *MIN* bytes long.

Unlike *GNU strings* **stringsext** can be configured to search for
valid characters not only in ASCII but also in many other input
encodings, e.g.: utf-8, utf-16be, utf-16le, big5-2003, euc-jp, koi8-r
and many others. **--list-encodings** shows a list of valid encoding
names based on the WHATWG Encoding Standard. When more than one encoding
is specified, the scan is performed in different threads simultaneously.

When searching for UTF-16 encoded strings, 96% of all possible two byte
sequences, interpreted as UTF-16 code unit, relate directly to a Unicode
code point. As a result, the probability of encountering valid Unicode
characters in a random byte stream, interpreted as UTF-16, is also 96%.
In order to reduce this big number of false positives, **stringsext**
provides a parameterizable Unicode-block-filter. See **--encodings**
option for more details.

**stringsext** reads its input data from **FILE**. With no **FILE**, or
when **FILE** is ``-``, it reads standard input *stdin*.

When invoked with ``stringsext -e ascii -c i`` **stringsext** can be
used as *GNU strings* replacement.

OPTIONS
=======

**-c** *MODE*, **--control-chars**\ =\ *MODE*
    Determine if and how control characters are printed.

    The search algorithm first scans for valid character sequences which
    are then are re-encoded into UTF-8 strings containing graphic
    (printable) and control (non-printable) characters.

    When *MODE* is set to **p** all valid (control and graphic)
    characters are printed. Warning: Control characters may contain a
    harmful payload. An attacker may exploit a vulnerability of your
    terminal or post processing software. Use with caution.

    *MODE* **r** will never print any control character but instead
    indicate their position: Control characters in valid strings are
    first grouped and then replaced with the Unicode replacement
    character '�' (U+FFFD). This mode is most useful together with
    **--radix** because it keeps the whole valid character sequence in
    one line allowing post-processing the output with line oriented
    tools like ``grep``. To ease post-processing, the output in MODE
    **r** is formatted slightly different from other modes: instead of
    indenting the byte-counter, the encoding name and the found string
    with *spaces* as separator, only one *tab* is inserted.

    When *MODE* is **i** all control characters are silently ignored.
    They are first grouped and then replaced with a newline character.

    See the output of **--help** for the default value of *MODE*.

**-e** *ENC*, **--encoding**\ =\ *ENC*
    Set (multiple) input search encodings.

    *ENC*\ ==\ *ENCNAME*\ [,\ *MIN*\ [,\ *UNICODEBLOCK*\ [,\ *UNICODEBLOCK*\ ]]]

    *ENCNAME*
        Search for strings in encoded in ENCNAME. Encoding names
        *ENCNAME* are denoted following the WATHWG standard.
        **--list-encodings** prints a list of available encodings.

    *MIN*
        Print only strings at least min bytes long. The length is
        measured in UTF-8 encoded bytes. *MIN* overwrites the general
        **--bytes MIN** option for this *ENC* only.

    *UNICODEBLOCK*
        Restrict the search to characters within *UNICODEBLOCK*. This
        can be used to search for a certain script or to reduce false
        positives, especially when searching for UTF-16 encoded strings. See
        ``https://en.wikipedia.org/wiki/Unicode_block`` for a list of
        scripts and their corresponding Unicode-block-ranges.
        *UNICODEBLOCK* has the following syntax:

        *UNICODEBLOCK*\ ==U+\ *XXXXXX*..U+\ *YYYYYY*

        *XXXXXX* and *YYYYYY* are the lower and upper bounds of the
        Unicode block in hexadecimal. The prefix ``U+`` can be omitted.
        The default value for this optional range is ``U+0..U+10FFFF``
        which means "no filter" or "print all characters whatever their
        Unicode code-point is". For performance reasons the filter is
        implemented with a logical bit-mask. If necessary, the given
        *UNICODEBLOCK* is enlarged to be representable as a bit-mask. In
        this case a warning specifying the enlarged *UNICODEBLOCK* is
        emitted.

        When a second optional *UNICODEBLOCK* is given, the total
        Unicode-point search range is the union of the first and the second.

    See the output of **--help** for the default value of *ENC*.

**-h, --help**
    Print a synopsis of available options and default values.

**-l, --list-encodings**
    List available encodings as WHATWG Encoding Standard names and exit.

**-n** *MIN*, **--bytes**\ =\ *MIN*
    Print only strings at least *min* bytes long. The length is measured
    in UTF-8 encoded bytes. **--help** shows the default value.

**-p** *FILE*, **--output**\ =\ *FILE*
    Print to *FILE* instead of *stdout*.

**-t** *RADIX*, **--radix**\ =\ *RADIX*
    Print the offset within the file before each valid string. The
    single character argument specifies the radix of the offset: **o**
    for octal, **x** for hexadecimal, or **d** for decimal. When a valid
    string is split into several graphic character sequences, the
    cut-off point is labelled according to the **--control-chars**
    option and no additional offset is printed at the cut-off point.

    The exception to the above is **--encoding=ascii --control-chars=i**
    for which the offset is always printed before each graphic character
    sequence.

    When the output of **stringsext** is piped to another filter you may
    consider **--control-chars=r** to keep multi-line strings in one
    line.

**-V, --version**
    Print version info and exit.

EXIT STATUS
===========

**0**
    Success.

**other values**
    Failure.

EXAMPLES
========

List available encodings:

::

    stringsext -l

Search for UTF-8 strings and strings in UTF-16 Big-Endian encoding:

::

    stringsext -e utf-8  -e utf-16be  someimage.raw

Same, but read from stream:

::

    cat someimage.raw | stringsext -e utf-8  -e utf-16be  -

The above is also useful when reading a non-file device:

::

    cat /dev/sda1  | stringsext -e utf-8  -e utf-16be  -

When used with pipes ``-c r`` is required:

::

    stringsext -e iso-8859-7  -c r  -t x  someimage.raw | grep "Ιστορία"

Reduce the number of false positives, when scanning an image file for
UTF-16. In the following example we search for Cyrillic, Arabic and Siriac
strings, which may contain these additional these symbols:
``\t !"#$%&'()*+,-./0123456789:;<=>?``

::

    stringsext -e UTF-16le,30,U+20..U+3f,U+400..U+07ff someimage.raw

The same but shorter:

::

    stringsext -e UTF-16le,30,20..3f,400..07ff someimage.raw

Combine Little-Endian and Big-Endian scanning:

::

    stringsext -e UTF-16be,20,U+0..U+3FF -e UTF-16le,20,U+0..U+3FF someimage.raw

The following settings are designed to produce bit-identical output with
*GNU strings*:

::

    stringsext -e ascii -c i         # equals `strings`
    stringsext -e ascii -c i -t d    # equals `strings -t d`
    stringsext -e ascii -c i -t x    # equals `strings -t x`
    stringsext -e ascii -c i -t o    # equals `strings -t o`

The following examples perform the same search, but the output format is
slightly different:

::

    stringsext -e UTF-16LE,10,0..7f  # equals `strings -n 10 -e l`
    stringsext -e UTF-16BE,10,0..7f  # equals `strings -n 10 -e b`


LIMITATIONS
===========

It is guaranteed that all valid string sequences are detected and printed
whatever their size is. However due to potential false positives when
interpreting binary data as multi-byte-strings, it may happen that the first
characters of a valid string may not be recognised immediately. In practice,
this effect occurs very rarely and the scanner synchronises with the correct
character boundaries quickly.

Valid strings not longer than FLAG\_BYTES\_MAX are never split and printed in
full length. However, when the size of a valid string exceeds FLAG\_BYTES\_MAX
bytes it may be split into two or more strings and then printed separately. Note
that this limitation refers to the *valid* string length. A valid string may
consist of several *graphic* strings.  If a valid string is longer than WIN\_LEN
bytes, it is always split. To know the values of the constants please refer
to the definition in the source code of your **stringsext** build. Original
values are: FLAG\_BYTES\_MAX = 6144 bytes, WIN\_LEN = 14342 bytes.

In practise the above limitation may appear when the search field contains large
vectors of Null (0x00) delimited strings. For most multi-byte encodings, as well
as for the Unicode-scanner, the Null (0x00) character is regarded as a valid
control character. Thus the Unicode scanner will detect such a string vector as
one big string which might exceed the WIN\_LEN buffer size. The scanner then
cuts the big string into pieces of length WIN\_LEN and it may happen that at the
cutting edge a short string is cut into 2 pieces. It will later appear as 2
separate findings. In the work case it might even happen that the first piece is
to short to be printed at all! This is because: only when the scanning process
for valid strings in the WIN\_LEN buffer is terminated, a second filter splits
the long valid strings into a sequence of short graphic strings.
These short graphic strings are subject to additional restrictions like
minimum length or a Unicode-block-filter (see above).

As a workaround, in case you search for certain character sequence in such large
Null (0x00) delimited string vectors, the ASCII scanner is recommended. The
ASCII scanner regards Null (0x00) as invalid character, so the string vector
will be detected as sequence of short distinguished strings. These short strings
will most likely never exceed the WIN\_LEN buffer and therefor will never be
split.  In such a scenario it is a good practise to run Unicode and ASCII
scanners in parallel.

When a graphic string has to be cut at the WIN_LEN buffer boundary, *stringsext*
can not in all cases determine the length of the first piece. In these rare
cases *stringsext* always prints the second piece, even when it is shorter than
**--bytes** would require.




RESOURCES
=========

**Project website:** https://github.com/getreu/stringsext

COPYING
=======

Copyright (C) 2016 Jens Getreu

Licenced under the Apache Licence, Version 2.0 (the "Licence"); you may
not use this file except in compliance with the Licence. You may obtain
a copy of the Licence at

::

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the Licence is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the Licence for the specific language governing permissions and
limitations under the Licence.
