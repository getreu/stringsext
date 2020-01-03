//! Small functions of general use, mainly used in module `scanner`.

use crate::mission::Utf8Filter;
#[cfg(test)]
use crate::mission::AF_ALL;
#[cfg(test)]
use crate::mission::UBF_LATIN;
#[cfg(test)]
use crate::mission::UBF_NONE;
use std::slice;
use std::str;

/// This macro is useful for zero-cost conversion from &[u8] to &str. Use
/// this with care. Make sure, that the byte-slice boundaries always fit character
/// boundaries and that the slice only contains valid UTF-8. Also, check for potential
/// race conditions yourself, because this disables borrow checking for
/// `$slice_u8`.
/// This is the immutable version.
#[macro_export]
macro_rules! as_str_unchecked_no_borrow_check {
    ($slice_u8:expr) => {{
        let ptr = $slice_u8.as_ptr();
        let len = $slice_u8.len();
        unsafe { str::from_utf8_unchecked(slice::from_raw_parts(ptr, len)) }
    }};
}

/// This macro is useful for zero-cost conversion from &[u8] to &str. Use
/// this with care. Make sure, that the byte-slice boundaries always fit character
/// boundaries and that the slice only contains valid UTF-8. Also, check for potential
/// race conditions yourself, because this disables borrow checking for
/// `$slice_u8`.
/// This is the mutable version.
#[macro_export]
macro_rules! as_mut_str_unchecked_no_borrow_check {
    ($slice_u8:expr) => {{
        let ptr = $slice_u8.as_mut_ptr();
        let len = $slice_u8.len();
        unsafe { str::from_utf8_unchecked_mut(slice::from_raw_parts_mut(ptr, len)) }
    }};
}

/// A macro useful to reuse an existing buffer while ignoring eventual existing
/// borrows. Make sure that this buffer is not used anymore before applying this!
/// Buffer reuse helps to avoid additional memory-allocations.
#[macro_export]
macro_rules! as_mut_slice_no_borrow_check {
    ($slice_u8:expr) => {{
        let ptr = $slice_u8.as_mut_ptr();
        let len = $slice_u8.len();
        unsafe { slice::from_raw_parts_mut(ptr, len) }
    }};
}

/// This struct defines the state of the iterator `SplitStr`.
#[allow(dead_code)]
pub struct SplitStr<'a> {
    /// The buffer where `next()` searches for substrings satisfying
    /// certain conditions.
    inp: &'a str,

    /// Initially points to the first byte of the `inp`-buffer. In case `ok_s` is
    /// very long and has `>=ok_s_len_max`, the iterator stops and sends out
    /// `ok_s`. Then `inp_start_p` is moved to the first byte after `ok_s` so that
    /// the next `next()` deals with the rest of the string. This way the second
    /// half will be identified to be the continuation of the first part.
    inp_start_p: *const u8,

    /// Points to the first byte after the end of `inp` buffer.
    inp_end_p: *const u8,

    /// `p` walks through `inp` and thus tracks the state of this iterator. After
    /// `next()` it points to the first non-read byte in `inp`.
    p: *const u8,

    /// Criteria that influences the search performed by `next()`. Normally only
    /// substrings larger than `>=chars_min_nb` will be returned by `next()`.
    /// This rule concerning only substrings touching one o fthe `inp` buffer
    /// boundaries has 2 exceptions:
    ///   
    /// 1. When `last_s_was_maybe_cut` is set and
    ///    the substring touches the left boundary of `inp`, the rule is ignored.
    /// 2. When a substring touches the right boundary of `inp`, it is always
    ///    returned, even when it is very short. In this case the rule is ignored
    ///    also. Such a substring tagged `is_s_to_be_filtered_again` when returning.
    chars_min_nb: u8,

    /// The caller informs the iterator, that the last string of the previous run
    /// was maybe cut. When the first substring of this run touches the left
    /// boundary of `inp`, we will tag it `s_completes_previous_s` when
    /// returning. Such a substring is subject to some filter rule exceptions.
    ///
    /// It may also happen, that this flag is `true` in the middle of a run, in
    /// this case indicating, that `SplitStr` has cut a substring at its own
    /// initiative, because the substring was too long to print in one go.
    last_s_was_maybe_cut: bool,

    /// The caller informs us, that beyond no strings can be continued
    /// beyond the right boundary of `inp`, because some invalid bytes
    /// will follow.
    pub invalid_bytes_after_inp: bool,

    /// We keep a reference to `Utf8Filter` here. This is, because `next()` uses
    /// `pass_filter()` to test if a certain leading byte satisfies the filter
    /// criteria. `pass_filter()` evaluates the substring using `Utf8Filter::af`
    /// and `Utf8Filter::ubf`. `Utf8Filter::grep_char` is not passed to
    /// `pass_filter()`. Instead, it is evaluated directly in `next()` and not
    /// forwarded further.
    utf8f: Utf8Filter,

    /// This imposes an additional constraint to the iterator and instructs him
    /// to never return substrings longer than `s_len_max`. Usually this is equal
    /// the `inp`-buffer's length, but there can be exceptions of longer
    /// `inp`-buffers. For example when the previous run has left some
    /// non-treated `left_over` bytes which are then prepended to the
    /// `inp`-buffer. In the worst case, such an `inp` is then twice as large.
    s_len_max: usize,
}

/// This enum describes result variants of the `SplitStr::next()` output.
#[derive(Debug, Eq, PartialEq)]
pub struct SplitStrResult<'a> {
    /// `s` is the main item of the iterator's output. It holds the current
    /// substring that satisfied all filter criteria. It comes with additional
    /// information describing its potential use delivered by the following
    /// flags.
    pub s: &'a str,

    /// The returned substring was found starting at the left buffer boundary. As
    /// the iterator was informed at the beginning, that the last found `s` in
    /// the previous `inp` buffer was of type `s_is_maybe_cut`, we indicate that
    /// this returned substring completes the previous one from last run.
    pub s_completes_previous_s: bool,

    /// The returned substring `&str` touches the right `inp`-buffer boundary and
    /// therefor is eventually cut. We will only find out during the next
    /// run. We will check if the first characters from the future `inp`-buffer
    /// eventually complete this substring. The flag is also true, when a
    /// substring was intentionally cut by this iterator itself. He does so
    /// when he considers`s` to be too long to be printed in one go.
    pub s_is_maybe_cut: bool,

    /// The returned string was found at the right buffer boundary and is
    /// considered to be too short to be printed in this run. Instead, it
    /// will be temporarily stored and then inserted at the beginning of the next
    /// `inp`-buffer.
    pub s_is_to_be_filtered_again: bool,

    /// This flag is `true` when the returned `s` has at least `chars_min_nb` characters.
    /// Usually the iterator always observes this minimum-rule, but there are
    /// some exceptions: e.g. with
    /// `last_s_was_maybe_cut` set, we can instruct the iterator to make such an
    /// exception. When he does, he sets also flag, so the caller can know.
    pub s_satisfies_grep_char_rule: bool,

    /// This flag is `true` when the returned `s` has at least one
    /// ASCII with code `grep_char`.
    /// Usually the iterator always observes this grep_char-rule, but there are
    /// some exceptions: e.g. with
    /// `last_s_was_maybe_cut` set, we can instruct the iterator to make such an
    /// exception. When he does, he sets also flag, so the caller can know.
    pub s_satisfies_min_char_rule: bool,
}
impl<'a> SplitStr<'a> {
    #[inline]
    pub fn new(
        inp: &str,
        chars_min_nb: u8,
        last_s_was_maybe_cut: bool,
        invalid_bytes_after_inp: bool,
        utf8f: Utf8Filter,
        s_len_max: usize,
    ) -> SplitStr {
        unsafe {
            SplitStr {
                // Input buffer.
                inp,
                // Points to the first byte in the buffer.
                inp_start_p: inp.as_ptr(),
                // This points to the last +1 byte in the buffer.
                inp_end_p: inp.as_ptr().add(inp.len()),
                // Points to the first byte to be treated, when next is called.
                p: inp.as_ptr(),
                chars_min_nb,
                last_s_was_maybe_cut,
                invalid_bytes_after_inp,
                // We will set this to false later, if `utf8f.grep_char` requires some
                // additional checking.
                utf8f,
                s_len_max,
            }
        }
    }
}
/// The iterator's `next()` returns some `SplitStrResult`-object, which is
/// essentially a substring `&str` pointing into a
/// `FindingCollection::output_buffer_bytes` with some additional information.
impl<'a> Iterator for SplitStr<'a> {
    type Item = SplitStrResult<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // Flag that indicates if the optional `grep_char`-criteria
        // should be checked.
        // When `grep_char` is not required, start with `true`,
        // otherwise with `false`.
        let mut grep_char_ok = self.utf8f.grep_char.is_none();
        let mut ok_s_p = self.p;
        let mut ok_s_len = 0usize;
        let mut ok_chars_nb = 0usize;
        // The longest `ok_s` we want to return in one `next()` iteration is
        // of length `ok_s_len_max`, which the usual `inp`-buffer size
        // when no extra bytes are prepended.
        // When we return such a maximum length string, we
        // keep the rest in `inp` for `next()`. Such a long string can only
        // appear, when some bytes form the last run had been prepended to
        // 'inp'.
        let ok_s_len_max = self.s_len_max;

        // The following loop has 4 exits:
        // 1. We finished the whole buffer: `self.p >= self.inp`
        // 2. A long string was found: `ok_s_len > ok_s_len_max`,
        //   `p` points to the first of the remaining bytes, left
        //    for the next `next()` run.
        // 3. We found a substring at the beginning of the buffer;
        // 4. We found a substring in somewhere in middle of the buffer;

        // Exit 1. and 2.
        while self.p < self.inp_end_p && ok_s_len < ok_s_len_max {
            // We do not need an additional boundary check, because we
            // know from above that there is at least one character in
            // `inp` and there are only valid UTF-8 in here.
            // This guaranty includes that the last character
            // also fits entirely in the buffer.

            // Is this a multi-byte-char?
            let leading_byte = unsafe { *self.p };
            let char_len = match leading_byte {
                c if c & 0b1000_0000 == 0b0000_0000 => {
                    {
                        // We can safely `unwrap()` here, because `grep_char_ok`
                        // can only be `false` when `self.utf8f.grep_char` is
                        // `Some()`.
                        if !grep_char_ok && self.utf8f.grep_char.unwrap() == c {
                            grep_char_ok = true;
                        };
                        // This check is done here for performance reasons. As
                        // must have applies to ASCII only, we ask only
                        // single-byte-characters.
                    }
                    1
                }
                c if c & 0b1110_0000 == 0b1100_0000 => 2,
                c if c & 0b1111_0000 == 0b1110_0000 => 3,
                c if c & 0b1111_1000 == 0b1111_0000 => 4,
                _ => 1, // this should never occur, but
                        // we do not test for errors here.
            };
            // We do not need to check if there is enough room, it is
            // guarantied by str.

            // So we assume there is enough space in buffer.
            // All information we need to check if the char pleases
            // the filter, is in `first_byte`, so we apply
            // the filter to `leading_byte`.
            if self.utf8f.pass_filter(leading_byte) {
                // This char is good. We keep on going.
                ok_s_len += char_len;
                ok_chars_nb += 1;
                // Set the pointer to the next char.
                self.p = unsafe { self.p.add(char_len) };
            } else {
                // This char did not please the filter.

                // We set the pointer to the next char.
                self.p = unsafe { self.p.add(char_len) };

                // Exit 3:
                if self.last_s_was_maybe_cut && ok_chars_nb > 0 && ok_s_p == self.inp_start_p {
                    break;
                // Exit 4:
                } else if ok_chars_nb >= self.chars_min_nb as usize && grep_char_ok {
                    // Yes, we collected enough for this run. The rest of the
                    // buffer can be treated later in a `next()`.
                    break;
                }

                // As we haven't found enough chars so far, we keep on searching.
                // We start from the top: optimistically and assume the next char is
                // good. The filter will reject the next char if we were wrong.
                ok_s_len = 0;
                ok_chars_nb = 0;
                ok_s_p = self.p;
                grep_char_ok = self.utf8f.grep_char.is_none();
            }
        }

        // We are here because we finished the buffer, or we found a string to give back
        // or both.
        // On the way, we have rejected all substrings, that did not
        // satisfy the search criteria.

        // This is save because we treat only complete chars.
        let ok_s = unsafe { str::from_utf8_unchecked(slice::from_raw_parts(ok_s_p, ok_s_len)) };

        // We ran through the buffer as far as possible. Did we find something?
        if ok_s.is_empty() {
            return None;
        };

        // What do we know so far?
        // Exit 1 or 5:
        let s_touches_left_boundary = ok_s_p == self.inp_start_p;
        // Exit 2 or 3:
        let s_touches_right_boundary = unsafe { ok_s_p.add(ok_s_len) } >= self.inp_end_p;

        let s_is_maybe_cut =
            ok_s_len >= ok_s_len_max || (s_touches_right_boundary && !self.invalid_bytes_after_inp);
        let s_completes_previous_s = s_touches_left_boundary && self.last_s_was_maybe_cut;

        // With this flag we tell the caller, that he should not immediately
        // print the returned string, but rather insert it at the the beginning
        // of the next input buffer and decode and run `SplitStr` again.
        //
        // Note, we require, that `ok_s_len` is at least 1 byte SMALLER then
        // `self.s_len_max` (`ok_s_len < self.s_len_max`). This way
        // we print strings that fill the whole output line directly.
        //
        // Note, `&& !s_completes_previous_s` guarantees, that
        // `s_is_to_be_filtered_again` is only set out for the first part
        // of a longer cut string. We only want the first part of string to be
        // completed with bytes from the `next()`-run. All following parts we do
        // not care, as long as the strings are long enough: We do this for 3
        // reasons:
        //
        // 1. When string is shorter than `chars_min_nb`, the filter can not
        // decide if it has to be rejected. It needs information from the stream
        // ahead. So better keep these bytes for later and insert them at the
        // beginning of the next buffer.
        //
        // 2. When the first part (==`!not_completes_previous`) of a longer
        // string who touches the right buffer boundary
        // (`==s_touches_right_boundary`) did start somewhere in the middle of
        // the buffer (==`ok_s_len < self.s_len_max`). We actually could
        // print it out now, because it has the minimum length, but we want to
        // print the beginning of a every string as long as possible (approx
        // `output_line_length`). Instead, we rather set
        // `s_is_to_be_filtered_again` instruction the caller to insert
        // this string at the beginning of the next buffer. Doing so, we
        // guarantee, that string beginnings are always assembled, even if they
        // crossed buffer boundaries. Thus, the user can pipe the output of
        // `stringsext` through additional filters, e.g. searching for
        // particular patterns.
        //
        // As `ok_chars_nb < chars_min_nb` is part of `ok_s_len < self.s_len_max`
        // we do not need to add this condition explicitly below.
        let s_is_to_be_filtered_again = !s_completes_previous_s
            && s_touches_right_boundary
            && !self.invalid_bytes_after_inp
            && (ok_s_len < self.s_len_max || !grep_char_ok);

        let s_satisfies_min_char_rule = ok_chars_nb >= self.chars_min_nb as usize;
        let s_satisfies_grep_char_rule = grep_char_ok;

        // Have we counted right?
        debug_assert_eq!(char_count(ok_s), ok_chars_nb, "We count wrongly.");

        // We dismiss this substring, because the `grep_char` condition is not
        // satisfied. There is only one exception, when we should not dismiss:
        // The string is at the right boundary and it is too short to be printed
        // now:
        //
        // As it will be inserted at the beginning of the next `output_buffer`,
        // we will see this string here again, and can decide then (seeing it in
        // full length) if we want to print it or not. To make this happen we
        // must not dismiss this substring, now. All other cases we dismiss the
        // substring.
        if !s_completes_previous_s
            && !s_is_to_be_filtered_again
            && (!s_satisfies_grep_char_rule || !s_satisfies_min_char_rule)
        {
            return None;
        };

        // Exit was 2: prepare the inner state for the next `next()` run.
        if ok_s_len >= ok_s_len_max {
            self.inp_start_p = self.p;
        };
        self.last_s_was_maybe_cut = s_is_maybe_cut;

        // Return results
        return Some(SplitStrResult {
            s: ok_s,
            s_completes_previous_s,
            s_is_maybe_cut,
            s_is_to_be_filtered_again,
            s_satisfies_min_char_rule,
            s_satisfies_grep_char_rule,
        });
    }
}

/// Small helper function that tests if some UTF-8 string starts with a
/// multi-byte-character.
#[inline]
pub fn starts_with_multibyte_char(s: &str) -> bool {
    s.as_bytes()[0] & 0x80 == 0x80
}

/// Count as fast as possible the chars in some UTF-8 str.
#[allow(dead_code)]
#[inline]
pub fn char_count(s: &str) -> usize {
    let mut n = 0usize;

    let mut i = 0usize;
    while i < s.len() {
        i += match s.as_bytes()[i] {
            c if c & 0b1000_0000 == 0b0000_0000 => 1,
            c if c & 0b1110_0000 == 0b1100_0000 => 2,
            c if c & 0b1111_0000 == 0b1110_0000 => 3,
            c if c & 0b1111_1000 == 0b1111_0000 => 4,
            _ => 1, // this should never occur, but
                    // we do not test for errors here.
        };
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    // To see println!() output in test run, launch
    // cargo test   -- --nocapture

    #[test]
    fn test_as_str_unchecked_no_borrow_check() {
        let s_in = "abc€déf";
        let b = s_in.as_bytes();
        let s_out = as_str_unchecked_no_borrow_check!(b);
        assert_eq!(s_in, s_out);
    }

    #[test]
    fn test_split_s() {
        // We filter Latin + ASCII.
        let utf8f = Utf8Filter {
            af: AF_ALL,
            ubf: UBF_LATIN,
            grep_char: None,
        };

        let b = "€abc€defg€hijk€lm€opq";

        let mut iter = SplitStr::new(b, 3, false, false, utf8f, b.len());
        let r = iter.next().unwrap();
        assert_eq!(r.s, "abc");
        assert_eq!(r.s_completes_previous_s, false);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "defg");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "hijk");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "opq");
        assert_eq!(iter.next(), None);

        let b = "ab€€defg€hijk€lm€opq";

        let mut iter = SplitStr::new(b, 3, true, false, utf8f, b.len());
        // Corner case: input=true + first string too short, but touches left boundary
        // -> Printed although too short, because it completes string from last run.
        let r = iter.next().unwrap();
        assert_eq!(r.s, "ab");
        assert_eq!(r.s_completes_previous_s, true);
        assert_eq!(r.s_satisfies_min_char_rule, false);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "defg");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "hijk");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "opq");
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(r.s_satisfies_min_char_rule, true);
        assert_eq!(r.s_is_to_be_filtered_again, true);
        assert_eq!(iter.next(), None);

        let b = "ab€€defg€hijk€lm€op";

        let mut iter = SplitStr::new(b, 3, false, false, utf8f, b.len());
        let r = iter.next().unwrap();
        assert_eq!(r.s, "defg");
        assert_eq!(r.s_completes_previous_s, false);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "hijk");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "op");
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(r.s_satisfies_min_char_rule, false);
        assert_eq!(r.s_is_to_be_filtered_again, true);
        assert_eq!(iter.next(), None);

        let b = "€abc€defg€hijk€lm";

        let mut iter = SplitStr::new(b, 4, false, false, utf8f, b.len());
        let r = iter.next().unwrap();
        assert_eq!(r.s, "defg");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "hijk");
        assert_eq!(r.s_is_maybe_cut, false);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "lm");
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(r.s_satisfies_min_char_rule, false);
        assert_eq!(r.s_is_to_be_filtered_again, true);
        assert_eq!(iter.next(), None);

        let b = "€abc€defg€hijk€lmno€";

        let mut iter = SplitStr::new(b, 4, false, false, utf8f, b.len());
        let r = iter.next().unwrap();
        assert_eq!(r.s, "defg");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "hijk");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "lmno");
        assert_eq!(r.s_is_maybe_cut, false);
        assert_eq!(r.s_satisfies_min_char_rule, true);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(iter.next(), None);

        // This tests the iterator's capability to cat substrings
        // > 7 bytes
        let b = "abc€defghiÜjklmnpqrs€";

        let mut iter = SplitStr::new(b, 4, false, false, utf8f, 7);
        let r = iter.next().unwrap();
        // Note, this is longer than 7 bytes.
        assert_eq!(r.s, "defghiÜ");
        assert_eq!(r.s_completes_previous_s, false);
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_satisfies_min_char_rule, true);

        let r = iter.next().unwrap();
        assert_eq!(r.s, "jklmnpq");
        assert_eq!(r.s_completes_previous_s, true);
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_satisfies_min_char_rule, true);

        let r = iter.next().unwrap();
        assert_eq!(r.s, "rs");
        assert_eq!(r.s_completes_previous_s, true);
        assert_eq!(r.s_is_maybe_cut, false);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_satisfies_min_char_rule, false);

        assert_eq!(iter.next(), None);

        let b = "abcdefghijklm";

        let mut iter = SplitStr::new(b, 4, false, false, utf8f, b.len());
        let r = iter.next().unwrap();
        assert_eq!(r.s, "abcdefghijklm");
        assert_eq!(r.s_completes_previous_s, false);
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_satisfies_min_char_rule, true);
        assert_eq!(iter.next(), None);

        let b = "abcdefghijklm€";

        let mut iter = SplitStr::new(b, 4, false, false, utf8f, b.len());
        let r = iter.next().unwrap();
        assert_eq!(r.s, "abcdefghijklm");
        assert_eq!(r.s_completes_previous_s, false);
        assert_eq!(r.s_is_maybe_cut, false);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_satisfies_min_char_rule, true);
        assert_eq!(iter.next(), None);

        let b = "öö€€ääää€üü€éééé€";

        let mut iter = SplitStr::new(b, 4, true, false, utf8f, b.len());
        let r = iter.next().unwrap();
        assert_eq!(r.s, "öö");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "ääää");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "éééé");
        assert_eq!(iter.next(), None);

        // New test:
        // We filter Latin + ASCII.

        let utf8f_ascii = Utf8Filter {
            af: AF_ALL,
            ubf: UBF_NONE,
            grep_char: None,
        };

        let b = "öö€€ääää€üü€éééé€";

        let mut iter = SplitStr::new(b, 4, true, false, utf8f_ascii, b.len());
        assert_eq!(iter.next(), None);
    }
    #[test]
    fn test_split_s_grep_char() {
        // We filter Latin + ASCII.
        let utf8f = Utf8Filter {
            af: AF_ALL,
            ubf: UBF_LATIN,
            grep_char: None,
        };

        let b = "ac€€xefg€xijk€xm€xp";

        let mut iter = SplitStr::new(b, 3, true, false, utf8f, b.len());
        // Corner case: input=true + first string too short, but touches left boundary
        // -> Printed although too short, because it completes string from last run.
        let r = iter.next().unwrap();
        assert_eq!(r.s, "ac");
        assert_eq!(r.s_completes_previous_s, true);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_is_maybe_cut, false);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "xefg");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "xijk");
        let r = iter.next().unwrap();
        assert_eq!(r.s, "xp");
        assert_eq!(r.s_completes_previous_s, false);
        assert_eq!(r.s_is_to_be_filtered_again, true);
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(iter.next(), None);

        // Next test, same input.
        let b = "ac€€xefg€xijk€xm€xp";

        let my_utf8f = Utf8Filter {
            af: AF_ALL,
            ubf: UBF_LATIN,
            grep_char: Some(b'b'),
        };

        let mut iter = SplitStr::new(b, 2, true, false, my_utf8f, 3);
        // Corner case: input=true + first string too short, but touches left boundary
        // -> Printed although too short, because it completes string from last run.
        // Only this have the compulsory "b".
        let r = iter.next().unwrap();
        assert_eq!(r.s, "ac");
        assert_eq!(r.s_completes_previous_s, true);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_is_maybe_cut, false);
        assert_eq!(iter.next(), None);

        // Next test, same input.
        let b = "ac€€xefg€xijk€xm€xp";

        let my_utf8f = Utf8Filter {
            af: AF_ALL,
            ubf: UBF_LATIN,
            grep_char: Some(b'x'),
        };

        let mut iter = SplitStr::new(b, 2, true, false, my_utf8f, 3);
        // Corner case: input=true + first string too short, but touches left boundary
        // -> Printed although too short, because it completes string from last run.
        // The first passes, because we told there should be no
        // restrictions to the first substring (touching the left boundary).
        // All others have the compulsory "x", so they are printed.
        let r = iter.next().unwrap();
        assert_eq!(r.s, "ac");
        assert_eq!(r.s_completes_previous_s, true);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_is_maybe_cut, false);
        assert_eq!(r.s_satisfies_grep_char_rule, false);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "xef");
        assert_eq!(r.s_completes_previous_s, false);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(r.s_satisfies_grep_char_rule, true);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "g");
        assert_eq!(r.s_completes_previous_s, true);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_is_maybe_cut, false);
        assert_eq!(r.s_satisfies_grep_char_rule, false);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "xij");
        assert_eq!(r.s_completes_previous_s, false);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(r.s_satisfies_grep_char_rule, true);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "k");
        assert_eq!(r.s_completes_previous_s, true);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_is_maybe_cut, false);
        assert_eq!(r.s_satisfies_grep_char_rule, false);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "xm");
        assert_eq!(r.s_completes_previous_s, false);
        assert_eq!(r.s_is_to_be_filtered_again, false);
        assert_eq!(r.s_is_maybe_cut, false);
        assert_eq!(r.s_satisfies_grep_char_rule, true);
        let r = iter.next().unwrap();
        assert_eq!(r.s, "xp");
        assert_eq!(r.s_completes_previous_s, false);
        assert_eq!(r.s_is_to_be_filtered_again, true);
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(r.s_satisfies_grep_char_rule, true);
        assert_eq!(iter.next(), None);

        // Next test.

        let b = "öä€€äüöä€äüöö€üö€üü";

        let my_utf8f = Utf8Filter {
            af: AF_ALL,
            ubf: UBF_LATIN,
            grep_char: Some(b'y'),
        };

        let mut iter = SplitStr::new(b, 3, false, false, my_utf8f, b.len());
        // Corner case: input=false + first string too short, but touches left boundary
        // -> Not printed, because it does not complete the string from last run.
        // No others have the compulsory "y", so they are not printed, except the last,
        // it might be completed.
        let r = iter.next().unwrap();
        assert_eq!(r.s, "üü");
        assert_eq!(r.s_completes_previous_s, false);
        assert_eq!(r.s_is_to_be_filtered_again, true);
        assert_eq!(r.s_is_maybe_cut, true);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_char_count() {
        assert_eq!("hello".len(), 5);
        assert_eq!(char_count("hello"), 5);

        assert_eq!("abcö".len(), 5);
        assert_eq!(char_count("abcö"), 4);

        assert_eq!("abc€".len(), 6);
        assert_eq!(char_count("abcö"), 4);

        assert_eq!("abc\u{10FFFF}def".len(), 10);
        assert_eq!(char_count("abc\u{10FFFF}def"), 7);
    }

    #[test]
    fn test_starts_with_multibyte_char() {
        assert_eq!(starts_with_multibyte_char("abcdef"), false);
        assert_eq!(starts_with_multibyte_char("aücdef"), false);
        assert_eq!(starts_with_multibyte_char("übcdef"), true);
    }
}
