//! This module deals with command-line arguments and directly related data
//! structures.

use crate::input::ByteCounter;
use lazy_static::lazy_static;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;

/// Encoding name literal used when simulating non-built-in
/// ASCII-decoder.
pub const ASCII_ENC_LABEL: &str = "ascii";

/// If no command-line argument `--chars_min` is given
/// and none is specified in `--encoding` use this.
/// Must be one of `--list-encodings`.
pub const ENCODING_DEFAULT: &str = "UTF-8";

/// Default value, when no `--chars-min` command-line-argument
/// is given. Must be `u8`.
pub const CHARS_MIN_DEFAULT: u8 = 4;

/// Default value, when no `--counter-offset` command-line-argument
/// is given.
pub const COUNTER_OFFSET_DEFAULT: ByteCounter = 0;

/// Default value when no `--output-line-len`
/// command-line-argument is given.
pub const OUTPUT_LINE_CHAR_NB_MAX_DEFAULT: usize = 64;

/// There must be space for at least 3 long Unicode characters,
/// to guarantee progress in streaming. You want much longer lines.
pub const OUTPUT_LINE_CHAR_NB_MIN: usize = 6;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(
    name = "stringsext",
    about = "Find multi-byte encoded strings in binary data."
)]
/// This structure holds the command-line-options and is populated by `docopt`.
/// See man-page and the output of `--list-encodings` and `--help` for more
/// information about their meaning.
pub struct Args {
    /// filter applied after decoding (see
    /// `--list-encodings` for AF examples)
    #[structopt(long, short = "a")]
    pub ascii_filter: Option<String>,
    /// never print byte-counter, encoding or filter
    #[structopt(long, short = "c")]
    pub no_metadata: bool,
    #[structopt(long, short = "d")]
    /// show how command-line-options are interpreted
    pub debug_option: bool,
    /// paths to files to scan (or `-` for stdin)
    #[structopt(name = "FILE", parse(from_os_str))]
    pub inputs: Vec<PathBuf>,
    /// set (multiple) encodings to search for
    #[structopt(long, short = "e")]
    pub encoding: Vec<String>,
    /// grep for characters with ASCII-code in output lines
    #[structopt(long, short = "g")]
    pub grep_char: Option<String>,
    #[structopt(long, short = "l")]
    /// list predefined encoding and filter names for ENC
    pub list_encodings: bool,
    #[structopt(long, short = "n")]
    /// minimum characters of printed strings
    pub chars_min: Option<String>,
    #[structopt(long, short = "r")]
    /// require chars in finding to be in the same Unicode-block
    pub same_unicode_block: bool,
    #[structopt(long, short = "p", parse(from_os_str))]
    /// print not to stdout but in file
    pub output: Option<PathBuf>,
    /// output line length in Unicode-codepoints
    #[structopt(long, short = "q")]
    pub output_line_len: Option<String>,
    /// start counting input bytes with NUM
    #[structopt(long, short = "s")]
    pub counter_offset: Option<String>,
    // enable byte-counter with radix `o`, `x` or `d`
    #[structopt(long, short = "t")]
    pub radix: Option<Radix>,
    /// filter applied after decoding
    /// (see `--list-encodings` for UBF examples)
    #[structopt(long, short = "u")]
    pub unicode_block_filter: Option<String>,
    /// print version and exit
    #[structopt(long, short = "V")]
    pub version: bool,
}

#[derive(Debug, Hash, Clone, Eq, PartialEq, Copy)]
/// radix of the `byte-counter` when printed
pub enum Radix {
    // octal
    O,
    // hexadecimal
    X,
    // decimal
    D,
}

impl FromStr for Radix {
    type Err = String;
    fn from_str(rad: &str) -> Result<Radix, Self::Err> {
        match &*rad.to_ascii_lowercase() {
            "o" => Ok(Radix::O),
            "x" => Ok(Radix::X),
            "d" => Ok(Radix::D),
            _ => Err(String::from("can not convert radix variant")),
        }
    }
}

lazy_static! {
/// Structure to hold the parsed command-line arguments.
pub static ref ARGS : Args = Args::from_args();
}

#[cfg(test)]
mod tests {

    /// Are the command-line option read and processed correctly?
    #[test]
    fn test_arg_parser() {
        use super::{Args, Radix};
        use std::path::PathBuf;
        use structopt::StructOpt;

        // The argv. Normally you"d just use `parse` which will automatically
        // use `std::env::args()`.
        let argv = vec![
            "stringsext",
            "-d",
            "-n",
            "10",
            "-g",
            "64",
            "-e",
            "ascii",
            "-e",
            "utf-8",
            "-V",
            "-l",
            "-s",
            "1500",
            "-p",
            "outfile",
            "-q",
            "40",
            "-t",
            "o",
            "-r",
            "infile1",
            "infile2",
        ];
        let args = Args::from_iter(argv);

        assert_eq!(args.inputs[0], PathBuf::from("infile1"));
        assert_eq!(args.inputs[1], PathBuf::from("infile2"));
        assert!(args.debug_option);
        assert_eq!(
            args.encoding,
            vec!["ascii".to_string(), "utf-8".to_string()]
        );
        assert!(args.version);
        assert!(args.list_encodings);
        assert_eq!(args.chars_min, Some("10".to_string()));
        assert!(args.same_unicode_block);
        assert_eq!(args.grep_char, Some("64".to_string()));
        assert_eq!(args.radix, Some(Radix::O));
        assert_eq!(args.counter_offset, Some("1500".to_string()));
        assert_eq!(args.output, Some(PathBuf::from("outfile")));
        assert_eq!(args.output_line_len, Some("40".to_string()));
        assert!(!args.no_metadata);
    }
}
