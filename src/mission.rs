//! Parse and convert command-line-arguments into static `MISSION` structures,
//! that are mainly used to initialize `ScannerState`-objects.

#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

extern crate anyhow;
extern crate encoding_rs;
use crate::input::ByteCounter;
use crate::options::ARGS;
use crate::options::ASCII_ENC_LABEL;
use crate::options::CHARS_MIN_DEFAULT;
use crate::options::COUNTER_OFFSET_DEFAULT;
use crate::options::ENCODING_DEFAULT;
use crate::options::OUTPUT_LINE_CHAR_NB_MAX_DEFAULT;
use crate::options::OUTPUT_LINE_CHAR_NB_MIN;
use anyhow::{anyhow, Context, Result};
use encoding_rs::*;
use lazy_static::lazy_static;
use std::cmp;
use std::cmp::{Eq, Ord};
use std::fmt;
use std::ops::Deref;
use std::process;
use std::str;
use std::str::FromStr;

/// A filter for ASCII encoding searches only. No control character pass, but
/// whitespace is allowed. This works like the traditional `stringsext`mode.
/// Unless otherwise specified on the command line, his filter is default for
/// ASCII-encoding searches.
pub const UTF8_FILTER_ASCII_MODE_DEFAULT: Utf8Filter = Utf8Filter {
    af: AF_ALL & !AF_CTRL,
    ubf: UBF_NONE,
    grep_char: None,
};

/// A default filter for all non-ASCII encoding searches.
/// For single-byte-characters (`af`-filter), no control character
/// pass, but whitespace is allowed. This works like the traditional
/// `stringsext`mode.
/// For multi-byte-characters we allow only Latin characters
/// with all kind of accents.
/// Unless otherwise specified on the command line, this filter
/// is default for non-ASCII-encoding searches.
pub const UTF8_FILTER_NON_ASCII_MODE_DEFAULT: Utf8Filter = Utf8Filter {
    af: AF_ALL & !AF_CTRL,
    ubf: UBF_COMMON,
    grep_char: None,
};

/// A filter that let pass all valid Unicode codepoints.
/// Useful for debugging.
#[cfg(test)]
pub const UTF8_FILTER_ALL_VALID: Utf8Filter = Utf8Filter {
    af: AF_ALL,
    ubf: UBF_ALL & !UBF_INVALID,
    grep_char: None,
};

/// A filter for Latin and accents.
/// Useful for debugging.
#[cfg(test)]
pub const UTF8_FILTER_LATIN: Utf8Filter = Utf8Filter {
    af: AF_ALL & !AF_CTRL | AF_WHITESPACE,
    ubf: UBF_LATIN | UBF_ACCENTS,
    grep_char: None,
};
/// Unicode-block-filter:
/// No leading bytes are filtered.
#[allow(dead_code)]
pub const UBF_ALL_VALID: u64 = UBF_ALL & !UBF_INVALID;
/// Unicode-block-filter:
/// A filter that let pass all valid Unicode codepoints, except for ASCII where
/// it behaves like the original `strings`. No leading bytes are filtered.
#[allow(dead_code)]
pub const UBF_ALL: u64 = 0xffff_ffff_ffff_ffff;
/// Unicode-block-filter:
/// No leading byte > 0x7F is accepted.
/// Therefor no multi-byte-characters in UTF-8, which means
/// this is an ASCII-filter.
#[allow(dead_code)]
pub const UBF_NONE: u64 = 0x0000_0000_0000_0000;
/// Unicode-block-filter:
/// These leading bytes are alway invalid in UTF-8
#[allow(dead_code)]
pub const UBF_INVALID: u64 = 0xffe0_0000_0000_0003;
/// Unicode-block-filter:
/// Latin: (U+80..U+240).
/// Usually used together with `UBF_ACCENTS`.
#[allow(dead_code)]
pub const UBF_LATIN: u64 = 0x0000_0000_0000_01fc;
/// Unicode-block-filter:
/// Accents: (U+300..U+380).
#[allow(dead_code)]
pub const UBF_ACCENTS: u64 = 0x0000_0000_0000_3000;
/// Unicode-block-filter:
/// Greek: (U+380..U+400).
#[allow(dead_code)]
pub const UBF_GREEK: u64 = 0x0000_0000_0000_C000;
/// Unicode-block-filter:
/// IPA: (U+240..U+300).
#[allow(dead_code)]
pub const UBF_IPA: u64 = 0x0000_0000_0000_0700;
/// Unicode-block-filter:
/// Cyrillic: (U+400..U+540)
#[allow(dead_code)]
pub const UBF_CYRILLIC: u64 = 0x0000_0000_001f_0000;
/// Unicode-block-filter:
/// Armenian: (U+540..U+580)
#[allow(dead_code)]
pub const UBF_ARMENIAN: u64 = 0x0000_0000_0020_0000;
/// Unicode-block-filter:
/// Hebrew: (U+580..U+600)
#[allow(dead_code)]
pub const UBF_HEBREW: u64 = 0x0000_0000_00c0_0000;
/// Unicode-block-filter:
/// Arabic: (U+600..U+700, U+740..U+780)
#[allow(dead_code)]
pub const UBF_ARABIC: u64 = 0x0000_0000_2f00_0000;
/// Unicode-block-filter:
/// Syriac: (U+700..U+740)
#[allow(dead_code)]
pub const UBF_SYRIAC: u64 = 0x0000_0000_1000_0000;
/// Unicode-block-filter:
/// Armenian: (U+0540..), Hebrew: (U+0580..), Arabic: (U+0600..),
/// Syriac: (U+0700..), Arabic: (U+0740..), Thaana: (U+0780..), N'Ko: (U+07C0..U+800)
#[allow(dead_code)]
pub const UBF_AFRICAN: u64 = 0x0000_0000_ffe0_0000;
/// Unicode-block-filter:
/// All 2-byte UFT-8 (U+07C0..U+800)
/// #[allow(dead_code)]
pub const UBF_COMMON: u64 = 0x0000_0000_ffff_fffc;
/// Unicode-block-filter:
/// Kana: (U+3000..U+4000).
#[allow(dead_code)]
pub const UBF_KANA: u64 = 0x0000_0008_0000_0000;
/// Unicode-block-filter:
/// CJK: (U+3000..A000).
#[allow(dead_code)]
pub const UBF_CJK: u64 = 0x0000_03f0_0000_0000;
/// Unicode-block-filter:
/// Hangul: (U+B000..E000).
#[allow(dead_code)]
pub const UBF_HANGUL: u64 = 0x0000_3800_0000_0000;
/// Unicode-block-filter:
/// Kana: (U+3000..), CJK: (U+4000..), Asian: (U+A000..), Hangul: (U+B000..U+E000).
#[allow(dead_code)]
pub const UBF_ASIAN: u64 = 0x0000_3ffc_0000_0000;
/// Unicode-block-filter:
/// Private use area (U+E00..F00), (U+10_0000..U+14_0000).
#[allow(dead_code)]
pub const UBF_PUA: u64 = 0x0010_4000_0000_0000;
/// Unicode-block-filter:
/// Misc: (U+1000..), Symbol:(U+2000..U+3000), Forms:(U+F000..U+10000).
#[allow(dead_code)]
pub const UBF_MISC: u64 = 0x0000_8006_0000_0000;
/// Unicode-block-filter:
/// Besides PUA, more very uncommon planes: (U+10_000-U+C0_000).
#[allow(dead_code)]
pub const UBF_UNCOMMON: u64 = 0x000f_0000_0000_0000;

/// Shortcuts for the hexadecimal representation of a unicode block filter.
/// The array is defined as `(key, value)` tuples.
/// For value see chapter *Codepage layout* in
/// [UTF-8 - Wikipedia](https://en.wikipedia.org/wiki/UTF-8)
pub const UNICODE_BLOCK_FILTER_ALIASSE: [([u8; 12], u64, [u8; 25]); 18] = [
    (*b"African     ", UBF_AFRICAN, *b"all in U+540..U+800      "),
    (
        *b"All-Asian   ",
        UBF_ALL & !UBF_INVALID & !UBF_ASIAN,
        *b"all, except Asian        ",
    ),
    (
        *b"All         ",
        UBF_ALL & !UBF_INVALID,
        *b"all valid multibyte UTF-8",
    ),
    (
        *b"Arabic      ",
        UBF_ARABIC | UBF_SYRIAC,
        *b"Arabic+Syriac            ",
    ),
    (
        *b"Armenian    ",
        UBF_ARMENIAN,
        *b"Armenian                 ",
    ),
    (*b"Asian       ", UBF_ASIAN, *b"all in U+3000..U+E000    "),
    (*b"Cjk         ", UBF_CJK, *b"CJK: U+4000..U+A000      "),
    (*b"Common      ", UBF_COMMON, *b"all 2-byte-UFT-8         "),
    (
        *b"Cyrillic    ",
        UBF_CYRILLIC,
        *b"Cyrillic                 ",
    ),
    (
        *b"Default     ",
        UBF_ALL & !UBF_INVALID,
        *b"all valid multibyte UTF-8",
    ),
    (*b"Greek       ", UBF_GREEK, *b"Greek                    "),
    (*b"Hangul      ", UBF_HANGUL, *b"Hangul: U+B000..U+E000   "),
    (*b"Hebrew      ", UBF_HEBREW, *b"Hebrew                   "),
    (*b"Kana        ", UBF_KANA, *b"Kana: U+3000..U+4000     "),
    (
        *b"Latin       ",
        UBF_LATIN | UBF_ACCENTS,
        *b"Latin + accents          ",
    ),
    (*b"None        ", !UBF_ALL, *b"block all multibyte UTF-8"),
    (*b"Private     ", UBF_PUA, *b"private use areas        "),
    (
        *b"Uncommon    ",
        UBF_UNCOMMON | UBF_PUA,
        *b"private + all>=U+10_000  ",
    ),
];

/// ASCII filter:
/// Let all ASCII pass the filter (0x01..0x100)
/// except Null (0x00) which is "end of string" marker.
/// [Null character - Wikipedia](https://en.wikipedia.org/wiki/Null_character)
#[allow(dead_code)]
pub const AF_ALL: u128 = 0xffff_ffff_ffff_ffff_ffff_ffff_ffff_fffe;

/// ASCII filter:
/// Nothing passes ASCII pass filter
#[allow(dead_code)]
pub const AF_NONE: u128 = 0x0000_0000_0000_0000_0000_0000_0000_0000;

/// ASCII filter:
/// Controls: (0x00..0x20, 0x7F)
/// [C0 and C1 control codes - Wikipedia](<https://en.wikipedia.org/wiki/C0_and_C1_control_codes>)
/// Unlike traditional `strings` we exclude "Space" (0x20) here, as
/// it can appear in filenames. Instead, we consider "Space" to be
/// a regular character.
#[allow(dead_code)]
pub const AF_CTRL: u128 = 0x8000_0000_0000_0000_0000_0000_ffff_ffff;

/// ASCII filter:
/// White-space
/// (0x09..=0x0c, 0x20)
/// [C0 and C1 control codes - Wikipedia](<https://en.wikipedia.org/wiki/C0_and_C1_control_codes>)
/// It do not include "Carriage Return" (0x0d) here. This way strings are
/// divided into shorter chunks and we get more location information.
#[allow(dead_code)]
pub const AF_WHITESPACE: u128 = 0x0000_0000_0000_0000_0000_0001_0000_1e00;

/// ASCII filter:
/// Set defaults close to those in traditional `strings`.
#[allow(dead_code)]
pub const AF_DEFAULT: u128 = AF_ALL & !AF_CTRL;

pub const ASCII_FILTER_ALIASSE: [([u8; 12], u128, [u8; 25]); 6] = [
    (*b"All         ", AF_ALL, *b"all ASCII = pass all     "),
    (
        *b"All-Ctrl    ",
        AF_ALL & !AF_CTRL,
        *b"all-control              ",
    ),
    (
        *b"All-Ctrl+Wsp",
        AF_ALL & !AF_CTRL | AF_WHITESPACE,
        *b"all-control+whitespace   ",
    ),
    (*b"Default     ", AF_DEFAULT, *b"all-control              "),
    (*b"None        ", AF_NONE, *b"block all 1-byte UTF-8   "),
    (
        *b"Wsp         ",
        AF_WHITESPACE,
        *b"only white-space         ",
    ),
];

lazy_static! {
    pub static ref MISSIONS: Missions = Missions::new(
        ARGS.counter_offset.as_ref(),
        &ARGS.encoding,
        ARGS.chars_min.as_ref(),
        ARGS.same_unicode_block,
        ARGS.ascii_filter.as_ref(),
        ARGS.unicode_block_filter.as_ref(),
        ARGS.grep_char.as_ref(),
        ARGS.output_line_len.as_ref(),
    )
    .unwrap_or_else(|error| {
        eprintln!("Error while parsing command-line arguments: {:?}", error);
        process::exit(1);
    });
}

/// When the decoder finds a valid Unicode character, it decodes it into UTF-8.
/// The leading byte of this UTF-8 multi-byte-character must then pass an
/// additional filter before being printed: the so called `Utf8Filter`. It comes
/// with three independant filter criteria:
///
/// 1. The Ascii-Filter `Utf8Filter::asf`,
/// 2. the Unicode-block-filter `Utf8Filter::ubf`,
/// 3. and the `Utf8::must_hame`-filter.
///
/// The Ascii-Filter `Utf8Filter::asf` and the Unicode-block-filter
/// `Utf8Filter::ubf` are implemented by the `Utf8Filter::pass_filter()`
/// function. The `Utf8::grep_char`-filter is implemented by the
/// `helper::SplitStr::next()` iterator function.

#[derive(Eq, PartialEq, Copy, Clone)]
pub struct Utf8Filter {
    /// Every bit `0..=127` of the `Utf8Filer::af` filter parameter maps to one
    /// ASCII-code-position `0x00..=0x7F` that is checked by `pass_filter()`
    /// against the UTF-8 leading byte of the incoming stream. For example if the
    /// leading byte's code is 32 and the `Utf8Filter::af` has bit number 32 set,
    /// then the character passes the filter. If not, it is rejected.
    pub af: u128,

    /// Every bit `0..=63` maps to one leading-byte's code position
    /// `0xC0..0xFF`, e.g. bit 0 is set -> all characters with leading byte `0xC0`
    /// pass the filter,
    /// If bit 1 is set -> all characters with all leading byte `0xC1`, ...
    /// pass the filter. Otherwise, the character is rejected.
    pub ubf: u64,

    /// If `Some()`, a finding must have at least one leading byte equal to the
    /// `grep_char` ASCII code. This is useful when you grep for path-strings:
    /// e.g. "0x2f" or "0x5c".
    pub grep_char: Option<u8>,
}

impl Utf8Filter {
    /// This function applies the Ascii-Filter `Utf8Filter::asf` to the
    /// UTF-8 leading byte `b`. It assumes that `b<=0x7f`!
    #[inline]
    pub fn pass_af_filter(&self, b: u8) -> bool {
        debug_assert!(b & 0x80 == 0x00);
        // We treat b values 0-128 here.
        1 << b & self.af != 0
    }
    /// This function applies the Unicode-Block-Filter `Utf8Filter::ubf` to the
    /// UTF-8 leading byte `b`. It assumes that `b>0x7f`!
    #[inline]
    pub fn pass_ubf_filter(&self, b: u8) -> bool {
        debug_assert!(b & 0x80 == 0x80);
        // We do not have to check for invalid continuation-bytes here, because we know the
        // input is valid UTF-8 and therefor the continuation-byte-codes `0x80..0xBF` can not
        // appear here. We treat b values of 192-255 here (128-191 can not occur in leading
        // UTF-8 bytes). We first map values 192-255 -> 0-128 with (b & 0x3f)
        1 << (b & 0x3f) & self.ubf != 0
    }
}

impl fmt::Debug for Utf8Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "af: 0x{:x}, ubf: 0x{:x}, grep_char: {:?}",
            self.af, self.ubf, self.grep_char
        )
    }
}

/// Needed for merging.
impl PartialOrd for Utf8Filter {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Needed for merging.
impl Ord for Utf8Filter {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self.ubf != other.ubf {
            self.ubf.cmp(&other.ubf)
        } else {
            (!self.af).cmp(&!other.af)
        }
    }
}

/// `Mission` represents the instruction parameters used mainly in `scanner::scan()`.
/// Each thread gets its own instance and stores it in `ScannerState`.
#[derive(Debug, Clone)]
pub struct Mission {
    /// An identifier for this mission. We use its position index in the
    /// `Missions.v` vector.
    pub mission_id: u8,

    /// Start offset for the input-stream-byte-counter. This is useful in case
    /// the input comes split in separate files, that should be analyzed with
    /// separate `stringsext` runs. Note: in general it is better to treat all
    /// input in one `stringsext` run and provide all split input-files as
    /// command-line-parameter for one `stringsext` run. This way `stringsext`
    /// can concatenate the split input files and is able to recognize split
    /// strings at the cutting edge between two input files.
    pub counter_offset: ByteCounter,
    /// Every thread gets a constant encoding to search for.
    ///
    pub encoding: &'static Encoding,

    /// Minimum required string length in Bytes for a finding to be printed.
    pub chars_min_nb: u8,

    /// When true imposes an addition condition for findings:
    /// Advises the filter to only accept multi-characters in a finding with
    /// the same leading byte. This does not affect 1-byte ASCII characters.
    pub require_same_unicode_block: bool,

    /// A filter, defining additional criteria for a finding to be printed.
    pub filter: Utf8Filter,

    /// Maximum length of output-lines in UTF-8 characters. Findings that do not
    /// fit, will be wrapped to two or more lines. The label `+` indicates that
    /// this line is the continuation of the previous line.
    pub output_line_char_nb_max: usize,

    /// The `encoding_rs` decoder has no direct support for ASCII. As a
    /// workaround, we simulate the missing ASCII-decoder with the
    /// `x-user-defined`-decoder and a special filter. With this flag is set, we
    /// indicate this case. It is later used to print out the label `ascii`
    /// instead of `x-user-defined`.
    pub print_encoding_as_ascii: bool,
}

/// A collection to bundle all `Mission`-objects.
#[derive(Debug)]
pub struct Missions {
    /// Vector of `Mission`s.
    pub v: Vec<Mission>,
}

/// Access `Mission` without `.v`.
impl Deref for Missions {
    type Target = Vec<Mission>;

    fn deref(&self) -> &Self::Target {
        &self.v
    }
}

/// Parses a filter expression from some hexadecimal string or
/// number string to an integer value.
///
/// `$s` is `Option<String>` to be parsed.
/// `$x_from_str_radix` is either `u128::from_str_radix` or u64::from_str_radix`.
/// `$x_from_str` is e.g. `u8::from_str` or usize::from_str`, ...
///
///  The marco returns a filter integer value in `Option<integer>` and
///  returns early when parsing is not successful.
#[macro_export]
macro_rules! parse_integer {
    ($s:expr, $x_from_str_radix:expr, $x_from_str:expr) => {{
        match $s {
            Some(s) if s.is_empty() => None,
            Some(s) if s.trim().len() >= 2 && s.trim()[..2] == *"0x" => Some(
                $x_from_str_radix(&s.trim()[2..], 16)
                    .with_context(|| format!("failed to parse hexadecimal number: `{}`", s))?,
            ),
            Some(s) => Some(
                $x_from_str(s.trim()).with_context(|| format!("failed to parse number: {}", s))?,
            ),
            None => None,
        }
    }};
}

/// Parses a filter expression from some hexadecimal string or from some
/// filter-alias-name in `$list` to a filter-integer value.
///
/// `$s` is `Option<String>` to be parsed.
/// `$list` is either `ASCII_FILTER_ALIASSE` or `UNICODE_BLOCK_FILTER_ALIASSE`.
/// `$x_from_str_radix` is either `u128::from_str_radix` or u64::from_str_radix`.
///
///  The marco returns a filter integer value in `Option<integer>` and
///  returns early when parsing is not successful.
#[macro_export]
macro_rules! parse_filter_parameter {
    ($s:expr, $x_from_str_radix:expr, $list:expr) => {{
        match $s {
            Some(s) if s.trim().len() >= 2 && s.trim()[..2] == *"0x" => Some(
                $x_from_str_radix(&s.trim()[2..], 16)
                    .with_context(|| format!("failed to parse hexadecimal number: `{}`", s))?,
            ),
            Some(s) if s.is_empty() => None,
            Some(s) => {
                let s = s.trim();
                let mut oubf = None;
                for (ubf_name, ubf, _) in $list.iter() {
                    if s.len() <= ubf_name.len() && *s.as_bytes() == ubf_name[..s.len()] {
                        oubf = Some(*ubf);
                        break;
                    };
                }
                if oubf.is_some() {
                    oubf
                } else {
                    return Err(anyhow!(
                        "filter name `{}` is not valid, try `--list-encodings`",
                        s
                    ));
                }
            }
            None => None,
        }
    }};
}

impl Missions {
    /// As `Mission` does not have its own constructor, the `Missions`
    /// constructor creates all `Mission`-objects in one row and stores them in
    /// some vector `Missions::v`. We guarantee that at least one (default)
    /// `Mission`-object will be created. The initialisation data coming from
    /// `options::ARGS` is completed with default values, then parsed and syntax
    /// checked before creating a `Mission`-object.

    pub fn new(
        flag_counter_offset: Option<&String>,
        flag_encoding: &[String],
        flag_chars_min_nb: Option<&String>,
        flag_same_unicode_block: bool,
        flag_ascii_filter: Option<&String>,
        flag_unicode_block_filter: Option<&String>,
        flag_grep_char: Option<&String>,
        flag_output_line_len: Option<&String>,
    ) -> Result<Self> {
        let flag_counter_offset = parse_integer!(
            flag_counter_offset,
            ByteCounter::from_str_radix,
            ByteCounter::from_str
        );

        let flag_chars_min_nb = parse_integer!(flag_chars_min_nb, u8::from_str_radix, u8::from_str);

        // Parse from `Option<String>` to `Option<u128>`
        let flag_ascii_filter = parse_filter_parameter!(
            flag_ascii_filter,
            u128::from_str_radix,
            ASCII_FILTER_ALIASSE
        );

        // Parse from `Option<String>` to `Option<u64>`
        let flag_unicode_block_filter = parse_filter_parameter!(
            flag_unicode_block_filter,
            u64::from_str_radix,
            UNICODE_BLOCK_FILTER_ALIASSE
        );

        let flag_grep_char = parse_integer!(flag_grep_char, u8::from_str_radix, u8::from_str);
        if let Some(m) = flag_grep_char {
            if m > 127 {
                return Err(anyhow!(
                    "you can only `--grep-char` for ASCII codes < 128, \
                     you tried: `{}`.",
                    m
                ));
            }
        }

        let flag_output_line_len =
            parse_integer!(flag_output_line_len, usize::from_str_radix, usize::from_str);
        if let Some(m) = flag_output_line_len {
            if m < OUTPUT_LINE_CHAR_NB_MIN {
                return Err(anyhow!(
                    "minimum for `--output-line-len` is `{}`, \
                     you tried: `{}`.",
                    OUTPUT_LINE_CHAR_NB_MIN,
                    m
                ));
            }
        }

        let mut v = Vec::new();
        let encoding_default: &[String; 1] = &[ENCODING_DEFAULT.to_string()];

        let enc_iter = if flag_encoding.is_empty() {
            encoding_default.iter()
        } else {
            flag_encoding.iter()
        };

        for (mission_id, enc_opt) in enc_iter.enumerate() {
            let (enc_name, chars_min_nb, filter_af, filter_ubf, filter_grep_char) =
                Self::parse_enc_opt(enc_opt)?;

            // DEFINE DEFAULTS

            let mut enc_name = match enc_name {
                Some(s) => s,
                None => ENCODING_DEFAULT,
            };

            let counter_offset = match flag_counter_offset {
                Some(n) => n,
                None => COUNTER_OFFSET_DEFAULT,
            };

            // If `char_min_nb` is not defined in `enc_opt`
            // use the command-line option.
            let chars_min_nb = match chars_min_nb {
                Some(n) => n,
                None => match flag_chars_min_nb {
                    Some(n) => n,
                    None => CHARS_MIN_DEFAULT,
                },
            };

            let require_same_unicode_block = flag_same_unicode_block;

            let output_line_char_nb_max = match flag_output_line_len {
                Some(n) => n,
                None => OUTPUT_LINE_CHAR_NB_MAX_DEFAULT,
            };

            if output_line_char_nb_max < OUTPUT_LINE_CHAR_NB_MIN {
                return Err(anyhow!(
                    "Scanner {}: \
                     minimum for `--output-line-len` is `{}`, \
                     you tried: `{}`.",
                    char::from((mission_id + 97) as u8),
                    OUTPUT_LINE_CHAR_NB_MIN,
                    output_line_char_nb_max,
                ));
            }

            // "ascii" encoding is missing in "encoding.rs". We emulate it with
            // "x-user-defined" and the `UTF8_FILTER_ASCII_MODE_DEFAULT`-filter,
            // if not otherwise specified.

            let filter_af = filter_af.unwrap_or_else(|| {
                flag_ascii_filter.unwrap_or(if enc_name == ASCII_ENC_LABEL {
                    UTF8_FILTER_ASCII_MODE_DEFAULT.af
                } else {
                    UTF8_FILTER_NON_ASCII_MODE_DEFAULT.af
                })
            });

            let filter_ubf = filter_ubf.unwrap_or_else(|| {
                flag_unicode_block_filter.unwrap_or(if enc_name == ASCII_ENC_LABEL {
                    UTF8_FILTER_ASCII_MODE_DEFAULT.ubf
                } else {
                    UTF8_FILTER_NON_ASCII_MODE_DEFAULT.ubf
                })
            });

            let filter_grep_char = match filter_grep_char {
                Some(f) => Some(f),
                None => match flag_grep_char {
                    Some(f) => Some(f),
                    None => {
                        if enc_name == ASCII_ENC_LABEL {
                            UTF8_FILTER_ASCII_MODE_DEFAULT.grep_char
                        } else {
                            UTF8_FILTER_NON_ASCII_MODE_DEFAULT.grep_char
                        }
                    }
                },
            };

            if let Some(m) = filter_grep_char {
                if m > 127 {
                    return Err(anyhow!(
                        "Scanner {}: \
                         you can only grep for ASCII codes < 128, \
                         you tried: `{}`.",
                        char::from((mission_id + 97) as u8),
                        m
                    ));
                }
            }

            let filter = Utf8Filter {
                af: filter_af,
                ubf: filter_ubf,
                grep_char: filter_grep_char,
            };

            let mut print_encoding_as_ascii = false;
            if enc_name == ASCII_ENC_LABEL {
                print_encoding_as_ascii = true;
                enc_name = "x-user-defined"
            };

            let encoding = &Encoding::for_label((enc_name).as_bytes()).with_context(|| {
                format!(
                    "Scanner {}: \
                     invalid input encoding name `{}`, try flag `--list-encodings`.",
                    char::from((mission_id + 97) as u8),
                    enc_name
                )
            })?;

            v.push(Mission {
                counter_offset,
                encoding,
                chars_min_nb,
                require_same_unicode_block,
                filter,
                output_line_char_nb_max,
                mission_id: mission_id as u8,
                print_encoding_as_ascii,
            });
        }

        Ok(Missions { v })
    }

    /// Return the number of `Mission`s stored.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.v.len()
    }

    /// Helper function to parse `enc_opt`.
    #[inline]
    fn parse_enc_opt(
        enc_opt: &str,
    ) -> Result<
        (
            Option<&str>,
            Option<u8>,
            Option<u128>,
            Option<u64>,
            Option<u8>,
        ),
        anyhow::Error,
    > {
        // Parse ',' separated strings
        let mut i = enc_opt.split_terminator(',');

        let enc_name = match i.next() {
            Some(s) if s.is_empty() => None,
            Some(s) => Some(s.trim()),
            None => None,
        };

        let chars_min_nb = parse_integer!(i.next(), u8::from_str_radix, u8::from_str);

        let filter_af =
            parse_filter_parameter!(i.next(), u128::from_str_radix, ASCII_FILTER_ALIASSE);

        let filter_ubf =
            parse_filter_parameter!(i.next(), u64::from_str_radix, UNICODE_BLOCK_FILTER_ALIASSE);

        let grep_char = parse_integer!(i.next(), u8::from_str_radix, u8::from_str);

        if i.next().is_some() {
            return Err(anyhow!("Too many items in `{}`.", enc_opt));
        }

        Ok((enc_name, chars_min_nb, filter_af, filter_ubf, grep_char))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::Utf8Filter;

    #[test]
    fn test_pass_filter() {
        // We filter Latin 1
        let utf8f = Utf8Filter {
            af: AF_ALL,
            ubf: UBF_LATIN,
            grep_char: None,
        };

        // Check lower bits
        assert!(utf8f.pass_af_filter("A".as_bytes()[0]));
        assert!(!utf8f.pass_ubf_filter("€".as_bytes()[0]));
        // Check upper bits
        // first byte of © in UTF-8 is 0xC2.       0xC2 & 0x80 = bit 0x42
        assert!(utf8f.pass_ubf_filter("©".as_bytes()[0]));
        // first byte of © in UTF-8 is 0xE2.       0xE2 & 0x80 = bit 0x62
        assert!(!utf8f.pass_ubf_filter("€".as_bytes()[0]));
    }

    #[test]
    fn test_enc_opt_parser() {
        assert_eq!(
            super::Missions::parse_enc_opt("ascii").unwrap(),
            (Some("ascii"), None, None, None, None)
        );

        assert_eq!(
            super::Missions::parse_enc_opt("utf-8,10,0x89AB,0xCDEF,0x2f").unwrap(),
            (
                Some("utf-8"),
                Some(10),
                Some(0x89AB),
                Some(0xCDEF),
                Some(0x2f)
            )
        );

        assert_eq!(
            super::Missions::parse_enc_opt("utf-8,10,0x89AB,0xCDEF,211").unwrap(),
            (
                Some("utf-8"),
                Some(10),
                Some(0x89AB),
                Some(0xCDEF),
                Some(211)
            )
        );

        assert_eq!(
            super::Missions::parse_enc_opt(",,,,,").unwrap(),
            (None, None, None, None, None)
        );

        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,0x89AB").unwrap(),
            (Some("ascii"), Some(10), Some(0x89AB), None, None)
        );

        assert!(super::Missions::parse_enc_opt("ascii, 10n").is_err());

        assert!(super::Missions::parse_enc_opt("ascii,10,0x89,0x?B").is_err());

        assert!(super::Missions::parse_enc_opt("ascii,10,0x?9,0xAB").is_err());

        assert!(super::Missions::parse_enc_opt("ascii,1000000000000000000000,0x1,0x2").is_err());

        assert!(super::Missions::parse_enc_opt("ascii,10,0x1,0x2,0x3,0x4").is_err());

        assert!(super::Missions::parse_enc_opt("ascii,10,123").is_err());

        assert!(super::Missions::parse_enc_opt("ascii,10,,123").is_err());

        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,Default").unwrap(),
            (Some("ascii"), Some(10), Some(AF_DEFAULT), None, None)
        );

        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,,Latin").unwrap(),
            (
                Some("ascii"),
                Some(10),
                None,
                Some(UBF_LATIN | UBF_ACCENTS),
                None
            )
        );

        assert!(super::Missions::parse_enc_opt("ascii,10,my-no-encoding").is_err());

        assert!(super::Missions::parse_enc_opt("ascii,10,,my-no-encoding").is_err());

        assert_eq!(
            super::Missions::parse_enc_opt("ascii,10,0x89AB").unwrap(),
            (Some("ascii"), Some(10), Some(0x89AB), None, None)
        );
    }
}
