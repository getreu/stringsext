//! This module contains code needed to convert commandline arguments
//! into a static MISSION structure.

use crate::codec::ascii::ASCII_GRAPHIC;
use crate::options::ControlChars;
use crate::options::ARGS;
use encoding;
use encoding::label::encoding_from_whatwg_label;
use encoding::EncodingRef;
use lazy_static::lazy_static;
use std;
use std::cmp;
use std::cmp::{Eq, Ord};
use std::error;
use std::fmt;
use std::io::prelude::*;
use std::num;
use std::process;
use std::str;
use std::str::FromStr;

lazy_static! {
    pub static ref MISSIONS: Missions = Missions::new(
        &ARGS.flag_encoding,
        &ARGS.flag_control_chars,
        &ARGS.flag_bytes
    );
}

/// When a valid Unicode sequence is found, it must pass additional filter before being
/// printed. One of these filters is `UnicodeBlockFilter`. For performance reasons it is
/// implemented as a bit-mask.
pub struct UnicodeBlockFilter {
    /// Unicode character filter: `if (c && and_mask) == and_result then print c`
    pub and_mask: u32,

    /// Unicode character filter: `if (c && and_mask) == and_result then print c`
    pub and_result: u32,

    /// Is this `and_mask`, `and_result` filtering anything?
    /// This information is redundant because:
    /// `is_some = (and_mask == 0xffe00000) && (and_result == 0x0)`
    /// It is precalculated to speed up later operations.
    pub is_some: bool,
}

#[allow(dead_code)]
impl UnicodeBlockFilter {
    /// This constructs a non-restricting filter letting pass all characters.
    pub fn new() -> Self {
        // This calculates: 0xffe00000
        let and_mask_all = !((std::char::MAX as u32).next_power_of_two() - 1);
        UnicodeBlockFilter {
            and_mask: and_mask_all,
            and_result: 0,
            is_some: false,
        }
    }

    pub fn new2(and_mask: u32, and_result: u32, is_some: bool) -> Self {
        UnicodeBlockFilter {
            and_mask: and_mask,
            and_result: and_result,
            is_some: is_some,
        }
    }
}

impl fmt::Debug for UnicodeBlockFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UnicodeBlockFilter[ and_mask:{:x}, and_result:{:x}, is_some:{:?} ]",
            self.and_mask, self.and_result, self.is_some
        )
    }
}

impl Eq for UnicodeBlockFilter {}

/// Useful to compare findings for debugging or testing.
impl PartialEq for UnicodeBlockFilter {
    fn eq(&self, other: &Self) -> bool {
        (self.and_mask == other.and_mask) && (self.and_result == other.and_result)
    }
}

/// Needed for merging
impl PartialOrd for UnicodeBlockFilter {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        if self.and_result != other.and_result {
            self.and_result.partial_cmp(&other.and_result)
        } else {
            (!self.and_mask).partial_cmp(&!other.and_mask)
        }
    }
}

/// Needed for merging
impl Ord for UnicodeBlockFilter {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self.and_result != other.and_result {
            self.and_result.cmp(&other.and_result)
        } else {
            (!self.and_mask).cmp(&!other.and_mask)
        }
    }
}

/// `Mission` represents the instruction data provided to each thread in
/// `ScannerPool::scan()`.
pub struct Mission {
    /// Every thread gets a constant encoding to search for.
    pub encoding: EncodingRef,

    /// Minimum required string length in Bytes for a finding to be printed.
    pub nbytes_min: u8,

    /// A `Mission` can have up to 2 filters. A strings is printed, when it passes
    /// either `filter1` or `filter2`.
    pub filter1: UnicodeBlockFilter,

    /// A `Mission` can have up to 2 filters. A strings is printed, when it passes
    /// either `filter1` or `filter2`.
    pub filter2: UnicodeBlockFilter,

    /// Some decoders return strings containing also control characters.
    /// These decoders need a special post treatment filtering like:
    /// scanner::filter!()
    pub enable_filter: bool,
}

impl fmt::Debug for Mission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Mission[encoding:{}, filter1:{:?}, filter2:{:?}, nbytes_min:{}, \
             enable_filter:{:?}]",
            self.encoding.name(),
            self.filter1,
            self.filter2,
            self.nbytes_min,
            self.enable_filter
        )
    }
}

// We derive `Debug` because all types should probably derive `Debug`.
// This gives us a reasonable human readable description of `CliError` values.
#[derive(Debug, PartialEq)]
/// Possible errors when parsing command line options
enum CliError {
    /// Wrapper for ParseIntError
    Format(num::ParseIntError),
    /// The `--encoding ` option value can have max 4 comma separated tokens:
    /// ENC == ENCNAME,MIN,UNICODEBLOCK,UNICODEBLOCK
    TooManyTokensError,
}

impl From<num::ParseIntError> for CliError {
    fn from(err: num::ParseIntError) -> CliError {
        CliError::Format(err)
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            // The underlying error already impl `Display`, so we defer to
            // its implementation.
            CliError::Format(ref err) => write!(f, "Format error: {}", err),
            CliError::TooManyTokensError => {
                write!(f, "Format error: too many comma separated tokens")
            }
        }
    }
}

impl error::Error for CliError {
    fn description(&self) -> &str {
        // The underlying error already impl `Error`, so we defer to its
        // implementation.
        match *self {
            CliError::Format(ref err) => err.description(),
            CliError::TooManyTokensError => "too many comma separated tokens",
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            CliError::Format(ref err) => Some(err),
            _ => None,
        }
    }
}

/// Every `--encoding` option is stored in a `Mission` object which are bound together in a
/// `Missions` object. This is used later in `ScannerPool::launch_scanner()` where every
/// scanner-thread has `ScannerState` pointing to the `Mission` of the thread.
#[derive(Debug)]
pub struct Missions {
    /// Vector of `Mission`s.
    pub v: Vec<Mission>,
}

impl Missions {
    /// Constructor. We assume that at least one encoding exist.
    pub fn new(
        encodings: &Vec<String>,
        control_chars: &ControlChars,
        flag_bytes: &Option<u8>,
    ) -> Self {
        let mut v = Vec::new();

        let control_char_filtering = match *control_chars {
            ControlChars::R => true,
            ControlChars::P => false,
            ControlChars::I => true,
        };

        for enc_opt in encodings.iter() {
            let (enc_name, nbytes_min, filter1, filter2) =
                match Self::parse_enc_opt(&enc_opt, flag_bytes.unwrap()) {
                    Ok(r) => r,
                    Err(e) => {
                        writeln!(
                            &mut std::io::stderr(),
                            "Error: {} while parsing `{}`.",
                            e,
                            &enc_opt
                        )
                        .unwrap();
                        process::exit(1);
                    }
                };

            let unicode_filtering = filter1.is_some || filter2.is_some;

            // The next if is a workaround for a bug in EncodingRef:
            // ASCII is translated into windows-1252 instead of pure ASCII.
            if enc_name == "ascii" {
                if *control_chars == ControlChars::I {
                    v.push(Mission {
                        // For ASCII with `-c i` we use our own decoder
                        encoding: ASCII_GRAPHIC as encoding::EncodingRef,
                        nbytes_min: nbytes_min,
                        filter1: filter1,
                        filter2: filter2,
                        enable_filter: unicode_filtering,
                    })
                } else {
                    v.push(Mission {
                        encoding: encoding::all::ASCII as encoding::EncodingRef,
                        nbytes_min: nbytes_min,
                        filter1: filter1,
                        filter2: filter2,
                        enable_filter: control_char_filtering || unicode_filtering,
                    })
                }
                continue;
            }

            v.push(match encoding_from_whatwg_label(enc_name) {
                Some(enc) => Mission {
                    encoding: enc,
                    filter1: filter1,
                    filter2: filter2,
                    nbytes_min: nbytes_min,
                    enable_filter: control_char_filtering || unicode_filtering,
                },
                None => {
                    writeln!(
                        &mut std::io::stderr(),
                        "Error: Invalid input encoding name '{}', try option -l.",
                        enc_name
                    )
                    .unwrap();
                    process::exit(1);
                }
            });
        }

        Missions { v: v }
    }

    /// Return the number of `Mission`s stored.
    pub fn len(&self) -> usize {
        self.v.len()
    }

    /// Helper function to parse enc_opt.
    fn parse_enc_opt(
        enc_opt: &str,
        nbytes_min_default: u8,
    ) -> Result<(&str, u8, UnicodeBlockFilter, UnicodeBlockFilter), CliError> {
        let mask = |(u_lower, u_upper): (u32, u32)| -> UnicodeBlockFilter {
            // CALCULATE FILTER PARAMETERS

            // u_and_mask is 0 from right up to the highest bit that has changed
            let u_changed_bits: u32 = u_upper ^ u_lower;
            let u_next_pow = u_changed_bits.next_power_of_two();
            let u_and_mask = !(u_next_pow - 1);

            // enlarge boundaries to fit u_and_mask
            let u_lower_ext = u_lower & u_and_mask;
            let u_upper_ext = u_upper | !u_and_mask;

            // if enlarged, print a warning
            if !((u_lower == 0) && (u_upper == std::char::MAX as u32))
                && ((u_lower != u_lower_ext) || (u_upper != u_upper_ext))
            {
                writeln!(
                    &mut std::io::stderr(),
                    "Warning: range in `{}` extended to range `U+{:x}..U+{:x}`.",
                    enc_opt,
                    u_lower_ext,
                    u_upper_ext
                )
                .unwrap();
            }

            let u_and_result = u_lower_ext;

            // Check if the filter is restrictive
            // filtering = (and_mask == 0xffe00000) && (and_result == 0x0)
            let filtering = !(u_and_mask == !((std::char::MAX as u32).next_power_of_two() - 1)
                && u_and_result == 0);

            UnicodeBlockFilter {
                and_mask: u_and_mask,
                and_result: u_and_result,
                is_some: filtering,
            }
        };

        let parse_range = |r: &str| -> Result<(u32, u32), CliError> {
            // Separate and parse the range string
            let mut j = r
                .split_terminator("..")
                .map(|s| s.trim_left_matches("U+"))
                .map(|s| u32::from_str_radix(s, 16));

            let u_lower: u32 = j.next().unwrap_or(Ok(0))?;
            let u_upper: u32 = j.next().unwrap_or(Ok(std::char::MAX as u32))?;
            Ok((u_lower, u_upper))
        };

        // Parse ',' separated strings
        let mut i = enc_opt.split_terminator(',');
        let enc_name = i.next().unwrap_or("");
        let opt = i.next();
        let nbytes_min = match opt {
            Some(s) => u8::from_str(s)?,
            None => nbytes_min_default,
        };

        let range1: &str = i.next().unwrap_or("");
        let filter1 = mask(parse_range(range1)?);

        let range2: &str = i.next().unwrap_or("");
        let filter2 = mask(parse_range(range2)?);

        if i.next().is_some() {
            return Err(CliError::TooManyTokensError);
        }

        Ok((enc_name, nbytes_min, filter1, filter2))
    }
}

#[cfg(test)]
mod tests {
    use crate::mission::CliError;
    use crate::mission::UnicodeBlockFilter;
    use std::str::FromStr;

    #[test]
    fn test_enc_opt_parser() {
        //let pie = ParseIntError {kind: std::num::InvalidDigit} //is private
        let pie_invalid_digit = CliError::Format(u32::from_str("x").unwrap_err());
        //let pie = ParseIntError {kind: std::num::Overflow} //is private
        let pie_overflow = CliError::Format(u8::from_str("257").unwrap_err());

        assert_eq!(
            super::Missions::parse_enc_opt("ascii", 8).unwrap(),
            (
                "ascii",
                8,
                UnicodeBlockFilter::new(),
                UnicodeBlockFilter::new()
            )
        );

        // range in `ascii,U+41..U+67` extended to range `U+40..U+7f`
        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,U+41..U+67", 8).unwrap(),
            (
                "ascii",
                10,
                UnicodeBlockFilter::new2(0xffffffc0, 0x40, true),
                UnicodeBlockFilter::new()
            )
        );

        // small letters, range is extended to `U+60..U+7f`
        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,U+61..U+7a", 8).unwrap(),
            (
                "ascii",
                10,
                UnicodeBlockFilter::new2(0xffffffe0, 0x60, true),
                UnicodeBlockFilter::new()
            )
        );

        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,U+4?1..U+67", 8).unwrap_err(),
            pie_invalid_digit
        );

        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,U+41..U+6?7", 8).unwrap_err(),
            pie_invalid_digit
        );

        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,U+4?1..U+67", 8).unwrap_err(),
            pie_invalid_digit
        );

        // range in `ascii,U+401..U+482,10` extended to range `U+400..U+4ff`
        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,U+401..U+482", 8).unwrap(),
            (
                "ascii",
                10,
                UnicodeBlockFilter::new2(0xffffff00, 0x400, true),
                UnicodeBlockFilter::new()
            )
        );

        // range in `ascii,10,U+40e..U+403,10` extended to range `U+400..U+40f`
        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,U+40e..U+403", 8).unwrap(),
            (
                "ascii",
                10,
                UnicodeBlockFilter::new2(0xfffffff0, 0x400, true),
                UnicodeBlockFilter::new()
            )
        );

        assert_eq!(
            super::Missions::parse_enc_opt("ascii,256,U+41..U+67", 8).unwrap_err(),
            pie_overflow
        );

        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,U+fffffffff..U+67", 8).unwrap_err(),
            pie_overflow
        );

        // range in `ascii,10,U+40e..U+403,10` extended to range `U+400..U+40f`
        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,U+0..ff,U+40e..U+403", 8).unwrap(),
            (
                "ascii",
                10,
                UnicodeBlockFilter::new2(0xffffff00, 0x0, true),
                UnicodeBlockFilter::new2(0xfffffff0, 0x400, true)
            )
        );

        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,0..f,10..1f,20..2f", 8).unwrap_err(),
            CliError::TooManyTokensError
        );
    }
}
