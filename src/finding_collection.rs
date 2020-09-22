use crate::finding::Finding;
use crate::finding::OUTPUT_BUF_LEN;
use crate::input::ByteCounter;
use crate::input::INPUT_BUF_LEN;
use std::io::Write;
use std::marker::PhantomPinned;
use std::ops::Deref;
use std::pin::Pin;

/// `FindingCollection` is a set of ordered `Finding` s.
/// The structs `v` and `output_buffer_bytes` are
/// self referential, because `v` points into
/// `output_buffer_bytes`, hence pinning is required here.
#[derive(Debug)]
pub struct FindingCollection<'a> {
    /// `Finding` s in this vector are in chronological order.
    pub v: Vec<Finding<'a>>,
    /// All concurrent `ScannerState::scan()` start at the same byte. All
    /// `Finding.position` refer to `first_byte_position` as zero.
    pub first_byte_position: ByteCounter,
    /// A buffer containing the UTF-8 representation of all findings during one
    /// `ScannerState::scan()` run. First, the `Decoder` fills in some UTF-8
    /// string. This string is then filtered. The result of this filtering is
    /// some `Finding`-objects stored in a `FindingCollection`. The
    /// `Finding`-objects have a `&str`-member called `Finding::s` that is
    /// a substring of `output_buffer_bytes`.
    pub output_buffer_bytes: Box<[u8]>,
    /// If `output_buffer` is too small to receive all findings, this is set
    /// `true` indicating that only the last `Finding` s could be stored. At
    /// least one `Finding` got lost. This incident is reported to the user. If
    /// ever this happens, the `OUTPUT_BUF_LEN` was not chosen big enough.
    pub str_buf_overflow: bool,
    _marker: PhantomPinned,
}
impl FindingCollection<'_> {
    pub fn new(byte_offset: ByteCounter) -> Self {
        // This buffer lives on the heap. let mut output_buffer_bytes =
        // Box::new([0u8; OUTPUT_BUF_LEN]);
        let output_buffer_bytes = Box::new([0u8; OUTPUT_BUF_LEN]);
        FindingCollection {
            v: Vec::new(),
            first_byte_position: byte_offset,
            output_buffer_bytes,
            str_buf_overflow: false,
            _marker: PhantomPinned,
        }
    }

    /// Clears the buffer to make more space after buffer overflow. Tag the
    /// collection as overflowed.
    pub fn clear_and_mark_incomplete(&mut self) {
        self.v.clear();
        self.str_buf_overflow = true;
    }

    /// This method formats and dumps a `FindingCollection` to the output
    /// channel, usually `stdout`.
    #[allow(dead_code)]
    pub fn print(&self, out: &mut dyn Write) -> Result<(), Box<std::io::Error>> {
        if self.str_buf_overflow {
            eprint!("Warning: output buffer overflow! Some findings might got lost.");
            eprintln!(
                "in input chunk 0x{:x}-0x{:x}.",
                self.first_byte_position,
                self.first_byte_position + INPUT_BUF_LEN as ByteCounter
            );
        }
        for finding in &self.v {
            finding.print(out)?;
        }
        Ok(())
    }
}

/// This allows us to create an iterator from a `FindingCollection`.
impl<'a> IntoIterator for &'a Pin<Box<FindingCollection<'a>>> {
    type Item = &'a Finding<'a>;
    type IntoIter = FindingCollectionIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        FindingCollectionIterator { fc: self, index: 0 }
    }
}

/// This allows iterating over `Finding`-objects in a `FindingCollection::v`.
/// The state of this iterator must hold the whole `FindingCollection` and not
/// only `FindingCollection::v`! This is required because `next()` produces a
/// link to `Finding`, whose member `Finding::s` is a `&str`. The content of this
/// `&str` is part of `FindingCollection::output_buffer_bytes`, thus the need for
/// the whole object `FindingCollection`.

pub struct FindingCollectionIterator<'a> {
    fc: &'a FindingCollection<'a>,
    index: usize,
}

/// This allows us to iterate over `FindingCollection`. It is needed
/// by `kmerge()`.
impl<'a> Iterator for FindingCollectionIterator<'a> {
    type Item = &'a Finding<'a>;
    fn next(&mut self) -> Option<&'a Finding<'a>> {
        let result = if self.index < self.fc.v.len() {
            Some(&self.fc.v[self.index])
        } else {
            None
        };
        self.index += 1;
        result
    }
}

/// We consider the "content" of a `FindingCollection`
/// to be `FindingCollection::v` which is a `Vec<Finding>`.
impl<'a> Deref for FindingCollection<'a> {
    type Target = Vec<Finding<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.v
    }
}
