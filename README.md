---
title:    stringsext - search for multi-byte encoded strings in binary data
---



**stringsext** is a Unicode enhancement of the *GNU strings* tool with
additional functionalities: **stringsext** recognizes Cyrillic, Arabic, CJKV
characters and other scripts in all supported multi-byte-encodings, while
*GNU strings* fails in finding any of these scripts in UTF-16 and many other
encodings.

**stringsext** prints all graphic character sequences in *FILE* or
*stdin* that are at least *MIN* bytes long.

Unlike *GNU strings* **stringsext** can be configured to search for
valid characters not only in ASCII but also in many other input
encodings, e.g.: UTF-8, UTF-16BE, UTF-16LE, BIG5-2003, EUC-JP, KOI8-R
and many others. The option **\--list-encodings** shows a list of valid
encoding names based on the WHATWG Encoding Standard. When more than one
encoding is specified, the scan is performed in different threads
simultaneously.

When searching for UTF-16 encoded strings, 96% of all possible two byte
sequences, interpreted as UTF-16 code unit, relate directly to Unicode
codepoints. As a result, the probability of encountering valid Unicode
characters in a random byte stream, interpreted as UTF-16, is also 96%.
In order to reduce this big number of false positives, **stringsext**
provides a parametrizable Unicode-block-filter. See **\--encodings**
option in the manual page for more details.

**stringsext** is mainly useful for extracting Unicode content out of
non-text files.

When invoked with `stringsext -e ascii -c i` **stringsext** can be used
as *GNU strings* replacement.

### Documentation

User documentation

*   [Manual page](https://blog.getreu.net/projects/stringsext/stringsext--man.html)
*   [Blogposts about Stringsext](https://blog.getreu.net/tags/stringsext/)

Developer documentation

*    [API documentation](https://blog.getreu.net/projects/stringsext/stringsext/index.html)
*    [Forensic Tool Development with Rust](https://blog.getreu.net/projects/forensic-tool-development-with-rust)

### Source code

Repository

*   [Stringsext on Github](https://github.com/getreu/stringsext)

### Distribution

Binaries

*   [Download](https://blog.getreu.net/projects/stringsext/_downloads/)

Manual page download

*   [stringsext.1.gz](https://blog.getreu.net/projects/stringsext/_downloads/stringsext.1.gz),

### Building and installing

1.  Install *Rust* with [rustup](https://www.rustup.rs/):

        curl https://sh.rustup.rs -sSf | sh

2.  Download [stringsext](#stringsext):

        git clone git@github.com:getreu/stringsext.git

3.  Build

    Enter the *Stringsext* directory where the file `Cargo.toml`
    resides:
    
        cd stringsext
    
    Then execute:

        cargo build --release
        ./doc/make-doc

4.  Install

    a.  Linux:

            # install binary
            sudo cp target/release/stringsext /usr/local/bin/

            # install man-page
            sudo cp man/stringsext.1.gz /usr/local/man/man1/
            sudo dpkg-reconfigure man-db   # e.g. Debian, Ubuntu

    b.  Windows

        Copy the binary `target/release/stringsext.exe` in a directory
        listed in your `PATH` environment variable.

# About

Author

*   Jens Getreu

Copyright

*   Apache 2 license or MIT license

Build status

*   ![status](https://travis-ci.org/getreu/stringsext.svg?branch=master)  

