.. Main project page for ``stringsext``






************
 stringsext
************



-------------------------------------------------------------------
stringsext - search for multi-byte encoded strings in binary data.
-------------------------------------------------------------------


:Author: Jens Getreu
:Copyright: Apache 2 license
:Build status: 
   .. image:: https://travis-ci.org/getreu/stringsext.svg?branch=master
      :target: https://travis-ci.org/getreu/stringsext

**stringsext** is a Unicode enhancement of the *GNU strings* tool with
additional functionalities: **stringsext** recognizes Cyrillic, CJKV
characters and other scripts in all supported multi-byte-encodings,
while *GNU strings* fails in finding any of these scripts in UTF-16 and
many other encodings.

**stringsext** prints all graphic character sequences in *FILE* or
*stdin* that are at least *MIN* bytes long.

Unlike *GNU strings* **stringsext** can be configured to search for
valid characters not only in ASCII but also in many other input
encodings, e.g.: UTF-8, UTF-16BE, UTF-16LE, BIG5-2003, EUC-JP, KOI8-R
and many others. The option **--list-encodings** shows a list of valid
encoding names based on the WHATWG Encoding Standard. When more than one
encoding is specified, the scan is performed in different threads
simultaneously.

When searching for UTF-16 encoded strings, 96% of all possible two byte
sequences, interpreted as UTF-16 code unit, relate directly to a Unicode
code point. As a result, the probability of encountering valid Unicode
characters in a random byte stream, interpreted as UTF-16, is also 96%.
In order to reduce this big number of false positives, **stringsext**
provides a parameterizable Unicode-block-filter. See **--encodings**
option in the manual page for more details.

**stringsext** is mainly useful for determining the Unicode content of
non-text files.

When invoked with ``stringsext -e ascii -c i`` **stringsext** can be
used as *GNU strings* replacement.

Documentation
=============

User documentation
    `manual
    page <https://getreu.net/public/downloads/doc/stringsext/./doc/build/stringsext--man.html>`__

Developer documentation
    | `API documentation`_
    | `Forensic Tool Development with Rust`_

.. _`API documentation`: https://getreu.net/public/downloads/doc/stringsext/./target/doc/stringsext/index.html_
.. _`Forensic Tool Development with Rust`: https://getreu.net/public/downloads/doc/forensic-tool-development-with-rust

Source code
===========

Repository
    `stringsext on Github <https://github.com/getreu/stringsext>`__

Distribution
============

Binaries
    Download `stringsext binaries`_ and verify  hashes_.

Manual page
    `stringsext.1.gz`_

.. _`stringsext binaries`: https://getreu.net/public/downloads/doc/stringsext/./target/
.. _hashes: https://getreu.net/public/sha256sum.txt
.. _`stringsext.1.gz`: https://getreu.net/public/downloads/doc/stringsext/./man/man1/stringsext.1.gz



Building and installing
=======================

#. Install *Rust* with rustup_::

      curl https://sh.rustup.rs -sSf | sh

#. Download stringsext_::

      git clone git@github.com:getreu/stringsext.git

#. Build

   Enter the *Stringsext* source directory where the file ``Cargo.toml`` resides. Then execute::

      cargo build --release
      ./make-doc

#. Install

   a. Linux:

      .. code:: bash

         # install binary
         sudo cp target/release/stringsext /usr/local/bin/

         # install man-page
         sudo cp man/stringsext.1.gz /usr/local/man/man1/
         sudo dpkg-reconfigure man-db   # e.g. Debian, Ubuntu

   b. Windows

      Copy the binary ``target/release/stringsext.exe`` in a directory
      listed in your ``PATH`` environment variable.

.. _rustup: https://www.rustup.rs/
