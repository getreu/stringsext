//! Store string-findings and prepare them for output.

extern crate encoding_rs;

use crate::input::ByteCounter;
use crate::input::INPUT_BUF_LEN;
use crate::mission::Mission;
use crate::options::Radix;
use crate::options::ARGS;
use crate::options::ASCII_ENC_LABEL;
use pin_project::pin_project;
use std::io::Write;
use std::ops::Deref;
use std::str;

/// `OUTPUT_BUF_LEN` needs to be long enough to hold all findings that are
/// decoded to UTF-8 in `scan::scan()`. To estimate the space needed to receive
/// all decodings in UTF-8, the worst case - Asian like `EUC_JP` - has to be
/// taken into consideration: Therefor, in order to avoid output buffer overflow,
/// `OUTPUT_BUF_LEN` should be at least twice as big as `INPUT_BUF_LEN`. You can
/// also check the minimum length with
/// `Decoder::max_utf8_buffer_length_without_replacement`. Unfortunately this can
/// not be done programmatically, because `output_buffer` is a statically
/// allocated array.
#[cfg(not(test))]
pub const OUTPUT_BUF_LEN: usize = 0x9192;
#[cfg(test)]
pub const OUTPUT_BUF_LEN: usize = 0x40;

/// Extra space in bytes for `ByteCounter` and encoding-name when `Finding::print()`
/// prints  a `Finding`.
pub const OUTPUT_LINE_METADATA_LEN: usize = 40;

/// `FindingCollection` is a set of ordered `Finding` s.
/// This struct is self referential, because `Finding` points into
/// `output_buffer_bytes`, hence pinning is required here.
#[pin_project]
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
    #[pin]
    pub output_buffer_bytes: Box<[u8]>,
    /// If `output_buffer` is too small to receive all findings, this is set
    /// `true` indicating that only the last `Finding` s could be stored. At
    /// least one `Finding` got lost. This incident is reported to the user. If
    /// ever this happens, the `OUTPUT_BUF_LEN` was not chosen big enough.
    pub str_buf_overflow: bool,
}
impl FindingCollection<'_> {
    pub fn new(byte_offset: ByteCounter) -> Self {
        // This buffer lives on the heap. let mut output_buffer_bytes =
        // Box::new([0u8; OUTPUT_BUF_LEN]);
        let output_buffer_bytes = Box::new([0u8; OUTPUT_BUF_LEN]);
        Self {
            v: Vec::new(),
            first_byte_position: byte_offset,
            output_buffer_bytes,
            str_buf_overflow: false,
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

impl<'a> IntoIterator for &'a FindingCollection<'a> {
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

#[derive(Debug, Eq, PartialEq)]
/// Used to express the precision of `Finding::position` when the algorithm can
/// not determine its exact position.
pub enum Precision {
    /// The finding is located somewhere before `Finding::position`. It is
    /// guarantied, that the finding is not farer than 2*`--output-line-len`
    /// bytes (or the previous finding from the same scanner) away.
    Before,
    /// The algorithm could determine the exact position of the `Finding` at
    /// `Finding::position`.
    Exact,
    /// The finding is located some `[1..2* --output_line_len]` bytes after
    /// `Finding::position` or - in any case - always before the next
    /// `Finding::position`.
    After,
}

/// `Finding` represents a valid result string decoded to UTF-8 with it's
/// original location and its original encoding in the input stream.
#[derive(Debug)]
pub struct Finding<'a> {
    /// A label identifying the origin of the input data: If the origin of the data
    /// is `stdin`: `None`, otherwise: `Some(1)` for input coming from the first
    /// file, `Some(2)` for input from the second file, `Some(3)` for ...
    pub input_file_id: Option<u8>,
    /// `Mission` associated with this finding. We need a reference to the
    /// corresponding `Mission` object here, in order to get additional information,
    /// e.g. the label of the encoding, when we print this `Finding`.
    pub mission: &'static Mission,
    /// The byte number position of this `Finding` in the input stream.
    pub position: ByteCounter,
    /// In some cases the `position` can not be determined exactly. Therefor,
    /// `position_precision` indicates how well the finding is localized. In case
    /// that the position is not exactly known, we indicate if the finding is
    /// somewhere before or after `position`.
    pub position_precision: Precision,
    /// Whatever the original encoding was, the result string `s` is always stored as
    /// UTF-8. `s` is a `&str` pointing into `FindingCollection::output_buffer`.
    pub s: &'a str,
    /// This flag indicates that `s` holds only the second part of a cut finding
    /// from the previous `scanner::scan()` run. This can happen when a finding from
    /// the previous run has hit the`input_buffer`-boundary.
    pub s_completes_previous_s: bool,
}

impl Eq for Finding<'_> {}

/// Useful to compare findings for debugging or testing.
impl PartialEq for Finding<'_> {
    fn eq(&self, other: &Self) -> bool {
        (self.position == other.position)
            && (self.position_precision == other.position_precision)
            && (self.mission.encoding.name() == other.mission.encoding.name())
            && (self.mission.filter == other.mission.filter)
            && (self.s == other.s)
    }
}

/// When `itertools::kmerge()` merges `FindingCollections` into an iterator over
/// `Finding` s, it needs to compare `Finding` s. Therefor, we must implement
/// `PartialOrd`.
impl PartialOrd for Finding<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.position != other.position {
            self.position.partial_cmp(&other.position)
        } else if self.mission.mission_id != other.mission.mission_id {
            self.mission
                .mission_id
                .partial_cmp(&other.mission.mission_id)
        } else if self.mission.filter.ubf != other.mission.filter.ubf {
            self.mission
                .filter
                .ubf
                .partial_cmp(&other.mission.filter.ubf)
        } else {
            self.mission.filter.af.partial_cmp(&other.mission.filter.af)
        }
    }
}

impl<'a> Finding<'a> {
    pub fn print(&self, out: &mut dyn Write) -> Result<(), Box<std::io::Error>> {
        out.write_all(b"\n")?;
        if !ARGS.no_metadata {
            if ARGS.inputs.len() > 1 {
                if let Some(i) = self.input_file_id {
                    // map 1 -> 'A', 2 -> 'B', 3 -> 'C'
                    out.write_all(&[i + 64 as u8, b' '])?;
                }
            };

            if ARGS.radix.is_some() {
                match &self.position_precision {
                    Precision::After => out.write_all(b">")?,
                    Precision::Exact => out.write_all(b" ")?,
                    Precision::Before => out.write_all(b"<")?,
                };
                match ARGS.radix {
                    Some(Radix::X) => out.write_fmt(format_args!("{:0x}", self.position,))?,
                    Some(Radix::D) => out.write_fmt(format_args!("{:0}", self.position,))?,
                    Some(Radix::O) => out.write_fmt(format_args!("{:0o}", self.position,))?,
                    None => {}
                };
                if self.s_completes_previous_s {
                    out.write_all(b"+\t")?
                } else {
                    out.write_all(b" \t")?
                };
            }

            if ARGS.encoding.len() > 1 {
                // map 0 -> 'a', 1 -> 'b', 2 -> 'c' ...
                out.write_all(&[b'(', self.mission.mission_id + 97 as u8, b' '])?;
                out.write_all(if self.mission.print_encoding_as_ascii {
                    ASCII_ENC_LABEL.as_bytes()
                } else {
                    self.mission.encoding.name().as_bytes()
                })?;
                // After ")" send two tabs.
                out.write_all(b")\t")?;
            };
        };
        out.write_all(self.s.as_bytes())?;
        Ok(())
    }
}
