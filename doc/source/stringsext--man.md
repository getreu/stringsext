% STRINGSEXT(1) Version 1.7.0 | Stringsext Documentation

<!--
previous versions
Date:   2016-11-25
Version: 1.1.0

Date: 2017-01-03
Version: 1.2.0

Date: 2017-01-04
Version: 1.2.1

Date: 2017-01-05
Version: 1.2.2

Date: 2017-01-07
Version: 1.3.0

Date: 2017-01-08
Version: 1.3.1

Date: 2017-01-10
Version: 1.4.0

Date: 2017-01-13
Version: 1.4.1

Date: 2017-01-16
Version: 1.4.2

Date: 2017-01-28
Version: 1.4.3

Date: 2017-09-03
Version: 1.4.4

Date: 2018-09-24
Version: 1.5.0

Date: 2018-09-30
Version: 1.6.0

Date: 2020-01-03
Version: 1.7.1
-->

# NAME

Search for multi-byte encoded strings in binary data.

# SYNOPSIS

    stringsext [options] [-e ENC...] [--] [FILE...]
    stringsext [options] [-e ENC...] [--] [-]

# DESCRIPTION

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
and many others. **\--list-encodings** shows a list of valid encoding
names based on the WHATWG Encoding Standard. When more than one encoding
is specified, the scan is performed in different threads simultaneously.

When searching for UTF-16 encoded strings, 96% of all possible two byte
sequences, interpreted as UTF-16 code unit, relate directly to a Unicode
code point. As a result, the probability of encountering valid Unicode
characters in a random byte stream, interpreted as UTF-16, is also 96%.
In order to reduce this big number of false positives, **stringsext**
provides a parameterizable Unicode-block-filter. See **\--encodings**
option for more details.

**stringsext** reads its input data from **FILE**. With no **FILE**, or
when **FILE** is `-`, it reads standard input *stdin*.

When invoked with `stringsext -e ascii -c i` **stringsext** can be used
as *GNU strings* replacement.

Under Windows a Unicode editor is required. For first tests `wordpad`
should do. Choose the `Courier new` font or `Segoe UI symbol` font to
see the flag symbols ⚑ (U+2691).

# OPTIONS

**-c** *MODE*, **\--control-chars**=*MODE*

:   Determine if and how control characters are printed.

    The search algorithm first scans for valid character sequences which
    are then re-encoded into UTF-8 strings containing graphic
    (printable) and control (non-printable) characters.

    When *MODE* is set to **p** all valid (control and graphic)
    characters are printed. Warning: Control characters may contain a
    harmful payload. An attacker may exploit a vulnerability of your
    terminal or post processing software. Use with caution.

    *MODE* **r** will never print any control character but instead
    indicate their position: Control characters in valid strings are
    first grouped and then replaced with the Unicode replacement
    character \'�\' (U+FFFD). This mode is most useful together with
    **\--radix** because it keeps the whole valid character sequence in
    one line allowing post-processing the output with line oriented
    tools like `grep`. To ease post-processing, the output in MODE **r**
    is formatted slightly different from other modes: instead of
    indenting the byte-counter, the encoding name and the found string
    with *spaces* as separator, only one *tab* is inserted.

    When *MODE* is **i** all control characters are silently ignored.
    They are first grouped and then replaced with a newline character.

    See the output of **\--help** for the default value of *MODE*.

**-e** *ENC*, **\--encoding**=*ENC*

:   Set (multiple) input search encodings.

    *ENC*==*ENCNAME*\[,*MIN*\[,*UNICODEBLOCK*\[,*UNICODEBLOCK*\]\]\]

    *ENCNAME*

    :   Search for strings in encoded in ENCNAME. Encoding names
        *ENCNAME* are denoted following the WATHWG standard.
        **\--list-encodings** prints a list of available encodings.

    *MIN*

    :   Print only strings at least min bytes long. The length is
        measured in UTF-8 encoded bytes. *MIN* overwrites the general
        **\--bytes MIN** option for this *ENC* only.

    *UNICODEBLOCK*

    :   Restrict the search to characters within *UNICODEBLOCK*. This
        can be used to search for a certain script or to reduce false
        positives, especially when searching for UTF-16 encoded strings.
        See `https://en.wikipedia.org/wiki/Unicode_block` for a list of
        scripts and their corresponding Unicode-block-ranges.
        *UNICODEBLOCK* has the following syntax:

        *UNICODEBLOCK*==U+*XXXXXX*..U+*YYYYYY*

        *XXXXXX* and *YYYYYY* are the lower and upper bounds of the
        Unicode block in hexadecimal. The prefix `U+` can be omitted.
        The default value for this optional range is `U+0..U+10FFFF`
        which means \"no filter\" or \"print all characters whatever
        their Unicode code-point is\". For performance reasons the
        filter is implemented with a logical bit-mask. If necessary, the
        given *UNICODEBLOCK* is enlarged to be representable as a
        bit-mask. In this case a warning specifying the enlarged
        *UNICODEBLOCK* is emitted.

        When a second optional *UNICODEBLOCK* is given, the total
        Unicode-point search range is the union of the first and the
        second.

    See the output of **\--help** for the default value of *ENC*.

**-f, \--print-file-name**

:   Print the name of the file before each string.

**-h, \--help**

:   Print a synopsis of available options and default values.

**-l, \--list-encodings**

:   List available encodings as WHATWG Encoding Standard names and exit.

**-n** *MIN*, **\--bytes**=*MIN*

:   Print only strings at least *MIN* bytes long. The length is measured
    in UTF-8 encoded bytes. **\--help** shows the default value.

**-p** *FILE*, **\--output**=*FILE*

:   Print to *FILE* instead of *stdout*.

**-s** *SPLIT-MIN*, **\--split\_bytes**=*SPLIT-MIN*

:   Print only split pieces at least *SPLIT-MIN* bytes long. The length
    is measured in UTF-8 encoded bytes and applies to all scanners.
    *SPLIT-MIN=1* (default) ensures that no byte can get lost (never any
    true negatives, but false positives possible). With a value
    *SPLIT-MIN\>1* the first or the second piece can get lost, but the
    probability of false positives is reduced.

    You only need this option when your output contains too many flag
    symbols ⚑ next to very short strings.

    Explanation: In some rare circumstances a graphic string is split
    into two smaller pieces (see LIMITATIONS). Their cutting edges are
    labelled with a flag symbol ⚑ (U+2691). This option controls the
    minimum length of a split piece to be printed.

**-t** *RADIX*, **\--radix**=*RADIX*

:   Print the offset within the file before each valid string. The
    single character argument specifies the radix of the offset: **o**
    for octal, **x** for hexadecimal, or **d** for decimal. When a valid
    string is split into several graphic character sequences, the
    cut-off point is labelled according to the **\--control-chars**
    option and no additional offset is printed at the cut-off point.

    The exception to the above is **\--encoding=ascii
    \--control-chars=i** for which the offset is always printed before
    each graphic character sequence.

    When the output of **stringsext** is piped to another filter you may
    consider **\--control-chars=r** to keep multi-line strings in one
    line.

**-v, \--version**

:   Print version info and exit.

# EXIT STATUS

**0**

:   Success.

**other values**

:   Failure.

# EXAMPLES

List available encodings:

    stringsext -l

Search for UTF-8 strings and strings in UTF-16 Big-Endian encoding:

    stringsext -e utf-8  -e utf-16be  someimage.raw

Same, but read from stream:

    cat someimage.raw | stringsext -e utf-8  -e utf-16be  -

The above is also useful when reading a non-file device:

    cat /dev/sda1  | stringsext -e utf-8  -e utf-16be  -

When used with pipes `-c r` is required:

    stringsext -e iso-8859-7  -c r  -t x  someimage.raw | grep "Ιστορία"

Reduce the number of false positives, when scanning an image file for
UTF-16. In the following example we search for Cyrillic, Arabic and
Siriac strings, which may contain these additional these symbols:
`\t !"#$%&'()*+,-./0123456789:;<=>?`

    stringsext -e UTF-16le,30,U+20..U+3f,U+400..U+07ff someimage.raw

The same but shorter:

    stringsext -e UTF-16le,30,20..3f,400..07ff someimage.raw

Combine Little-Endian and Big-Endian scanning:

    stringsext -e UTF-16be,20,U+0..U+3FF -e UTF-16le,20,U+0..U+3FF someimage.raw

The following settings are designed to produce bit-identical output with
*GNU strings*:

    stringsext -e ascii -c i         # equals `strings`
    stringsext -e ascii -c i -t d    # equals `strings -t d`
    stringsext -e ascii -c i -t x    # equals `strings -t x`
    stringsext -e ascii -c i -t o    # equals `strings -t o`

The following examples perform the same search, but the output format is
slightly different:

    stringsext -e UTF-16LE,10,0..7f  # equals `strings -n 10 -e l`
    stringsext -e UTF-16BE,10,0..7f  # equals `strings -n 10 -e b`

# OPERATING PRINCIPLE

A *valid* string is a sequence a valid characters according to the
encoding chosen with **\--encoding**. A valid string may contain
*control* characters and *graphic* (visible and human readable)
characters. **stringsext** is a tool to extract sequences of graphic
characters out of a binary data stream.

A *scanner* is defined with the **\--encoding ENC** option. Multiple
scanners operate in parallel. The search field is divided into input
chunks of WIN\_LEN bytes (see source code for exact size) in size. A
scanner is a module that extracts valid character sequences, valid
strings, of an input chunk.

A valid string is then fed into a **filter** that extracts multiple
graphic strings out of a valid string. A filter may apply additional
criteria such as *MIN* or *UNICODEBLOCK*.

# LIMITATIONS

1.  Valid strings smaller than FINISH\_STR\_BUF are never cut. When a
    valid string exceeds WIN\_LEN bytes it is always cut. It may happen
    that at the cutting edge locates a short graphic string that is then
    split into two pieces which are printed on separate lines.
    **stringsext** labels such a cutting edge with two flag symbols ⚑
    (U+2691). Furthermore, one or both of those pieces may then become
    too short to meet the **\--bytes** condition. In order not to loose
    any bytes of a piece the **\--bytes** option is not observed for
    split strings. The downside of this is the appearance of some
    undesirable false positives. Therefore the **\--split-bytes** option
    allows to set an additional condition to control the appearance of
    these false positives: The *SPLIT-MIN* value determines the minimum
    number of bytes a split piece must have to be printed. Note that
    with a value *SPLIT-MIN \> 1* some bytes of the split graphic string
    may not appear in the output. Therefore the default is *SPLIT-MIN =
    1*.

    In practice, the above limitation occurs only when the search field
    contains large vectors of Null (0x00) terminated strings. For most
    multi-byte encodings, as well as for the Unicode-scanner, the Null
    (0x00) character is regarded as a valid control character. Thus the
    Unicode scanner will detect such a string vector as one big string
    which might exceed the WIN\_LEN buffer size.

    For searching in large Null (0x00) terminated string vectors, the
    ASCII scanner is recommended. The ASCII scanner regards Null (0x00)
    as an invalid character, so the string vector will be detected as a
    sequence of short distinguishable valid strings. These short strings
    will most likely never exceed the WIN\_LEN buffer and therefore will
    never be split. In such a scenario it is a good practise to run
    Unicode and ASCII scanners in parallel.

    Summary: It is guaranteed that valid strings not longer than
    FINISH\_STR\_BUF are never split. However, when the size of a valid
    string exceeds FINISH\_STR\_BUF bytes it may be split into two or
    more valid strings and then filtered separately. Note that this
    limitation refers to the *valid* string length. A valid string may
    consist of several *graphic* strings. If a valid string is longer
    than WIN\_LEN bytes, it is always split. To know the values of the
    constants please refer to the definition in the source code of your
    **stringsext** build. Original values are: FINISH\_STR\_BUF = 6144
    bytes, WIN\_LEN = 14342 bytes.

2.  It is guaranteed that all string sequences are detected and printed
    according to the search criteria. However due to potential false
    positives when interpreting binary data as multi-byte-strings, it
    may happen that the first characters of a valid string may not be
    recognised immediately. In practice, this effect occurs very rarely
    and the scanner synchronises with the correct character boundaries
    quickly.

# RESOURCES

**Project website:** <https://github.com/getreu/stringsext>

# COPYING

Copyright (C) 2016-2019 Jens Getreu

Licensed under either of

-   Apache License, Version 2.0 (\[LICENSE-APACHE\](LICENSE-APACHE) or
    <http://www.apache.org/licenses/LICENSE-2.0>)
-   MIT license (\[LICENSE-MIT\](LICENSE-MIT) or
    <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms
or conditions. Licenced under the Apache Licence, Version 2.0 (the
\"Licence\"); you may not use this file except in compliance with the
Licence. You may obtain a copy of the Licence at


# AUTHORS

Jens Getreu <getreu@web.de>
