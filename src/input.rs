//! Cut the input stream in chunks for batch processing.

use crate::as_mut_slice_no_borrow_check;
use crate::options::ARGS;
use std::fs::File;
use std::io;
use std::io::Read;
use std::iter::Peekable;
use std::path::Path;
use std::slice;
use std::slice::Iter;
use std::string::String;

/// This is the type used to count bytes in the input stream. Maybe in a future
/// version we raise this to `u128`.
pub type ByteCounter = u64;

/// This is the size of `input_buffer` in bytes. It should be aligned with a
/// multiple of the memory page size, which is - depending on the hardware - `n *
/// 4096` bytes.
#[cfg(not(test))]
pub const INPUT_BUF_LEN: usize = 4096;

#[cfg(test)]
pub const INPUT_BUF_LEN: usize = 0x20;

/// Struct to store the `Slicer`-iterator state. The iterator fills the
/// `input-buffer` with bytes coming from files, whose names are given in the
/// vector `ARGS.arg_FILE`. When one file is exhausted, the iterator switches
/// automatically and transparently to the next file in `ARGS.arg_FILE`. When no
/// data is left in any file, `next()` returns `None`.

pub struct Slicer<'a> {
    /// An iterator over `ARGS.arg_FILE` wrapped in an option. If the option is
    /// `Some()`, then the input should be read from files, whose filenames are
    /// delivered with the iterator's `next()`. If the option is `None`, then the
    /// data comes from `std::stdin`.
    filename_iter: Option<Peekable<Iter<'a, String>>>,

    /// The reader associated with the current file.
    reader: Box<dyn Read>,

    /// An index identifying the source of the input:
    /// The input comes from:
    /// * 0: `stdin`,
    /// * 1: the first file in `ARGS.arg_FILE`,
    /// * 2: the second file in `ARGS.arg_FILE`,
    /// * 3: ...
    current_input_idx: usize,

    /// Is true, when this is the last iteration. After this, comes
    /// only `None`.
    current_input_is_last: bool,

    /// Buffer to store all incoming bytes from the readers. The input is
    /// streamed in this buffer first, before being analysed later in batches.
    input_buffer: [u8; INPUT_BUF_LEN],
}

impl<'a> Slicer<'_> {
    #[inline]
    pub fn new() -> Self {
        if (ARGS.arg_FILE.is_empty()) || ((ARGS.arg_FILE.len() == 1) && ARGS.arg_FILE[0] == "-") {
            Self {
                filename_iter: None,
                reader: Box::new(io::stdin()) as Box<dyn Read>,
                current_input_idx: 0,
                current_input_is_last: true,
                input_buffer: [0u8; INPUT_BUF_LEN],
            }
        } else {
            let mut filename_iter = ARGS.arg_FILE.iter().peekable();
            // `unwrap()` is save because we know `if` above, that there is at least one
            // filename.
            let filename = filename_iter.next().unwrap();
            let reader = match File::open(&Path::new(filename)) {
                Ok(file) => Box::new(file) as Box<dyn Read>,
                Err(e) => {
                    eprintln!("Error: can not read file`{}`: {}", filename, e);
                    Box::new(io::empty()) as Box<dyn Read>
                }
            };
            let current_input_is_last = filename_iter.peek().is_none();

            Self {
                filename_iter: Some(filename_iter),
                // Just to start with something, will be overwritten
                // immediately.
                reader,
                // Convention here: `0` means "not started".
                current_input_idx: 1,
                // There might be more than one file.
                current_input_is_last,
                input_buffer: [0u8; INPUT_BUF_LEN],
            }
        }
    }
}

/// Iterator over the input stream coming from `std::stdin` or from files whose
/// names are listed in `ARGS.arg_FILES`.
impl<'a> Iterator for Slicer<'a> {
    /// The iterator's `next()` returns a tuple `(&[u8], Option<u8>, bool)` with 3 members:
    /// * First member `&[u8]`: \
    ///   a slice of input bytes comprising all valid bytes in `input_buffer`.
    /// * Second member `Option<u8>`:\
    ///   A label identifying the origin of the bytes in `&[u8]`:\
    ///   * `None`: the origin of the input is `stdin`,
    ///   * `Some(1)`: the bytes come from the first file in `ARGS.arg_FILES`,
    ///   * `Some(2)`: the bytes come from the second file in `ARGS.arg_FILES`,
    ///   * `Some(3)`: ...
    ///  * Third member `bool`:\
    ///    * `true`: this chunk of input data is the very last one. All further
    ///      `next()` will return `None`.
    ///    * `false`: More input data will come with the next `next()`.
    type Item = (&'a [u8], Option<u8>, bool);
    /// Returns the next slice of input.
    fn next(&mut self) -> Option<Self::Item> {
        let input_buffer_slice = as_mut_slice_no_borrow_check!(self.input_buffer);
        // Fill the input buffer.
        let no_bytes_received = self.reader.read(input_buffer_slice).expect(&*format!(
            "Error: Could not read input stream no. {}",
            self.current_input_idx
        ));
        let result = &input_buffer_slice[..no_bytes_received];
        let this_stream_ended = no_bytes_received == 0;
        let input_ended = self.current_input_is_last && this_stream_ended;

        // More files to open?
        if this_stream_ended {
            if self.current_input_is_last {
                // Early return
                return None;
            } else {
                // We can safely do first `unwrap()` because
                // `!self.current_input_is_last` can only happen (be true)
                // if `self.filename_iter()` is not `None`.
                // We can safely do second `unwarp()` here, because we have `peek()` ed
                // we already and know there is at least one more filename.
                let filename = self.filename_iter.as_mut().unwrap().next().unwrap();
                self.current_input_idx += 1;
                // The next run needs to know if there is more.
                self.current_input_is_last = self.filename_iter.as_mut().unwrap().peek().is_none();
                let reader = match File::open(&Path::new(filename)) {
                    Ok(file) => Box::new(file) as Box<dyn Read>,
                    Err(e) => {
                        eprintln!("Error: can not read file: {}", e);
                        Box::new(io::empty()) as Box<dyn Read>
                    }
                };
                // Store the reader for the `next()` run.
                self.reader = reader;
            }
        };

        // Change type for output.
        let current_file_id = match self.current_input_idx {
            0 => None,
            // Map 1 -> "A", 2 -> "B", ...
            c => Some(c as u8),
        };
        Some((result, current_file_id, input_ended))
    }
}
