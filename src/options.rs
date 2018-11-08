//! This module deals with commandline arguments and related data
//! structures.
use docopt::Docopt;
use lazy_static::lazy_static;
use serde_derive::Deserialize;

#[cfg(test)]
pub const FLAG_BYTES_MAX: usize = 0xff; // max of Args.flag_bytes

/// Help message and string for `Docopt` used to populate the `Args` structure.
const USAGE: &'static str = "
Usage: stringsext [options] [-e ENC...] [--] [FILE...]
       stringsext [options] [-e ENC...] [--] [-]

Options:
 -c MODE, --control-chars=MODE  `p` prints ctrl-chars, `r` replaces with '�'. [default: i]
 -e ENC, --encoding=ENC         Set (multiple) input search encodings. [default: ascii]
                                ENC==ENCNAME[,MIN[,UNICODEBLOCK]]
                                ENCNAME: one of `--list-encodings`.
                                MIN: overwrites general `--bytes MIN` for this ENC only.
                                UNICODEBLOCK: search only for characters in range
                                (defaults to all: U+0..U+10FFFF).
 -f, --print-file-name          Print the name of the file before each string.
 -h, --help                     Display this message.
 -l, --list-encodings           List available encoding-names for ENCNAME.
 -n MIN, --bytes=MIN            Minimum length of printed strings. [default: 4]
 -p FILE, --output=FILE         Print not to stdout but in file.
 -s MIN, --split-bytes=MIN      Minimum length of printed split strings. [default: 1]
 -t RADIX, --radix=RADIX        Enable Byte counter with radix `o`, `x` or `d`.
 -V, --version                  Print version info and exit.
";

/// This structure holds the command-line-options and is populated by `docopt`.
#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct Args {
    /// Pathname of the input data file. `None` defaults to `stdin`.
    pub arg_FILE: Vec<String>,
    /// Do not filter (valid) control chars.
    pub flag_control_chars: ControlChars,
    /// A vector holding encodings to scan for.
    pub flag_encoding: Vec<String>,
    /// Show control characters as  '�' (U+FFFD).
    pub flag_list_encodings: bool,
    /// Print version and exit.
    pub flag_version: bool,
    /// Required minimum length of printed strings in UTF8-Bytes.
    pub flag_bytes: Option<u8>,
    /// Required minimum length of a split strings to be printed.
    pub flag_split_bytes: Option<u8>,
    /// The radix of the Byte counter when printed.
    pub flag_radix: Option<Radix>,
    /// Pathname of the output file. `None` defaults to `stdout`.
    pub flag_output: Option<String>,
    /// Print the name of the file before each string.
    pub flag_print_file_name: bool,
}

/// Mode how to print control characters
#[derive(PartialEq, Debug, Deserialize)]
pub enum ControlChars {
    /// print all valid characters, without filtering
    P,
    /// group and replace control characters with '�' (U+FFFD)
    R,
    /// silently ignore all control characters
    I,
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
    /// Static `Args` stucture.
    // TODO? compose custom error type to improve error messages
    pub static ref ARGS : Args = Docopt::new(USAGE)
                            .and_then(|d| d.deserialize())
                            .unwrap_or_else(|e| e.exit());

}

#[cfg(test)]
mod tests {

    /// Are the command-line option read and processed correctly?
    #[test]
    fn test_arg_parser() {
        use super::{Args, ControlChars, Radix, USAGE};
        use docopt::Docopt;
        // The argv. Normally you'd just use `parse` which will automatically
        // use `std::env::args()`.
        let argv = || {
            vec![
                "stringsext",
                "-c",
                "r",
                "-n",
                "10",
                "-s",
                "11",
                "-e",
                "ascii",
                "-e",
                "utf-8",
                "-V",
                "-l",
                "-p",
                "outfile",
                "-t",
                "o",
                "infile1",
                "infile2",
            ]
        };
        let args: Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(argv().into_iter()).deserialize())
            .unwrap_or_else(|e| e.exit());
        //println!("{:?}",args);

        fn s(x: &str) -> String {
            x.to_string()
        }
        assert_eq!(args.arg_FILE[0], "infile1".to_string());
        assert_eq!(args.arg_FILE[1], "infile2".to_string());
        assert_eq!(args.flag_control_chars, ControlChars::R);
        assert_eq!(args.flag_encoding, vec![s("ascii"), s("utf-8")]);
        assert_eq!(args.flag_version, true);
        assert_eq!(args.flag_list_encodings, true);
        assert_eq!(args.flag_bytes, Some(10u8));
        assert_eq!(args.flag_split_bytes, Some(11u8));
        assert_eq!(args.flag_radix, Some(Radix::O));
        assert_eq!(args.flag_output, Some(s("outfile")));
        assert_eq!(args.flag_print_file_name, false);
    }
}
