//! Find encoded strings in some input chunk, apply a filter (defined by a
//! `Mission`-object) and store the filtered strings as UTF-8 in `Finding`-objects.

extern crate encoding_rs;
use crate::as_mut_str_unchecked_no_borrow_check;
use crate::as_str_unchecked_no_borrow_check;
use crate::finding::{Finding, FindingCollection, Precision};
use crate::helper::starts_with_multibyte_char;
use crate::helper::SplitStr;
use crate::input::ByteCounter;
use crate::mission::Mission;
use crate::mission::MISSIONS;
#[cfg(test)]
use crate::mission::{Utf8Filter, AF_ALL, AF_CTRL, AF_WHITESPACE, UBF_LATIN, UBF_NONE};
#[cfg(test)]
use crate::mission::{UTF8_FILTER_ALL_VALID, UTF8_FILTER_LATIN};
use encoding_rs::*;
use std::ops::Deref;
use std::slice;
use std::str;

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

/// Some object that holds the state of the `scanner::scan()` function allowing
/// to process the input stream in batches.
pub struct ScannerState {
    /// It contains all (static) information needed to parametrize the decoding and the
    /// filtering performed by `scanner::scan()`
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
    /// value will be update after a `scan()` run to point to the first not
    /// scanned byte in the input stream.
    pub consumed_bytes: ByteCounter,
}

impl<'a> ScannerState {
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
            last_scan_run_leftover: String::with_capacity(mission.output_line_char_nb_max as usize),
            last_run_str_was_printed_and_is_maybe_cut_str: false,
            consumed_bytes: mission.counter_offset,
        }
    }
}

/// First, scans for valid encoded strings in `input_buffer, then decodes them `
/// using `ss.decoder` to UTF-8 and writes the results as UTF-8 in
/// `fc.output_buffer_bytes`. Finally some filter is applied to the found strings
/// retaining only those who satisfy the filter criteria.\
///
/// * The input of this function is `input_buffer`.
/// * The output of this function is the returned `FindingCollection`.
///
/// The input parameter `input_file_id` is forwarded and stored in each `Finding`
/// of the returned `FindingCollection`.\
/// The function keeps its inner state in
/// `ss.decoder`, `ss.last_scan_run_leftover`,
/// `ss.last_run_str_was_printed_and_is_maybe_cut_str` and `ss.consumed_bytes`.\
/// `ss.mission` is not directly used in this function, but some part of it, the
/// `ss.mission.filter`, is forwarded to the helper function:
/// `helper::SplitStr::next()`.\
/// In case this is the last `input_buffer` of the stream, `last` must be set
/// to correctly flush the `ss.decoder`.

pub fn scan<'a>(
    ss: &mut ScannerState,
    input_file_id: Option<u8>,
    input_buffer: &[u8],
    is_last_input_buffer: bool,
) -> FindingCollection<'a> {
    let mut fc = FindingCollection::new(ss.consumed_bytes);
    // We do not clear `output_buffer_bytes`, we just overwrite.

    // Initialisation
    let mut extra_round = false;
    let mut decoder_input_start = 0usize;
    let mut decoder_input_end;
    let mut decoder_output_start = 0usize;

    // Copy `ScannerState` in `last_window...`
    // Copy last run leftover bytes at the beginning of `output_buffer`.
    let mut last_window_leftover_len = 0usize;
    if !ss.last_scan_run_leftover.is_empty() {
        // TODO:
        // We don't need to copy here, we just rewind temporarily
        // `decoder_output_start` to `ss.last_scan_run_leftover`.
        fc.output_buffer_bytes
            // Make the same space.
            [decoder_output_start..decoder_output_start +  ss.last_scan_run_leftover.len()]
            .copy_from_slice(ss.last_scan_run_leftover.as_bytes());
        // Remember for later use.
        last_window_leftover_len = ss.last_scan_run_leftover.len();
        ss.last_scan_run_leftover.clear();
        // Make the decoder write behind the insertion.
        decoder_output_start += last_window_leftover_len;
    }
    let mut last_window_str_was_printed_and_is_maybe_cut_str =
        ss.last_run_str_was_printed_and_is_maybe_cut_str;

    // In many encodings (e.g. UTF16), to fill one `output_line` we need more bytes of input.
    // If ever the string gets longer than `output_line_char_nb_max`, `SplitStr` will wrap the line.
    let decoder_input_window = 2 * ss.mission.output_line_char_nb_max;
    let mut is_last_window = false;

    // iterate over `input_buffer with ``decoder_input_window`-sized slices.
    'input_window_loop: while decoder_input_start < input_buffer.len() {
        decoder_input_end = match decoder_input_start + decoder_input_window {
            n if n < input_buffer.len() => n, // There are at least one byte more left in `input_buffer`.
            _ => {
                is_last_window = true;
                input_buffer.len()
            }
        };

        // Decode one `input_window`, go as far as you can, then loop again.
        'decoder: loop {
            let output_buffer_slice: &mut str = as_mut_str_unchecked_no_borrow_check!(
                &mut fc.output_buffer_bytes[decoder_output_start..]
            );
            let (decoder_result, decoder_read, decoder_written) =
                ss.decoder.decode_to_str_without_replacement(
                    &input_buffer[decoder_input_start..decoder_input_end],
                    output_buffer_slice,
                    extra_round,
                );

            // If the assumption is wrong we change later.
            let mut position_precision = Precision::Exact;

            // Regardless of whether the intermediate buffer got full
            // or the input buffer was exhausted, let's process what's
            // in the intermediate buffer.

            // The target encoding is always UTF-8.
            if decoder_written > 0 {
                // With the following `if`, we check if the previous scan has
                // potentially left some remaining bytes in the Decoder's inner
                // state. This is a complicated corner case, because the inner
                // state of the `encoding_rs` decoder is private and there is
                // yet not method to query if the decoder is in a neutral state.
                // Read the related Issue [Enhancement: get read access to the
                // decoder's inner state · Issue #48 ·
                // hsivonen/encoding_rs](https://github.com/hsivonen/encoding_rs/issues/48)
                //
                // As a workaround, we first check if this is the first round
                // (`decoder_input_start == 0`). Seeing, that we only know the
                // `ByteCounter` precisely at that point and that all other
                // round's findings will be tagged `Precision::After` anyway,
                // there is no need to investigate further in these cases.
                //
                // We can reduce the cases of double decoding by checking if the
                // first decoded character is a multi-byte UTF-8. If yes, this
                // means (in most cases), that no bytes had been stored in the
                // decoder's inner state and therefore we can assume that the
                // first character was found exactly at `decoder_input_start`.
                // If so, we can then tag this string-finding with
                // `Precision::exact`.
                if decoder_input_start == 0 && starts_with_multibyte_char(output_buffer_slice) {
                    // The only way to find out from which scan() run the first
                    // bytes came, is to scan again with a new Decoder and compare
                    // the results.
                    let mut empty_decoder =
                        ss.decoder.encoding().new_decoder_without_bom_handling();
                    // A short buffer on the stack will do.
                    let mut buffer_bytes = [0u8; 8];
                    // This is save, because there are only valid 0 in
                    // `buffer_bytes`.
                    let buffer: &mut str = as_mut_str_unchecked_no_borrow_check!(buffer_bytes[..]);
                    // Alternative code, but slower. let tmp_buffer: &mut str =
                    // std::str::from_utf8_mut(&mut buffer_bytes[..]).unwrap();
                    let (_, _, written) = empty_decoder.decode_to_str_without_replacement(
                        &input_buffer[..],
                        &mut buffer[..],
                        true,
                    );
                    // When the result of the two decoders is not the same, as the
                    // bytes originating from the previous run, we know the extra
                    // bytes come from the previous run. Unfortunately there is no
                    // way to determine how many the decoder had internally stored.
                    // I can be one, two, or three. We only know that the multibyte
                    // sequence started some byte before 0.

                    if (written == 0)
                        || (fc.output_buffer_bytes[0..written] != buffer_bytes[0..written])
                    {
                        position_precision = Precision::Before;
                    }
                }
            }

            // Prepare input for `SplitStr`
            let mut split_str_start = decoder_output_start;
            let split_str_end = decoder_output_start + decoder_written;
            // Enlarge window to the left, to cover not treated bytes again.
            if last_window_leftover_len > 0 {
                // Go some bytes to the left.
                split_str_start -= last_window_leftover_len;
                // We use it only once.
                last_window_leftover_len = 0;
                // We lose precision.
                position_precision = Precision::Before;
            };

            // This is safe because the decoder guarantees us to return only valid UTF-8.
            // We need unsafe code here because the buffer is still borrowed mutably by decoder.
            let split_str_buffer = as_str_unchecked_no_borrow_check!(
                fc.output_buffer_bytes[split_str_start..split_str_end]
            );

            // Another way of saying (decoder_result == DecoderResult::Malformed) ||
            // (is_last_window ...):
            // This can only be `false`, when `split_str_buffer` touches the right boundary (end)
            // of an `input_window`. Normally it `true` because we usually stop at
            // `DecoderResult::Malformed`.
            let invalid_bytes_after_split_str_buffer = (decoder_result
                != DecoderResult::InputEmpty
                && decoder_result != DecoderResult::OutputFull)
                || (is_last_window && is_last_input_buffer);

            // Use it only once.
            let continue_str_if_possible = last_window_str_was_printed_and_is_maybe_cut_str;
            last_window_str_was_printed_and_is_maybe_cut_str = false;

            // Now we split `split_str_buffer` into substrings and store them in
            // vector `fc.v`.

            'chunk_loop: for chunk in SplitStr::new(
                split_str_buffer,
                ss.mission.chars_min_nb,
                continue_str_if_possible,
                invalid_bytes_after_split_str_buffer,
                ss.mission.filter,
                ss.mission.output_line_char_nb_max,
            ) {
                if !chunk.s_is_to_be_filtered_again {
                    // We keep it for printing.
                    fc.v.push(Finding {
                        input_file_id,
                        mission: &ss.mission,
                        position: ss.consumed_bytes + decoder_input_start as ByteCounter,
                        position_precision,
                        s: chunk.s,
                        s_completes_previous_s: chunk.s_completes_previous_s,
                    });

                    last_window_leftover_len = 0;

                    last_window_str_was_printed_and_is_maybe_cut_str = chunk.s_is_maybe_cut;
                } else {
                    // `chunk.s_is_to_be_filtered_again`

                    // This chunk will be inserted at the beginning
                    // of the `output_buffer_bytes` and we do not print it
                    // now. As we will see it (completed to its full
                    // length) again, we can decide later what to do with
                    // it.

                    // As we exactly know where `chunk.s` is located in
                    // `ss.output_buffer_bytes`, it is enough to remember
                    // its length.
                    last_window_leftover_len = chunk.s.len();
                    // As the chunk is not printed now, so we set this
                    // to `false`:
                    last_window_str_was_printed_and_is_maybe_cut_str = false;
                }

                // For all other following `SplitStr` we set this,
                // since we do not know their exact position.
                position_precision = Precision::After;
            }

            decoder_output_start += decoder_written;

            decoder_input_start += decoder_read;

            // Now let's see if we should read again or process the
            // rest of the current input buffer.
            match decoder_result {
                DecoderResult::InputEmpty => {
                    if is_last_window && is_last_input_buffer && !extra_round {
                        extra_round = true;
                    } else {
                        break 'decoder;
                    }
                }
                DecoderResult::OutputFull => {
                    // This should never happen. If ever it does we clear
                    // the the FindingCollection to make more space and
                    // forget all previous findings.
                    fc.clear_and_mark_incomplete();
                    eprintln!("Buffer overflow. Output buffer is too small to receive all decoder data.\
                            Some findings got lost in input {:x}..{:x} from file {:?} for scanner ({})!",
                        ss.consumed_bytes,
                        ss.consumed_bytes + decoder_input_start as ByteCounter,
                        input_file_id,
                        char::from((ss.mission.mission_id + 97) as u8),
                    );
                    decoder_output_start = 0;
                    debug_assert!(
                        true,
                        "Buffer overflow. Output buffer is too small to receive all decoder data."
                    );
                }
                DecoderResult::Malformed(_, _) => {}
            };
        }
    }

    // Store possible leftovers in `ScannerState` for next `scanner::scan()`.
    let last_window_leftover = as_str_unchecked_no_borrow_check!(
        fc.output_buffer_bytes
            [decoder_output_start - last_window_leftover_len..decoder_output_start]
    );
    // Update inner state for next `scan()` run.
    ss.last_scan_run_leftover = String::from(last_window_leftover);
    ss.last_run_str_was_printed_and_is_maybe_cut_str =
        last_window_str_was_printed_and_is_maybe_cut_str;
    ss.consumed_bytes += decoder_input_start as ByteCounter;
    fc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::Mission;
    use lazy_static::lazy_static;

    // To see println!() output in test run, launch
    // cargo test   -- --nocapture

    lazy_static! {
        pub static ref MISSION_ALL_UTF8: Mission = Mission {
            mission_id: 0,
            counter_offset: 10_000,
            print_encoding_as_ascii: false,
            encoding: &Encoding::for_label(("utf-8").as_bytes()).unwrap(),
            chars_min_nb: 3,
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
            encoding: &Encoding::for_label(("utf-8").as_bytes()).unwrap(),
            chars_min_nb: 3,
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
            encoding: &Encoding::for_label(("utf-8").as_bytes()).unwrap(),
            chars_min_nb: 3,
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
            encoding: &Encoding::for_label(("x-user-defined").as_bytes()).unwrap(),
            chars_min_nb: 3,
            filter: UTF8_FILTER_ALL_VALID,
            output_line_char_nb_max: 10,
        };
    }
    lazy_static! {
        pub static ref MISSION_ASCII: Mission = Mission {
            mission_id: 0,
            counter_offset: 10_000,
            print_encoding_as_ascii: false,
            encoding: &Encoding::for_label(("x-user-defined").as_bytes()).unwrap(),
            chars_min_nb: 3,
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
            encoding: &Encoding::for_label(("utf-8").as_bytes()).unwrap(),
            chars_min_nb: 4,
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
        let fc = scan(&mut ss, Some(0), input, true);

        assert_eq!(fc.v[0].position, 10000);
        assert_eq!(fc.v[0].position_precision, Precision::Exact);
        assert_eq!(fc.v[0].s, "a234567890");

        assert_eq!(fc.v[1].position, 10000);
        assert_eq!(fc.v[1].position_precision, Precision::After);
        assert_eq!(fc.v[1].s, "b234567890");

        assert_eq!(fc.v[2].position, 10020);
        assert_eq!(fc.v[2].position_precision, Precision::Exact);
        assert_eq!(fc.v[2].s, "c234");
        assert_eq!(ss.last_run_str_was_printed_and_is_maybe_cut_str, false);

        assert_eq!(fc.first_byte_position, 10000);
        // This should never be true, since `OUTPUT_BUF_LEN` is 2* `INP_BUF_LEN`.
        assert_eq!(fc.str_buf_overflow, false);
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
        let fc = scan(&mut ss, Some(0), input, true);

        assert_eq!(fc.v.len(), 3);
        assert_eq!(fc.first_byte_position, 10000);
        // This should never be true, since `OUTPUT_BUF_LEN` is 2* `INP_BUF_LEN`.
        assert_eq!(fc.str_buf_overflow, false);

        assert_eq!(fc.v[0].position, 10000);
        assert_eq!(fc.v[0].position_precision, Precision::Exact);
        assert_eq!(fc.v[0].s, "a234567890");

        assert_eq!(fc.v[1].position, 10000);
        assert_eq!(fc.v[1].position_precision, Precision::After);
        assert_eq!(fc.v[1].s, "b234567890");

        assert_eq!(fc.v[2].position, 10020);
        assert_eq!(fc.v[2].position_precision, Precision::Exact);
        assert_eq!(fc.v[2].s, "c2");

        assert_eq!(ss.last_run_str_was_printed_and_is_maybe_cut_str, false);
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
        let fc = scan(&mut ss, Some(0), input, false);

        assert_eq!(fc.v[0].position, 10000);
        assert_eq!(fc.v[0].position_precision, Precision::Exact);
        assert_eq!(fc.v[0].s, "You");

        // "co" is not printed, because we do not know if
        // it can be completed by the next run.
        // It will be forwarded to the next run.
        assert_eq!(fc.v.len(), 1);
        assert_eq!(ss.last_scan_run_leftover, "co");

        assert_eq!(fc.first_byte_position, 10000);
        assert_eq!(fc.str_buf_overflow, false);
        assert_eq!(ss.consumed_bytes, 10000 + 8);

        let input = b"me\xC0\x82\xC0home.";
        // True, because last input.
        let fc = scan(&mut ss, Some(0), input, true);

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
        assert_eq!(fc.str_buf_overflow, false);
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
        let fc = scan(&mut ss, Some(0), input, false);

        assert_eq!(fc.v.len(), 0);

        // "co" is not printed, because we do not know if
        // it can be completed by the next run.
        // It will be forwarded to the next run.
        assert_eq!(ss.last_scan_run_leftover, "co");

        assert_eq!(fc.first_byte_position, 10000);
        assert_eq!(fc.str_buf_overflow, false);
        assert_eq!(ss.consumed_bytes, 10000 + 8);

        let input = b"me*\xC0\x82\xC0ho*me.\x82";
        // True, because last input.
        let fc = scan(&mut ss, Some(0), input, true);

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
        assert_eq!(fc.str_buf_overflow, false);
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
        let _fc = scan(&mut ss, Some(0), input, false);

        //println!("{:#?}",fc);

        //second run
        // The first byte is the remaining € sign from the
        // last run.
        let input = b"\xacoh\xC0no no";

        let fc = scan(&mut ss, Some(0), input, false);

        //println!("{:#?}",fc);

        assert_eq!(fc.v[0].position, 10006);
        assert_eq!(fc.v[0].position_precision, Precision::Before);
        assert_eq!(fc.v[0].s, "word€oh");

        assert_eq!(fc.first_byte_position, 10006);
        assert_eq!(fc.str_buf_overflow, false);
        assert_eq!(ss.consumed_bytes, 10006 + 9);

        // Third run.
        // There are no remaining bytes stored in the decoder. The first byte is the beginning
        // of the € sign.
        let input = b"\xe2\x82\xacStream end.";

        let fc = scan(&mut ss, Some(0), input, true);

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
        assert_eq!(fc.str_buf_overflow, false);
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

        let fc = scan(&mut ss, Some(0), input, false);

        //println!("{:#?}", fc.v);

        assert_eq!(fc.first_byte_position, 10000);
        assert_eq!(fc.str_buf_overflow, false);
        assert_eq!(fc.v.len(), 2);

        assert_eq!(fc.v[0].s, "abc");
        assert_eq!(fc.v[0].position, 10003);
        assert_eq!(fc.v[0].position_precision, Precision::Exact);

        // Note that "de" is missing, too short.
        assert_eq!(fc.v[1].s, "fgh");
        assert_eq!(fc.v[1].position, 10011);
        assert_eq!(fc.v[1].position_precision, Precision::Exact);

        assert_eq!(ss.consumed_bytes, 10000 + 18);
        assert_eq!(ss.last_run_str_was_printed_and_is_maybe_cut_str, false);
        assert_eq!(ss.last_scan_run_leftover, "ijk");

        // Second run
        // Only "def" is long enough.
        let input = b"b\xC0\x82c\xC0def";

        let fc = scan(&mut ss, Some(0), input, true);

        //println!("{:#?}", fc.v);

        assert_eq!(fc.first_byte_position, 10018);
        assert_eq!(fc.str_buf_overflow, false);
        assert_eq!(fc.v.len(), 2);

        assert_eq!(fc.v[0].position, 10018);
        assert_eq!(fc.v[0].position_precision, Precision::Before);
        assert_eq!(fc.v[0].s, "ijkb");

        assert_eq!(fc.v[1].position, 10023);
        assert_eq!(fc.v[1].position_precision, Precision::Exact);
        assert_eq!(fc.v[1].s, "def");

        assert_eq!(ss.consumed_bytes, 10018 + 8);
        assert_eq!(ss.last_run_str_was_printed_and_is_maybe_cut_str, false);
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

        let fc = scan(&mut ss, Some(0), input, false);

        //println!("{:#?}", fc.v);

        assert_eq!(fc.first_byte_position, 10000);
        assert_eq!(fc.str_buf_overflow, false);
        assert_eq!(fc.v.len(), 2);

        assert_eq!(fc.v[0].s, "ääà");
        assert_eq!(fc.v[0].position, 10000);
        // This was cat at the edge of `input_window`.
        assert_eq!(fc.v[0].position_precision, Precision::Exact);

        // Note that "de" is missing, too short.
        assert_eq!(fc.v[1].s, "fgh");
        assert_eq!(fc.v[1].position, 10020);
        // This was cat at the edge of `input_window`.
        assert_eq!(fc.v[1].position_precision, Precision::Before);

        assert_eq!(ss.consumed_bytes, 10000 + 31);
        assert_eq!(ss.last_run_str_was_printed_and_is_maybe_cut_str, false);
        assert_eq!(ss.last_scan_run_leftover, "ijk");

        // Second run
        // Only "def" is long enough.
        let input = b"b\xC0\x82c\xC0def";

        let fc = scan(&mut ss, Some(0), input, true);

        //println!("{:#?}", fc.v);

        assert_eq!(fc.first_byte_position, 10031);
        assert_eq!(fc.str_buf_overflow, false);
        assert_eq!(fc.v.len(), 2);

        assert_eq!(fc.v[0].position, 10031);
        assert_eq!(fc.v[0].position_precision, Precision::Before);
        assert_eq!(fc.v[0].s, "ijkb");

        assert_eq!(fc.v[1].position, 10036);
        // This was cat at the edge of `input_window`.
        assert_eq!(fc.v[1].position_precision, Precision::Exact);
        assert_eq!(fc.v[1].s, "def");

        assert_eq!(ss.consumed_bytes, 10031 + 8);
        assert_eq!(ss.last_run_str_was_printed_and_is_maybe_cut_str, false);
        assert_eq!(ss.last_scan_run_leftover, "");
    }

    #[test]
    fn test_ascii_emulation() {
        let m: &'static Mission = &MISSION_ALL_X_USER_DEFINED;

        let mut ss = ScannerState::new(m);

        let input = b"abcdefg\x58\x59\x80\x82h\x83ijk\x89\x90";

        let fc = scan(&mut ss, Some(0), input, true);

        //println!("{:#?}", fc.v);

        assert_eq!(fc.first_byte_position, 10_000);
        assert_eq!(fc.str_buf_overflow, false);
        assert_eq!(fc.v.len(), 2);

        assert_eq!(fc.v[0].position, 10_000);
        assert_eq!(fc.v[0].position_precision, Precision::Exact);
        assert_eq!(fc.v[0].s, "abcdefgXY\u{f780}");
        // Next output line.

        assert_eq!(fc.v[1].position, 10_000);
        assert_eq!(fc.v[1].position_precision, Precision::After);
        assert_eq!(fc.v[1].s, "\u{f782}h\u{f783}ijk\u{f789}\u{f790}");

        assert_eq!(
            // We only compare the first 35 bytes, the others are 0 anyway.
            unsafe { str::from_utf8_unchecked(&fc.output_buffer_bytes[..35]) },
            "abcdefg\u{58}\u{59}\u{f780}\u{f782}h\u{f783}ijk\u{f789}\u{f790}\
             \u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}"
        );

        assert_eq!(ss.consumed_bytes, 10000 + 18);
        // false, because we told the `scan()` this is the last run.
        assert_eq!(ss.last_run_str_was_printed_and_is_maybe_cut_str, false);
        assert_eq!(ss.last_scan_run_leftover, "");

        // Second run.

        let m: &'static Mission = &MISSION_ASCII;

        let mut ss = ScannerState::new(m);

        let input = b"abcdefg\x58\x59\x80\x82h\x83ijk\x89\x90";

        let fc = scan(&mut ss, Some(0), input, false);

        //println!("{:#?}", fc.v);

        assert_eq!(fc.v.len(), 2);
        assert_eq!(fc.first_byte_position, 10000);
        assert_eq!(fc.str_buf_overflow, false);

        assert_eq!(fc.v[0].position, 10_000);
        assert_eq!(fc.v[0].position_precision, Precision::Exact);
        assert_eq!(fc.v[0].s, "abcdefgXY");
        // Next output line.

        assert_eq!(fc.v[1].position, 10_000);
        assert_eq!(fc.v[1].position_precision, Precision::After);
        // Note that `h` is gone.
        assert_eq!(fc.v[1].s, "ijk");

        assert_eq!(
            // We only compare the first 35 bytes, the others are 0 anyway.
            unsafe { str::from_utf8_unchecked(&fc.output_buffer_bytes[..35]) },
            "abcdefg\u{58}\u{59}\u{f780}\u{f782}h\u{f783}ijk\u{f789}\u{f790}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}"
        );

        assert_eq!(ss.consumed_bytes, 10000 + 18);
        assert_eq!(ss.last_run_str_was_printed_and_is_maybe_cut_str, false);
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
        let mut ss = ScannerState::new(&m);

        let input = b"\x00\x00\x00\x00\x40\x00\x38\x00\x0c\x00\x40\x00\x2c\x00\x2b\x00";
        let fc = scan(&mut ss, Some(0), input, false);
        // Test that the bug is gone
        assert_ne!(fc.v.len(), 1);
        //assert_ne!(fc.v[0].s, "+");
    }
}
