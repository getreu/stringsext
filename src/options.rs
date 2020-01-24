//! This module deals with command-line arguments and directly related data
//! structures.

use docopt::Docopt;
use lazy_static::lazy_static;
use serde_derive::Deserialize;

/// Encoding name literal used when simulating non-built-in
/// ASCII-decoder.
#[macro_export]
macro_rules! ascii_enc_label {
    () => {
        "ascii"
    };
}

/// If no command-line argument `--chars_min` is given
/// and none is specified in `--encoding` use this.
/// Must be one of `--list-encodings`.
#[macro_export]
macro_rules! encoding_default {
    () => {
        //ascii_enc_label!()
        "UTF-8"
    };
}

/// Default value, when no `--chars-min` command-line-argument
/// is given. Must be `u8`.
#[macro_export]
macro_rules! chars_min_default {
    () => {
        4u8
    };
}

/// Default value, when no `--counter-offset` command-line-argument
/// is given. Must be of type `ByteCounter`.
#[macro_export]
macro_rules! counter_offset_default {
    () => {
        0
    };
}

/// Default value when no `--output-line-len`
/// command-line-argument is given. Must be `usize`.
#[macro_export]
macro_rules! output_line_char_nb_max_default {
    () => {
        64usize
    };
}

/// There must be space for at least 3 long Unicode characters,
/// to guarantee progress in streaming. You want much longer lines.
pub const OUTPUT_LINE_CHAR_NB_MIN: usize = 6;

/// Message printed for command-line `--help`.
const USAGE: &str = concat!(
    "
Usage: stringsext [options] [-e ENC...] [--] [FILE...]
       stringsext [options] [-e ENC...] [--] [-]

Options:
 -a AF --ascii-filter=AF        ASCII-filter AF applied after decoding. See
                                `--list-encodings` for AF examples.
 -c, --no-metadata              Never print byte-counter, encoding or filter.
 -d, --debug-options            Show how command-line-options are interpreted.
 -e ENC, --encoding=ENC         Set (multiple) input search encodings (default: ",
    encoding_default!(),
    ").
                                ENC==[ENCNAME],[MIN],[AF],[UBF],[GREP-CHAR]
                                ENCNAME: `ascii`, `utf-8`, `big5`, ...
                                MIN: overwrites general `--bytes MIN` for this ENC only.
                                AF (ASCII-FILTER): `all-ctrl`, `0xffff...`, ...
                                UBF (UNICODE-BLOCK-FILTER: `latin`, `cyrillic`, ...
                                GREP-CHAR: grep for GREP-CHAR ASCII-code.
                                See `--list-encodings` for more detail.
 -g ASCII, --grep-char=ASCII    Grep for characters with ASCII-code in output lines.
 -h, --help                     Display this message.
 -l, --list-encodings           List predefined encoding and filter names for ENC.
 -n NUM, --chars-min=NUM        Minimum characters of printed strings (default: ",
    chars_min_default!(),
    ").
 -p FILE, --output=FILE         Print not to stdout but in file.
 -q NUM, --output-line-len=NUM  Output line length in UTF-8 characters (default: ",
    output_line_char_nb_max_default!(),
    ").
 -s NUM, --counter-offset=NUM   Start counting input bytes with NUM (default: ",
    counter_offset_default!(),
    ").

 -t RADIX, --radix=RADIX        Enable byte-counter with radix `o`, `x` or `d`.
 -u UBF, --unicode-block-filter=UBF 
                                Unicode-block-filter UBF applied after decoding.
                                See `--list-encodings` for UBF examples.
 -V, --version                  Print version and exit.
"
);

/// This structure holds the command-line-options and is populated by `docopt`.
/// See man-page and the output of `--list-encodings` and `--help` for more
/// information about their meaning.
#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct Args {
    pub flag_ascii_filter: Option<String>,
    pub flag_no_metadata: bool,
    pub flag_debug_options: bool,
    pub arg_FILE: Vec<String>,
    pub flag_encoding: Vec<String>,
    pub flag_grep_char: Option<String>,
    pub flag_list_encodings: bool,
    pub flag_chars_min: Option<String>,
    pub flag_output: Option<String>,
    pub flag_output_line_len: Option<String>,
    pub flag_counter_offset: Option<String>,
    pub flag_radix: Option<Radix>,
    pub flag_unicode_block_filter: Option<String>,
    pub flag_version: bool,
}

/// Radix of the `byte-counter` when printed.
#[derive(PartialEq, Debug, Deserialize)]
pub enum Radix {
    /// octal
    O,
    /// hexadecimal
    X,
    /// decimal
    D,
}

lazy_static! {
    /// Static `Args` stuct.
    pub static ref ARGS : Args = Docopt::new(USAGE)
                            .and_then(|d| d.deserialize())
                            .unwrap_or_else(|e| e.exit());

}

#[cfg(test)]
mod tests {

    /// Are the command-line option read and processed correctly?
    #[test]
    fn test_arg_parser() {
        use super::{Args, Radix, USAGE};
        use docopt::Docopt;

        // The argv. Normally you'd just use `parse` which will automatically
        // use `std::env::args()`.
        let argv = || {
            vec![
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
                "-p",
                "outfile",
                "-q",
                "40",
                "-s",
                "1500",
                "-t",
                "o",
                "infile1",
                "infile2",
            ]
        };
        let args: Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(argv().into_iter()).deserialize())
            .unwrap_or_else(|e| e.exit());

        fn s(x: &str) -> String {
            x.to_string()
        }
        assert_eq!(args.arg_FILE[0], "infile1".to_string());
        assert_eq!(args.arg_FILE[1], "infile2".to_string());
        assert_eq!(args.flag_debug_options, true);
        assert_eq!(args.flag_encoding, vec![s("ascii"), s("utf-8")]);
        assert_eq!(args.flag_version, true);
        assert_eq!(args.flag_list_encodings, true);
        assert_eq!(args.flag_chars_min, Some("10".to_string()));
        assert_eq!(args.flag_grep_char, Some("64".to_string()));
        assert_eq!(args.flag_radix, Some(Radix::O));
        assert_eq!(args.flag_counter_offset, Some("1500".to_string()));
        assert_eq!(args.flag_output, Some(s("outfile")));
        assert_eq!(args.flag_output_line_len, Some("40".to_string()));
        assert_eq!(args.flag_no_metadata, false);
    }
}
