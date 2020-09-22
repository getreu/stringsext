extern crate encoding_rs;

use crate::as_mut_str_unchecked_no_borrow_check;
use crate::as_str_unchecked_no_borrow_check;
use crate::finding::Finding;
use crate::finding::Precision;
use crate::finding::OUTPUT_BUF_LEN;
use crate::helper::starts_with_multibyte_char;
use crate::helper::SplitStr;
use crate::input::ByteCounter;
use crate::input::INPUT_BUF_LEN;
use crate::scanner::ScannerState;
use encoding_rs::DecoderResult;
use std::io::Write;
use std::marker::PhantomPinned;
use std::ops::Deref;
use std::pin::Pin;
use std::slice;
use std::str;

/// `FindingCollection` is a set of ordered `Finding` s.
/// The box `output_buffer_bytes` and the struct `Finding` are self-referential,
/// because `Finding.s` points into `output_buffer_bytes`. Therefore, special
/// care is taken that, `output_buffer_bytes` is protected from being moved in
// memory:
/// 1. `output_buffer_bytes` is private.
/// 2. The returned `FindingCollection` is wrapped in a
///    `Pin<Box<FindingCollection>>>`.
#[derive(Debug)]
pub struct FindingCollection<'a> {
    /// `Finding` s in this vector are in chronological order.
    pub v: Vec<Finding<'a>>,
    /// All concurrent `ScannerState::scan()` start at the same byte. All
    /// `Finding.position` refer to `first_byte_position` as zero.
    pub first_byte_position: ByteCounter,
    /// A buffer containing the UTF-8 representation of all findings during one
    /// `Self::from()` run. First, the `Decoder` fills in some UTF-8
    /// string. This string is then filtered. The result of this filtering is
    /// a collection of `Finding`-objects stored in a `FindingCollection`. The
    /// `Finding`-objects have a `&str`-member called `Finding.s` that is
    /// a substring (slice) of `output_buffer_bytes`.
    output_buffer_bytes: Box<[u8]>,
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

    pub fn from<'a>(
        ss: &mut ScannerState,
        input_file_id: Option<u8>,
        input_buffer: &[u8],
        is_last_input_buffer: bool,
    ) -> Pin<Box<FindingCollection<'a>>> {
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
        '_input_window_loop: while decoder_input_start < input_buffer.len() {
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
                        let buffer: &mut str =
                            as_mut_str_unchecked_no_borrow_check!(buffer_bytes[..]);
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

                '_chunk_loop: for chunk in SplitStr::new(
                    split_str_buffer,
                    ss.mission.chars_min_nb,
                    ss.mission.require_same_unicode_block,
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

        // Now we pin the `FindingCollection`.
        Box::pin(fc)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::Precision;
    use crate::finding_collection::FindingCollection;
    use crate::mission::Mission;
    use crate::scanner::tests::MISSION_ALL_X_USER_DEFINED;
    use crate::scanner::tests::MISSION_ASCII;
    use std::str;

    // To see println!() output in test run, launch
    // cargo test   -- --nocapture

    #[test]
    fn test_ascii_emulation() {
        let m: &'static Mission = &MISSION_ALL_X_USER_DEFINED;

        let mut ss = ScannerState::new(m);

        let input = b"abcdefg\x58\x59\x80\x82h\x83ijk\x89\x90";

        let fc = FindingCollection::from(&mut ss, Some(0), input, true);

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
        // false, because we told the `FindingCollection::scan()` this is the last run.
        assert_eq!(ss.last_run_str_was_printed_and_is_maybe_cut_str, false);
        assert_eq!(ss.last_scan_run_leftover, "");

        // Second run.

        let m: &'static Mission = &MISSION_ASCII;

        let mut ss = ScannerState::new(m);

        let input = b"abcdefg\x58\x59\x80\x82h\x83ijk\x89\x90";

        let fc = FindingCollection::from(&mut ss, Some(0), input, false);

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
}
