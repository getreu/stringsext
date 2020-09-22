---
title:    stringsext - search for multi-byte encoded strings in binary data
---

[![Cargo](https://img.shields.io/crates/v/stringsext.svg)](
https://crates.io/crates/stringsext)
[![Documentation](https://docs.rs/stringsext/badge.svg)](
https://docs.rs/stringsext)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/getreu/stringsext)


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
and **\--same-unicode-block** options in the manual page for more details.

**stringsext** is mainly useful for extracting Unicode content out of
non-text files.

When invoked with `stringsext -e ascii` **stringsext** can be used
as *GNU strings* replacement.


## Screenshot

```
stringsext -tx -e utf-8 -e utf-16le -e utf-16be \
           -n 10 -a None -u African  /dev/disk/by-uuid/567a8410

 3de2fff0+	(b UTF-16LE)	ݒݓݔݕݖݗݙݪ
 3de30000+	(b UTF-16LE)	ݫݱݶݷݸݹݺ
<3de36528 	(a UTF-8)	فيأنمامعكلأورديافىهولملكاولهبسالإنهيأيقدهلثمبهلوليبلايبكشيام
>3de36528+	(a UTF-8)	أمنتبيلنحبهممشوش
<3de3a708 	(a UTF-8)	علىإلىهذاآخرعددالىهذهصورغيركانولابينعرضذلكهنايومقالعليانالكن
>3de3a708+	(a UTF-8)	حتىقبلوحةاخرفقطعبدركنإذاكمااحدإلافيهبعضكيفبح
 3de3a780+	(a UTF-8)	ثومنوهوأناجدالهاسلمعندليسعبرصلىمنذبهاأنهمثلكنتالاحيثمصرشرححو
 3de3a7f8+	(a UTF-8)	لوفياذالكلمرةانتالفأبوخاصأنتانهاليعضووقدابنخيربنتلكمشاءوهياب
 3de3a870+	(a UTF-8)	وقصصومارقمأحدنحنعدمرأياحةكتبدونيجبمنهتحتجهةسنةيتمكرةغزةنفسبي
 3de3a8e8+	(a UTF-8)	تللهلناتلكقلبلماعنهأولشيءنورأمافيكبكلذاترتببأنهمسانكبيعفقدحس
 3de3a960+	(a UTF-8)	نلهمشعرأهلشهرقطرطلب
 3df4cca8 	(c UTF-16BE)	փօև։֋֍֏֑֛֚֓֕֗֙֜֝֞׹
<3df4cd20 	(c UTF-16BE)	־ֿ׀ׁׂ׃ׅׄ׆ׇ׈׉׊׋
```


## Documentation

User documentation

*   [Manual page (html)](https://blog.getreu.net/projects/stringsext/stringsext--manpage.html)

*   [Manual page (pdf)](https://blog.getreu.net/_downloads/stringsext--manpage.pdf)

*   [Blogposts about Stringsext](https://blog.getreu.net/tags/stringsext/)

*   [Paper about Stringsext](https://commons.erau.edu/jdfsl/vol14/iss2/4)

Developer documentation

*   [API documentation](https://blog.getreu.net/projects/stringsext/_downloads/doc/stringsext/)

*   [Forensic Tool Development with Rust](https://blog.getreu.net/projects/forensic-tool-development-with-rust)

## Source code

Repository

*   [Stringsext on Github](https://github.com/getreu/stringsext)

*   [Stringsext on Gitlab](https://gitlab.com/getreu/stringsext)

## Distribution

* Binaries for Ubuntu-Linux 18.04, Windows, MacOS (see below for
  Debian binaries)

    1. Open: [Releases - getreu/stringsext](https://github.com/getreu/stringsext/releases)

    2. Open the latest release.

    3. Open *assets*.

    4. Download the packed executable for your operating system.

    5. Installation: see below.

* Binaries and packages (usually built from latest commit):

  - Executable for Windows:

    [x86_64-pc-windows-gnu/release/stringsext.exe](https://blog.getreu.net/projects/stringsext/_downloads/x86_64-pc-windows-gnu/release/stringsext.exe)

  - Binary for Debian 10 Buster:

    [x86_64-unknown-linux-gnu/release/stringsext](https://blog.getreu.net/projects/stringsext/_downloads/x86_64-unknown-linux-gnu/release/stringsext)

    [x86_64-unknown-linux-musl/release/stringsext](https://blog.getreu.net/projects/stringsext/_downloads/x86_64-unknown-linux-musl/release/stringsext)

    [i686-unknown-linux-gnu/release/stringsext](https://blog.getreu.net/projects/stringsext/_downloads/i686-unknown-linux-gnu/release/stringsext)

    [i686-unknown-linux-musl/release/stringsext](https://blog.getreu.net/projects/stringsext/_downloads/i686-unknown-linux-musl/release/stringsext)

  - Package for Debian 10 Buster:

    [x86_64-unknown-linux-gnu/debian/stringsext_2.3.3_amd64.deb](https://blog.getreu.net/projects/stringsext/_downloads/x86_64-unknown-linux-gnu/debian/stringsext_2.3.3_amd64.deb)

    [i686-unknown-linux-gnu/debian/stringsext_2.3.3_i386.deb](https://blog.getreu.net/projects/stringsext/_downloads/i686-unknown-linux-gnu/debian/stringsext_2.3.3_i386.deb)

* Installable Unix man-page:

  - [stringsext.1.gz](https://blog.getreu.net/projects/stringsext/_downloads/stringsext.1.gz)

* Zipfile with all binaries and documentation:

  - [stringsext all](https://blog.getreu.net/_downloads/stringsext.zip)



## Building and installing

1. [Install Rust](https://www.rust-lang.org/tools/install), e.g.

       curl https://sh.rustup.rs -sSf | sh

2. Download, compile and install:

       cargo install stringsext
       sudo cp ~/.cargo/bin/stringsext /usr/local/bin

This project follows [Semantic Versioning](https://semver.org/).



## About

Author

*   Jens Getreu

Copyright

*   Apache 2 license or MIT license

Build status

*   ![status](https://travis-ci.org/getreu/stringsext.svg?branch=master)

