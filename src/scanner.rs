//! Find encoded strings in some input chunk, apply a filter (defined by a
//! `Mission`-object) and store the filtered strings as UTF-8 in `Finding`-objects.

extern crate encoding_rs;

use crate::input::ByteCounter;
use crate::mission::Mission;
use crate::mission::MISSIONS;
use encoding_rs::Decoder;
use std::ops::Deref;

/// A vector of `ScannerState` s.
pub struct ScannerStates {
    /// Vector of ScannerState
    pub v: Vec<ScannerState>,
}

impl ScannerStates {
    /// Constructor.
    pub fn new(missions: &'static MISSIONS) -> Self {
        let mut v = Vec::with_capacity(missions.len());
        for i in 0..missions.len() {
            v.push(ScannerState::new(&missions[i]))
        }
        Self { v }
    }
}

/// Access `ScannerState` without `.v`.
impl Deref for ScannerStates {
    type Target = Vec<ScannerState>;

    fn deref(&self) -> &Self::Target {
        &self.v
    }
}

/// Some object that holds the state of the `scanner::FindingCollection::scan()` function allowing
/// to process the input stream in batches.
pub struct ScannerState {
    /// It contains all (static) information needed to parametrize the decoding and the
    /// filtering performed by `scanner::FindingCollection::scan()`
    pub mission: &'static Mission,

    /// The decoder may hold in its internal state, among other
    /// things, some bytes of output, when a multibyte encoder was cut at the end
    /// of a buffer.
    pub decoder: Decoder,

    /// For short strings (`< chars_min_nb`) at the very end of the buffer, we
    /// can not decide immediately, if they have to be printed or not, because we
    /// can not `peek()` into what is coming in the next chunk. Maybe the
    /// beginning of the next chunk completes this short string from the previous
    /// run, and both together are long enough (`>= chars_min_nb`) to be printed?
    pub last_scan_run_leftover: String,

    /// The last printed string touched the right boundary of the buffer, so it
    /// might cut and to be continued with the first string in the next run.
    /// `last_run_str_was_printed_and_is_maybe_cut_str` remembers this fact and
    /// advises the filter to check if the first string of the next run touches
    /// the left boundary of the buffer. If yes, this string will be printed,
    /// whatever length it has.
    pub last_run_str_was_printed_and_is_maybe_cut_str: bool,

    /// This an absolute byte counter counting bytes of the input stream. The
    /// value will be update after a `FindingCollection::scan()` run to point to the first not
    /// scanned byte in the input stream.
    pub consumed_bytes: ByteCounter,
}

impl ScannerState {
    /// Constructor.
    pub fn new(mission: &'static Mission) -> Self {
        Self {
            mission,
            decoder: mission.encoding.new_decoder_without_bom_handling(),
            //
            // We keep only short substrings for the next run, because about all
            // longer ones we can decide immediately.
            // `mission.chars_min_nb` is enough space, we never need more.
            // We multiply `mission.chars_min_nb` by 4, because it is
            // counted Unicode-codepoints and a codepoint can have
            // maximum 4 bytes in UTF-8.
            last_scan_run_leftover: String::with_capacity(mission.output_line_char_nb_max),
            last_run_str_was_printed_and_is_maybe_cut_str: false,
            consumed_bytes: mission.counter_offset,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::finding::Precision;
    use crate::finding_collection::FindingCollection;
    use crate::mission::Mission;
    use crate::mission::{Utf8Filter, AF_ALL, AF_CTRL, AF_WHITESPACE, UBF_LATIN, UBF_NONE};
    use crate::mission::{UTF8_FILTER_ALL_VALID, UTF8_FILTER_LATIN};
    use encoding_rs::Encoding;
    use lazy_static::lazy_static;

    // To see println!() output in test run, launch
    // cargo test   -- --nocapture

    lazy_static! {
        pub static ref MISSION_ALL_UTF8: Mission = Mission {
            mission_id: 0,
            counter_offset: 10_000,
            print_encoding_as_ascii: false,
            encoding: Encoding::for_label(("utf-8").as_bytes()).unwrap(),
            chars_min_nb: 3,
            require_same_unicode_block: false,
            // this is a pass all filter
            filter: UTF8_FILTER_ALL_VALID,
            output_line_char_nb_max: 10,
        };
    }
    lazy_static! {
        pub static ref MISSION_LATIN_UTF8: Mission = Mission {
            mission_id: 0,
            counter_offset: 10_000,
            print_encoding_as_ascii: false,
            encoding: Encoding::for_label(("utf-8").as_bytes()).unwrap(),
            chars_min_nb: 3,
            require_same_unicode_block: false,
            // this is a pass all filter
            filter: UTF8_FILTER_LATIN,
            output_line_char_nb_max: 10,
        };
    }

    lazy_static! {
        pub static ref MISSION_LATIN_UTF8_GREP42: Mission = Mission {
            mission_id: 0,
            counter_offset: 10_000,
            print_encoding_as_ascii: false,
            encoding: Encoding::for_label(("utf-8").as_bytes()).unwrap(),
            chars_min_nb: 3,
            require_same_unicode_block: false,
            // this is a pass all filter
            filter: Utf8Filter {
                af: AF_ALL & !AF_CTRL | AF_WHITESPACE,
                ubf: UBF_LATIN,
                grep_char: Some(42),
            },
            output_line_char_nb_max: 10,
        };
    }

    lazy_static! {
        pub static ref MISSION_ALL_X_USER_DEFINED: Mission = Mission {
            mission_id: 0,
            counter_offset: 10_000,
            print_encoding_as_ascii: false,
            encoding: Encoding::for_label(("x-user-defined").as_bytes()).unwrap(),
            chars_min_nb: 3,
            require_same_unicode_block: false,
            filter: UTF8_FILTER_ALL_VALID,
            output_line_char_nb_max: 10,
        };
    }
    lazy_static! {
        pub static ref MISSION_ASCII: Mission = Mission {
            mission_id: 0,
            counter_offset: 10_000,
            print_encoding_as_ascii: false,
            encoding: Encoding::for_label(("x-user-defined").as_bytes()).unwrap(),
            chars_min_nb: 3,
            require_same_unicode_block: false,
            // this is a pass all filter
            filter: Utf8Filter {
                af: AF_ALL & !AF_CTRL | AF_WHITESPACE,
                ubf: UBF_NONE,
                grep_char: None,
            },
            output_line_char_nb_max: 10,
        };
    }
    lazy_static! {
        pub static ref MISSION_REAL_DATA_SCAN: Mission = Mission {
            mission_id: 0,
            counter_offset: 10_000,
            print_encoding_as_ascii: false,
            encoding: Encoding::for_label(("utf-8").as_bytes()).unwrap(),
            chars_min_nb: 4,
            require_same_unicode_block: false,
            // this is a pass all filter
            filter: UTF8_FILTER_LATIN,
            output_line_char_nb_max: 60,
        };
    }
    #[test]
    fn test_scan_input_buffer_chunks() {
        // This test uses INP_BUF_LEN=0x20 and
        // OUTPUT_BUF_LEN=0x40.
        // For other parameter see `ALL` above.
        let m: &'static Mission = &MISSION_ALL_UTF8;

        let mut ss = ScannerState::new(m);

        let input = b"a234567890b234567890c234";
        let fc = FindingCollection::from(&mut ss, Some(0), input, true);

        assert_eq!(fc.v[0].position, 10000);
        assert_eq!(fc.v[0].position_precision, Precision::Exact);
        assert_eq!(fc.v[0].s, "a234567890");

        assert_eq!(fc.v[1].position, 10000);
        assert_eq!(fc.v[1].position_precision, Precision::After);
        assert_eq!(fc.v[1].s, "b234567890");

        assert_eq!(fc.v[2].position, 10020);
        assert_eq!(fc.v[2].position_precision, Precision::Exact);
        assert_eq!(fc.v[2].s, "c234");
        assert!(!ss.last_run_str_was_printed_and_is_maybe_cut_str);

        assert_eq!(fc.first_byte_position, 10000);
        // This should never be true, since `OUTPUT_BUF_LEN` is 2* `INP_BUF_LEN`.
        assert!(!fc.str_buf_overflow);
        assert_eq!(ss.consumed_bytes, 10000 + 24);
    }

    #[test]
    fn test_scan_store_in_scanner_state() {
        // This test uses INP_BUF_LEN=0x20 and
        // OUTPUT_BUF_LEN=0x40.
        // For other parameter see `ALL` above.
        let m: &'static Mission = &MISSION_ALL_UTF8;

        let mut ss = ScannerState::new(m);

        let input = b"a234567890b234567890c2";
        // True because this is the only and last input.
        let fc = FindingCollection::from(&mut ss, Some(0), input, true);

        assert_eq!(fc.v.len(), 3);
        assert_eq!(fc.first_byte_position, 10000);
        // This should never be true, since `OUTPUT_BUF_LEN` is 2* `INP_BUF_LEN`.
        assert!(!fc.str_buf_overflow);

        assert_eq!(fc.v[0].position, 10000);
        assert_eq!(fc.v[0].position_precision, Precision::Exact);
        assert_eq!(fc.v[0].s, "a234567890");

        assert_eq!(fc.v[1].position, 10000);
        assert_eq!(fc.v[1].position_precision, Precision::After);
        assert_eq!(fc.v[1].s, "b234567890");

        assert_eq!(fc.v[2].position, 10020);
        assert_eq!(fc.v[2].position_precision, Precision::Exact);
        assert_eq!(fc.v[2].s, "c2");

        assert!(!ss.last_run_str_was_printed_and_is_maybe_cut_str);
        assert_eq!(ss.consumed_bytes, 10000 + 22);
    }

    #[test]
    fn test_split_str_iterator_and_store_in_scanner_state() {
        // This test uses INP_BUF_LEN=0x20 and
        // OUTPUT_BUF_LEN=0x40.
        // For other parameter see `ALL` above.
        // We test UTF-8 as input encoding.
        let m: &'static Mission = &MISSION_ALL_UTF8;

        let mut ss = ScannerState::new(m);

        let input = b"You\xC0\x82\xC0co";
        // `false` because this is not the last input.
        let fc = FindingCollection::from(&mut ss, Some(0), input, false);

        assert_eq!(fc.v[0].position, 10000);
        assert_eq!(fc.v[0].position_precision, Precision::Exact);
        assert_eq!(fc.v[0].s, "You");

        // "co" is not printed, because we do not know if
        // it can be completed by the next run.
        // It will be forwarded to the next run.
        assert_eq!(fc.v.len(), 1);
        assert_eq!(ss.last_scan_run_leftover, "co");

        assert_eq!(fc.first_byte_position, 10000);
        assert!(!fc.str_buf_overflow);
        assert_eq!(ss.consumed_bytes, 10000 + 8);

        let input = b"me\xC0\x82\xC0home.";
        // True, because last input.
        let fc = FindingCollection::from(&mut ss, Some(0), input, true);

        assert_eq!(fc.v.len(), 2);
        assert_eq!(fc.v[0].position, 10008);
        assert_eq!(fc.v[0].position_precision, Precision::Before);
        // Note the "co"!
        assert_eq!(fc.v[0].s, "come");

        assert_eq!(fc.v[1].position, 10013);
        assert_eq!(fc.v[1].position_precision, Precision::Exact);
        assert_eq!(fc.v[1].s, "home.");

        assert_eq!(ss.last_scan_run_leftover, "");

        assert_eq!(fc.first_byte_position, 10008);
        assert!(!fc.str_buf_overflow);
        assert_eq!(ss.consumed_bytes, 10008 + 10);
    }

    #[test]
    fn test_grep_in_scan() {
        // This test uses INP_BUF_LEN=0x20 and
        // OUTPUT_BUF_LEN=0x40.
        // For other parameter see `ALL` above.
        // We test UTF-8 as input encoding.
        let m: &'static Mission = &MISSION_LATIN_UTF8_GREP42;

        let mut ss = ScannerState::new(m);

        let input = b"You\xC0\x82\xC0co";
        // `false` because this is not the last input.
        let fc = FindingCollection::from(&mut ss, Some(0), input, false);

        assert_eq!(fc.v.len(), 0);

        // "co" is not printed, because we do not know if
        // it can be completed by the next run.
        // It will be forwarded to the next run.
        assert_eq!(ss.last_scan_run_leftover, "co");

        assert_eq!(fc.first_byte_position, 10000);
        assert!(!fc.str_buf_overflow);
        assert_eq!(ss.consumed_bytes, 10000 + 8);

        let input = b"me*\xC0\x82\xC0ho*me.\x82";
        // True, because last input.
        let fc = FindingCollection::from(&mut ss, Some(0), input, true);

        assert_eq!(fc.v.len(), 2);
        assert_eq!(fc.v[0].position, 10008);
        assert_eq!(fc.v[0].position_precision, Precision::Before);
        // Note the "co"!
        assert_eq!(fc.v[0].s, "come*");

        assert_eq!(fc.v[1].position, 10014);
        assert_eq!(fc.v[1].position_precision, Precision::Exact);
        assert_eq!(fc.v[1].s, "ho*me.");

        assert_eq!(ss.last_scan_run_leftover, "");

        assert_eq!(fc.first_byte_position, 10008);
        assert!(!fc.str_buf_overflow);
        assert_eq!(ss.consumed_bytes, 10008 + 13);
    }

    #[test]
    /// What happens when a multi-byte UTF-8 is split at the
    /// end of the input buffer between two scan runs?
    fn test_scan_buffer_split_multibyte() {
        // We test UTF-8 as input encoding.
        let m: &'static Mission = &MISSION_ALL_UTF8;

        let mut ss = ScannerState::new(m);

        // One letter more, and we get "OutputFull" because
        // the scanner can not be sure to have enough space.
        // The last bytes are the beginning of a multi-byte
        // character, that is cut between two runs.
        let input = b"word\xe2\x82";

        // This `FindingCollection` is empty.
        let _fc = FindingCollection::from(&mut ss, Some(0), input, false);

        //println!("{:#?}",fc);

        //second run
        // The first byte is the remaining € sign from the
        // last run.
        let input = b"\xacoh\xC0no no";

        let fc = FindingCollection::from(&mut ss, Some(0), input, false);

        //println!("{:#?}",fc);

        assert_eq!(fc.v[0].position, 10006);
        assert_eq!(fc.v[0].position_precision, Precision::Before);
        assert_eq!(fc.v[0].s, "word€oh");

        assert_eq!(fc.first_byte_position, 10006);
        assert!(!fc.str_buf_overflow);
        assert_eq!(ss.consumed_bytes, 10006 + 9);

        // Third run.
        // There are no remaining bytes stored in the decoder. The first byte is the beginning
        // of the € sign.
        let input = b"\xe2\x82\xacStream end.";

        let fc = FindingCollection::from(&mut ss, Some(0), input, true);

        //println!("{:#?}", fc);

        assert_eq!(fc.len(), 2);

        assert_eq!(fc.v[0].position, 10015);
        assert_eq!(fc.v[0].position_precision, Precision::Before);
        assert_eq!(fc.v[0].s, "no no€Stre");
        // Here the line is full.

        assert_eq!(fc.v[1].position, 10015);
        assert_eq!(fc.v[1].position_precision, Precision::After);
        assert_eq!(fc.v[1].s, "am end.");

        assert_eq!(fc.first_byte_position, 10015);
        assert!(!fc.str_buf_overflow);
        assert_eq!(ss.consumed_bytes, 10015 + 14);
    }

    #[test]
    fn test_to_short1() {
        // As `chars_min_nb` is 3, we expect stings with
        // length 1 to be omitted.

        // We test UTF-8 as input encoding.
        let m: &'static Mission = &MISSION_ALL_UTF8;

        let mut ss = ScannerState::new(m);

        let input = b"ii\xC0abc\xC0\xC1de\xC0fgh\xC0ijk";

        let fc = FindingCollection::from(&mut ss, Some(0), input, false);

        //println!("{:#?}", fc.v);

        assert_eq!(fc.first_byte_position, 10000);
        assert!(!fc.str_buf_overflow);
        assert_eq!(fc.v.len(), 2);

        assert_eq!(fc.v[0].s, "abc");
        assert_eq!(fc.v[0].position, 10003);
        assert_eq!(fc.v[0].position_precision, Precision::Exact);

        // Note that "de" is missing, too short.
        assert_eq!(fc.v[1].s, "fgh");
        assert_eq!(fc.v[1].position, 10011);
        assert_eq!(fc.v[1].position_precision, Precision::Exact);

        assert_eq!(ss.consumed_bytes, 10000 + 18);
        assert!(!ss.last_run_str_was_printed_and_is_maybe_cut_str);
        assert_eq!(ss.last_scan_run_leftover, "ijk");

        // Second run
        // Only "def" is long enough.
        let input = b"b\xC0\x82c\xC0def";

        let fc = FindingCollection::from(&mut ss, Some(0), input, true);

        //println!("{:#?}", fc.v);

        assert_eq!(fc.first_byte_position, 10018);
        assert!(!fc.str_buf_overflow);
        assert_eq!(fc.v.len(), 2);

        assert_eq!(fc.v[0].position, 10018);
        assert_eq!(fc.v[0].position_precision, Precision::Before);
        assert_eq!(fc.v[0].s, "ijkb");

        assert_eq!(fc.v[1].position, 10023);
        assert_eq!(fc.v[1].position_precision, Precision::Exact);
        assert_eq!(fc.v[1].s, "def");

        assert_eq!(ss.consumed_bytes, 10018 + 8);
        assert!(!ss.last_run_str_was_printed_and_is_maybe_cut_str);
        assert_eq!(ss.last_scan_run_leftover, "");
    }

    #[test]
    fn test_to_short2() {
        // As `chars_min_nb` is 3, we expect stings with
        // length 1 to be omitted.

        // We test UTF-8 as input encoding.
        let m: &'static Mission = &MISSION_LATIN_UTF8;

        let mut ss = ScannerState::new(m);

        let input = "ii€ääà€€de€fgh€ijk".as_bytes();

        let fc = FindingCollection::from(&mut ss, Some(0), input, false);

        //println!("{:#?}", fc.v);

        assert_eq!(fc.first_byte_position, 10000);
        assert!(!fc.str_buf_overflow);
        assert_eq!(fc.v.len(), 2);

        assert_eq!(fc.v[0].s, "ääà");
        assert_eq!(fc.v[0].position, 10000);
        // This was cut at the edge of `input_window`.
        assert_eq!(fc.v[0].position_precision, Precision::Exact);

        // Note that "de" is missing, too short.
        assert_eq!(fc.v[1].s, "fgh");
        assert_eq!(fc.v[1].position, 10020);
        // This was cut at the edge of `input_window`.
        assert_eq!(fc.v[1].position_precision, Precision::Before);

        assert_eq!(ss.consumed_bytes, 10000 + 31);
        assert!(!ss.last_run_str_was_printed_and_is_maybe_cut_str);
        assert_eq!(ss.last_scan_run_leftover, "ijk");

        // Second run
        // Only "def" is long enough.
        let input = b"b\xC0\x82c\xC0def";

        let fc = FindingCollection::from(&mut ss, Some(0), input, true);

        //println!("{:#?}", fc.v);

        assert_eq!(fc.first_byte_position, 10031);
        assert!(!fc.str_buf_overflow);
        assert_eq!(fc.v.len(), 2);

        assert_eq!(fc.v[0].position, 10031);
        assert_eq!(fc.v[0].position_precision, Precision::Before);
        assert_eq!(fc.v[0].s, "ijkb");

        assert_eq!(fc.v[1].position, 10036);
        // This was cut at the edge of `input_window`.
        assert_eq!(fc.v[1].position_precision, Precision::Exact);
        assert_eq!(fc.v[1].s, "def");

        assert_eq!(ss.consumed_bytes, 10031 + 8);
        assert!(!ss.last_run_str_was_printed_and_is_maybe_cut_str);
        assert_eq!(ss.last_scan_run_leftover, "");
    }

    #[test]
    fn test_field_with_zeros() {
        //  Input-data

        // 00000000  7f 45 4c 46 02 01 01 00  00 00 00 00 00 00 00 00  |.ELF............|
        // 00000010  03 00 3e 00 01 00 00 00  40 51 07 00 00 00 00 00  |..>.....@Q......|
        // 00000020  40 00 00 00 00 00 00 00  c8 c1 46 01 00 00 00 00  |@.........F.....|
        // 00000030  00 00 00 00 40 00 38 00  0c 00 40 00 2c 00 2b 00  |....@.8...@.,.+.|

        // First line in the following output is a bug.
        // ./stringsext -e utf-8 -t X -q 16 ../debug/stringsext

        // <U+FEFF> 30     `+`
        // 2e0    `/lib64/ld-linux-`
        // 2f0+   `x86-64.so.2`
        // 353    `B1(M`

        // We test UTF-8 as input encoding.
        let m: &'static Mission = &MISSION_REAL_DATA_SCAN;
        let mut ss = ScannerState::new(m);

        let input = b"\x00\x00\x00\x00\x40\x00\x38\x00\x0c\x00\x40\x00\x2c\x00\x2b\x00";
        let fc = FindingCollection::from(&mut ss, Some(0), input, false);
        // Test that the bug is gone
        assert_ne!(fc.v.len(), 1);
        //assert_ne!(fc.v[0].s, "+");
    }
}
