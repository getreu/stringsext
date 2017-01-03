//! This module encapsulates and abstracts the interface with the `encoding`-crate and
//! spawns worker threads `scanners`, searching for valid strings.
//!
//! # Scanner algorithm
//!
//! 1. A _scanner_ is a thread with an individual search `Mission` containing
//! the _encoding_ it searches for.
//!
//! 2. The input data is divided into consecutive overlapping memory chunks.
//! A chunk is a couple of 4KB memory pages, `WIN_LEN` Bytes in size.
//!
//! 3. Scanners are in pause state until they receive a pointer to a memory chunk
//! with a dedicated search `Mission`.
//!
//! 4. All scanner-threads search simultaneously in one memory chunk only.
//! This avoids that the threads drift to far apart.
//!
//! 5. Every scanner thread searches its encoding consecutively Byte by Byte from
//! lower to higher memory.
//!
//! 6. When a scanner finds a valid string, it encodes it into a UTF-8 copy.
//! Valid strings are composed of control characters and graphical characters.
//!
//! 7. The copy of the above valid string is split into one or several graphical
//! strings. Hereby all control characters are omitted. The graphical strings are
//! then concatenated and the result  is stored in a `Finding`
//! object. A `Finding`-object also carries the memory
//! location of the finding and a label describing the search mission.   Goto 5.
//!
//! 8. A scanner stops when it passes the upper border `WIN_STEP` of the current
//! memory chunk.
//!
//! 9. The scanner stores its `Finding`-objects in a vector referred as `Findings`.
//! The vector is ascending in memory location.
//!
//! 10. Every scanner sends its `Findings` to the _merger-printer-thread_.  In order
//! to resume later, it updates a marker in its `Mission`-object pointing to the
//! exact Byte where it has stopped scanning. Besides this marker, the scanner
//! is _stateless_. Finally the scanner pauses and waits for the next
//! memory chunk and mission.
//!
//! 11. After all scanners have finished their search in the current chunk,
//! the _merger-printer-thread_ receives the `Findings` and collects them in
//! a vector.
//!
//! 12. The _merger-printer-thread_ merges all `Findings` from all threads into one
//! timeline and prints the formatted result through the output channel.
//!
//! 13. In order to prepare the next iteration, pointers are set to beginning of the
//! next chunk.  Every scanner resumes exactly where it stopped before.
//!
//! 14. Goto 3.
//!
//! 15. Repeat until the last chunk is reached.


use std::str;
extern crate memmap;
extern crate itertools;

use std::sync::mpsc::SyncSender;

extern crate scoped_threadpool;
use scoped_threadpool::Pool;

extern crate encoding;

#[cfg(not(test))]
use input::WIN_STEP;
#[cfg(test)]
use self::tests::WIN_STEP;

#[cfg(not(test))]
use input::WIN_OVERLAP;
#[cfg(test)]
use self::tests::WIN_OVERLAP;


#[cfg(not(test))]
use input::UTF8_LEN_MAX;
#[cfg(test)]
use self::tests::UTF8_LEN_MAX;


use Mission;
use Missions;
use finding::FindingCollection;

/// As the scan_window() function itself is stateless, the following variables store some
/// data that will be transfered from iteration to iteration.
/// Each thread has an associated `Mission` with a `ScannerState`.
pub struct ScannerState {
    /// The position relative to a WIN_LEN window used to start the next iteration search at.
    /// This value is updated after each iteration to ensuring that the next
    /// iteration starts the scanning exactly where the previous stopped.
    /// This variable together with
    pub offset: usize,

    /// When a finding exceeds WIN_LEN it has to be cut, the remaining part
    /// of the string might be shorter then . This variable informs the next
    /// iteration to ignore the minimum size restriction for the first string in found
    /// at the beginning of the next window.
    pub completes_last_str: bool,
}

/// Holds the runtime environment for `Scanner::launch_scanner()`.
pub struct Scanner <'a> {
    /// Each thread `x` gets an encoding e.g. `missions.v[x].encoding` it will keep
    /// until the end of the program. Unlike `missions.v[x].encoding` the
    /// `missions.v[x].offset` is dynamically updated
    /// each iteration. It communicates the end position as start position to the
    /// next iteration ensuring that it starts exactly where the
    /// previous ended.
    pub missions: Vec<Mission<>>,
    /// A collection of threads ready to execute a `Mission`.
    pub pool: Pool,
    /// The sender used by all threads to report their results.
    pub tx:   &'a SyncSender<FindingCollection>,
}



impl <'a> Scanner <'a> {
    /// Constructor: Prepare the runtime environment for `Scanner::launch_scanner()`.
    ///
    pub fn new(missions: Missions, tx: &'a SyncSender<FindingCollection>) -> Self {

        let n_threads = missions.len();

        Scanner { missions: missions.v,
                             pool: Pool::new(n_threads as u32),
                             tx: &tx,
                 }

    }

    /// Takes an input slice, searches for valid strings according
    /// to the encoding specified in `Scanner::missions` and sends the results
    /// as a `FindingCollection` package to the Merger-thread using a `SyncSender`.
    /// As runtime environment `launch_scanner()` relies on an initialized `Missions` vector,
    /// as well as a thread pool and a `SyncSender` where it can push its results.
    ///
    pub fn launch_scanner<'b> (&mut self, byte_counter: &usize,
                            input_slice: &'b [u8])  {

        Scanner::launch_scanner2 (
                        &mut self.missions,
                        &byte_counter, &input_slice,
                        &mut self.pool, &self.tx);
    }

    /// This method is only called by `launch_scanner()`.
    /// The redirection is necessary since the current version of `scoped_threadpool`
    /// does not allow threads to access the parent's member variables.
    /// Only the parents stack-frame can be accessed.
    ///
    fn launch_scanner2<'b> ( missions: &'b mut Vec<Mission>,
                            byte_counter: &usize,
                            input_slice: &'b [u8],
                            pool: &mut Pool,
                            tx: &SyncSender<FindingCollection>)  {
               pool.scoped(|scope| {
                   for mission in missions.iter_mut() {
                        let tx = tx.clone();
                        scope.execute(move || {
                            let (m, end_pos,completes_last_str) = Scanner::scan_window ( mission,
                                                           byte_counter,
                                                           input_slice );

                            // Update `mission.offset` to indicate the position
                            // Where the next iteration should resume the work.
                            mission.state.offset = if end_pos >= WIN_STEP {
                                end_pos - WIN_STEP
                            } else {
                                0
                            };
                            mission.state.completes_last_str = completes_last_str;
                            match tx.send(m) {
                                Ok(_) => {},
                                Err(_) => { panic!("Can not send FindingCollection:"); },
                            };
                        });
                   }
               });
    }


    /// Scans the `input`-slice beginning from position `start` for valid strings
    /// encoded as `enc`.
    ///
    /// In case `start` points into the UTF8_LEN_MAX fragment, i.e.:
    ///
    /// ```text
    ///      WIN_OVERLAP - UTF8_LEN_MAX <= start <= WIN_OVERLAP
    /// ```
    /// we know that the previous `input`-slice ended with an incomplete string.
    /// This is why we will then print the first string we find at `start` notwithstanding its
    /// size.
    ///
    /// The `Finding`s are returned as a `FindingCollection` vector.
    /// After execution the `start` variable points to the first unprocessed Byte
    /// in `input`, usually in WIN_OVERLAP.

    ///

    /// Please note that this function is stateless (static)!
    ///
    fn scan_window <'b> (mission:&Mission,
                         byte_counter: &usize,
                         input: &'b [u8]) -> (FindingCollection, usize, bool) {
        // True if `mission.offset` is in the last UTF8_LEN_MAX Bytes of WIN_OVERLAP
        // (*mission.offset  >= WIN_OVERLAP as usize - UTF8_LEN_MAX as usize) ;
        // Above: human readable, below: equivalent and more secure
        let completes_last_str = mission.state.completes_last_str;


        let mut ret = Box::new(FindingCollection::new(mission));
        let mut decoder = mission.encoding.raw_decoder();

        //let mut unprocessed = mission.state.offset;
        let mut remaining = mission.state.offset;

        while remaining < WIN_STEP { // Never do mission.offset new search in overlapping space
            ret.close_old_init_new_finding(byte_counter+remaining,
                                           mission);
            let (offset, err) = decoder.raw_feed(&input[remaining..], &mut *ret);
            //unprocessed = remaining + offset;
            if let Some(err) = err {
                remaining = (remaining as isize + err.upto) as usize;
                //we do not care what the error was, instead we continue
            }
            else {
                remaining += offset;
                // we have reached the end. Somewhere between
                // WIN_LEN-UTF8_LEN_MAX and WIN_LEN
                let _ = decoder.raw_finish(&mut *ret); // Is this really necessary? Why?
                break;
            }

        };
        // This closes the current finding strings and adds an
        // empty one we have to remove with `close_finding_collection()` later.
        ret.close_old_init_new_finding(byte_counter+remaining, mission);

        // Remove empty surplus
        ret.close_finding_collection();
        // unprocessed points to the first erroneous byte, remaining 1 byte beyond:
        // -> remaining is a bit faster
        let end_pos = remaining;
        // For debugging/testing we remember that `completes_last_str` was set.
        ret.completes_last_str = completes_last_str;

        let is_incomplete = (end_pos + (UTF8_LEN_MAX as usize) >= WIN_OVERLAP + WIN_STEP)
                            && ret.v.len() != 0  ;

        (*ret, end_pos, is_incomplete)
    }
}






#[cfg(test)]
mod tests {
    use super::*;
    use options::{Args, Radix, ControlChars};
    extern crate encoding;
    use encoding::EncodingRef;
    use std::str;
    extern crate rand;
    use Mission;
    use finding::Finding;
    use finding::FindingCollection;

    pub const WIN_STEP: usize  = 17;
    pub const WIN_OVERLAP: usize  = 5 + 3; // flag_bytes + UTF8_LEN_MAX
    pub const WIN_LEN:  usize  = WIN_STEP + WIN_OVERLAP as usize; // =25
    pub const UTF8_LEN_MAX: u8 = 3;

    lazy_static! {
        pub static ref ARGS:Args = Args {
           arg_FILE: Some("myfile.txt".to_string()),
           flag_control_chars:  ControlChars::R,
           flag_encoding: vec!["ascii".to_string(), "utf8".to_string()],
           flag_list_encodings: false,
           flag_version: false,
           flag_bytes:  Some(5),
           flag_radix:  Some(Radix::X),
           flag_output: None,
        };
    }

    /// Are the `Ordering` traits implemented properly?
    #[test]
    fn test_compare_findings(){
        let small = Finding{ptr: 5, enc: encoding::all::UTF_16LE, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "Hello".to_string() };
        let smaller = Finding{ptr: 5, enc: encoding::all::UTF_16BE, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "Hello".to_string()};
        let smallest = Finding{ptr: 5, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "Hello".to_string()};
        let big1 = Finding{ ptr: 12, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "world!".to_string() };
        let big2 = Finding{ ptr: 12, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "world!".to_string() };
        let big3 = Finding{ ptr: 12, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "zulu!".to_string() };

        assert_eq!(big1, big2);
        assert!(big1 != big3);
        assert!(small > smaller);
        assert!(smaller < small);
        assert!(smallest < smaller);
        assert!(smallest < small);
        assert!(small < big1);
        assert!(small < big3);
    }


    /// Does the `Scanner::scan_window()` respect the minimum constraint?
    #[test]
    fn test_scan_min_bytes(){

       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr:  0, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "Hello".to_string() },
                Finding{ ptr: 12, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "world!".to_string() }
                ], completes_last_str: false};

       // The word "new" in "Helloünewüworld!" is too short (<5) and will be ommited.
       let start = 0;
       let inp = "Helloünewüworld!".as_bytes();

       let m = Mission {encoding: encoding::all::ASCII,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: false,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res,start, _) = Scanner::scan_window(&m, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 18);



       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr:  5, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "How are you?".to_string() },
                ], completes_last_str: false};

       // The words "Hi!" in "Hi!üHow are you?üHi!" are too short (<5) and will be ommited.
       let start = 0;
       let inp = "Hi!üHow are you?üHi!".as_bytes();

       let m = Mission {encoding: encoding::all::ASCII,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res,start, _) = Scanner::scan_window(&m, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 18);
    }

    /// Test sample is an extract of a raw dist image
    #[test]
    fn test_scan_raw_input(){
       // to see output of println on console:  cargo test -- --nocapture
            // This is the input data:
            //  H W.1  z..
            //. <...AWAVAU
            //ATUSH  .H .L

       let inp = vec![
                 0xEBu8, 0xA1, 0x48, 0x8B, 0x57, 0x08, 0x31, 0xC0, 0x83, 0x7A, 0x08, 0x03,
                   0x0F, 0x85, 0x3C, 0x01, 0x00, 0x00, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55,
                   0x41, 0x54, 0x55, 0x53, 0x48, 0x83, 0xEC, 0x18, 0x48, 0x8B, 0x07, 0x4C,
                  ];
       println!("{:?}",inp);

       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr: 14, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "\u{fffd}AWAVAUA".to_string() }
                ], completes_last_str: false};

       let start = 0;

       let m = Mission {encoding: encoding::all::ASCII,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res, mut start, _) = Scanner::scan_window(&m, &start, &inp[..WIN_LEN]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, WIN_LEN);



       // Simulate next iteration.
       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr:  WIN_LEN, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "TUSH".to_string() },
                ], completes_last_str: true};

       start -= WIN_STEP;
       let m = Mission {encoding: encoding::all::ASCII,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: start, completes_last_str: true}
       };

       let (res,start,incomplete) = Scanner::scan_window(&m, &WIN_STEP, &inp[WIN_STEP..]);
       assert_eq!(res, expected_fc);
       assert_eq!(start, 17);
       assert_eq!(incomplete, false);
    }


    /// Does FindingCollection::close_old_init_new_finding() checks
    /// `ARGS.flag_control_chars`  ?
    #[test]
    fn test_scan_control_chars(){
       // to see output of println on console:  cargo test -- --nocapture
       let inp = "\u{0}\u{0}34567890\u{0}\u{0}345678\u{0}0123\u{0}\u{0}".as_bytes();

       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr: 0, enc: encoding::all::UTF_8, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "\u{fffd}34567890\u{fffd}345678".to_string() }
                ], completes_last_str: false};

       let start = 0;

       let m = Mission {encoding: encoding::all::UTF_8,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res,start,incomplete) = Scanner::scan_window(&m, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 25);
       assert_eq!(incomplete, true);



       // to see output of println on console:  cargo test -- --nocapture
       let inp = "\u{0}\u{0}\u{0}\u{0}".as_bytes();

       let expected_fc = FindingCollection{ v: vec![], completes_last_str: false };

       let start = 0;

       let m = Mission {encoding: encoding::all::ASCII,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res,start,incomplete) = Scanner::scan_window(&m, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 4);
       assert_eq!(incomplete, false);

    }





    /// When a string has to be cut just before its end, will the remaining
    /// part be printed even if it is too short?
    #[test]
    fn test_scan_incomplete_utf8_simulate_iterations(){

       // to see output of println on console:  cargo test -- --nocapture
       // 20 Bytes and then [226, 130, 172, 226, 130, 172]
       let inp = "12345678901234567890€€".as_bytes();

       // pic the first €
       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr: 0, enc: encoding::all::UTF_8, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "12345678901234567890€".to_string() }
                ], completes_last_str: false};

       let start = 0;
       let mut m = Mission {encoding: encoding::all::UTF_8,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res, mut start,incomplete) = Scanner::scan_window(&m, &start, &inp[..WIN_LEN]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 23);
       assert_eq!(incomplete, true);

       // Simulate next iteration.
       // The following is problematic because "€" is shorter then ARGS.flag_bytes and
       // will be normally omitted.
       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr:  23, enc: encoding::all::UTF_8, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "€".to_string() },
                ], completes_last_str: true};

       start -= WIN_STEP;
       m.state.offset = start;
       m.state.completes_last_str = incomplete;

       let (res,start,incomplete) = Scanner::scan_window(&m, &WIN_STEP, &inp[WIN_STEP..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 9);
       assert_eq!(incomplete, false);
    }



    /// Is mission.offset set properly in case a byte-sequence is overlaps WIN_LEN?
    #[test]
    fn test_scan_incomplete_utf8(){

       // to see output of println on console:  cargo test -- --nocapture
       // 19 Bytes and then [226, 130, 172, 226, 130, 172, 226, 130, 172]
       let inp = "1234567890123456789€€€".as_bytes();

       // pic the first 2 €
       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr: 0, enc: encoding::all::UTF_8, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "1234567890123456789€€".to_string() }
                ], completes_last_str: false};

       let start = 0;
       let m = Mission {encoding: encoding::all::UTF_8 as EncodingRef,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res,start, _) = Scanner::scan_window(&m, &start, &inp[..WIN_LEN]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 25);
    }


    /// Are valid strings correctly separated and listed?
    #[test]
    fn test_scan_erroneous_ascii(){

       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr:  0, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "Hello".to_string() },
                Finding{ ptr: 11, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "world!".to_string() }
                ], completes_last_str: false};

       // Erroneous  Bytes are in the middle only
       let start = 0;
       let inp = "Helloüüüworld!".as_bytes();

       let m = Mission {encoding: encoding::all::ASCII,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res,start, _) = Scanner::scan_window(&m, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 17);


       // Erroneous  Bytes are in the middle and at the end
       let start = 0;
       let inp = "Helloüüüworld!üü".as_bytes();

       let m = Mission {encoding: encoding::all::ASCII,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res,start, _) = Scanner::scan_window(&m, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 18);

    }

    /// Does the loop in `Scanner::scan_window()` relaunching `Decoder::raw_feed()`
    /// terminate properly when `mission.offset` has reached WIN_OVERLAP?
    #[test]
    fn test_scan_no_new_search_in_overlapping_space(){

       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr:  0, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "Hello".to_string() },
                Finding{ ptr: 11, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "world!".to_string() }
                ], completes_last_str: false};

       // "How are you?" started in overlapping space and should not be found.
       let start = 0;
       let inp = "Helloüüüworld!üüHow are you?".as_bytes();

       let m = Mission {encoding: encoding::all::ASCII,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res,start, _) = Scanner::scan_window(&m, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 18);



       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr:  0, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "Hello".to_string() },
                Finding{ ptr: 11, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "world! How are you?".to_string() }
                ], completes_last_str: false};


       // "How are you?" is in overlapping space but started earlier. So is should be found.
       let start = 0;
       let inp = "Helloüüüworld! How are you?".as_bytes();

       let m = Mission {encoding: encoding::all::ASCII,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res,start, _) = Scanner::scan_window(&m, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 30);

    }

    /// Is `Mission::offset()` is interpreted correctly to
    /// instruct the next iteration to print the remaining part of a cut string
    /// even if it is too short?
    #[test]
    fn test_scan_do_not_forget_first_short_string_in_next_chunk(){

       // The next should be found and fill the whole WIN_LEN and 2 Bytes more
       let expected_fc = FindingCollection{ v: vec![
                Finding{ ptr:  0, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "Hello world! How do you d".to_string() },
                ], completes_last_str: false};
       let start = 0;
       let inp = "Hello world! How do you do?".as_bytes();

       let mut m = Mission {encoding: encoding::all::ASCII,
                        u_and_mask: 0xffe00000,
                        u_and_result: 0,
                        nbytes_min: 5,
                        enable_filter: true,
                        state: ScannerState {offset: 0, completes_last_str: false}
       };

       let (res, mut start, incomplete) = Scanner::scan_window(&m, &start, &inp[..WIN_LEN]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 25);
       assert_eq!(incomplete,true);


       // The following is problematic because "o?" is shorter then ARGS.flag_bytes and
       // will be normally omitted.
       let expected2 = FindingCollection{ v: vec![
                Finding{ ptr:  25, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "o?".to_string() },
                ], completes_last_str: true};

       // Prepare next iteration
       start -= WIN_STEP;
       m.state.offset = start;
       m.state.completes_last_str = incomplete;

       let (res,start,incomplete) = Scanner::scan_window(&m, &WIN_STEP, &inp[WIN_STEP..]);

       assert_eq!(res, expected2);
       assert_eq!(start, 10);
       assert_eq!(incomplete,false);

    }




    /// Test the overall behavior of threads used by `Scanner::launch_scanner()`.
    #[test]
    fn test_scan_scanner_threads(){
        use Missions;
        use std::sync::mpsc;
        use std::thread::JoinHandle;
        use std::thread;


        let missions = Missions::new(&vec!["ascii".to_string(),"utf8".to_string()],
                                         &ARGS.flag_control_chars
        );
        //println!("{:?}",missions);

        let merger: JoinHandle<()>;

        {
            let (tx, rx) = mpsc::sync_channel(0);
            let mut sc = Scanner::new(missions, &tx);

            merger = thread::spawn(move || {

                let expected1 = FindingCollection{ v: vec![
                    Finding{ ptr: 1000, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                    u_and_result:0, s: "hallo1234".to_string() },
                    Finding{ ptr: 1015, enc: encoding::all::ASCII, u_and_mask: 0xffe00000,
                        u_and_result:0, s: "so567890".to_string() },
                    ], completes_last_str: false};

                let expected2 = FindingCollection{ v: vec![
                    Finding{ ptr: 1000, enc: encoding::all::UTF_8, u_and_mask: 0xffe00000,
                        u_and_result:0, s: "hallo1234üduüso567890".to_string() },
                    ], completes_last_str: false};

                let res1 = rx.recv().unwrap();
                let res2 = rx.recv().unwrap();
                //println!("Result 1: {:?}",res1);
                //println!("Result 2: {:?}",res2);

                assert!((expected1 == res1) || (expected1 == res2));
                assert!((expected2 == res1) || (expected2 == res2));
                //println!("Merger terminated.");
            });

            sc.launch_scanner (&1000usize, &"hallo1234üduüso567890".as_bytes() );
            assert_eq!(sc.missions[0].state.offset, 6);
            assert_eq!(sc.missions[1].state.offset, 6);

        } // tx drops here

        merger.join().unwrap();
    }


    /// Feed `Scanner::scan_window()` with random data to test its robustness.
    #[test]
    #[allow(unused_variables)]
    fn test_scan_random_input(){
        use self::rand::Rng;
        use codec::ascii::ASCII_GRAPHIC;

        let mut rng = rand::thread_rng();
        for _ in 0..0x1000 {
            let inp = (0..WIN_LEN).map(|_| rng.gen::<u8>()).collect::<Vec<u8>>();

            let start=0;
            let m = Mission {encoding: encoding::all::ASCII,
                            u_and_mask: 0xffe00000,
                            u_and_result: 0,
                            nbytes_min: 5,
                            enable_filter: true,
                            state: ScannerState {offset: 0, completes_last_str: false}
            };
            let (res1,start, _) = Scanner::scan_window(&m, &start, &inp[..]);


            let start=0;
            let m = Mission {encoding: encoding::all::UTF_8 as EncodingRef,
                            u_and_mask: 0xffe00000,
                            u_and_result: 0,
                            nbytes_min: 5,
                            enable_filter: true,
                            state: ScannerState {offset: 0, completes_last_str: false}
            };
            let (res2,start, _) = Scanner::scan_window(&m, &start, &inp[..]);

            let start=0;
            let m = Mission {encoding: encoding::all::UTF_16BE as EncodingRef,
                            u_and_mask: 0xffe00000,
                            u_and_result: 0,
                            nbytes_min: 5,
                            enable_filter: true,
                            state: ScannerState {offset: 0, completes_last_str: false}
            };
            let (res3,start, _) = Scanner::scan_window(&m, &start, &inp[..]);


            let start=0;
            let m = Mission {encoding: ASCII_GRAPHIC as EncodingRef,
                            u_and_mask: 0xffe00000,
                            u_and_result: 0,
                            nbytes_min: 5,
                            enable_filter: true,
                            state: ScannerState {offset: 0, completes_last_str: false}
            };
            let (res4,start, _) = Scanner::scan_window(&m, &start, &inp[..]);



            let start=0;
            let m = Mission {encoding: encoding::all::EUC_JP as EncodingRef,
                            u_and_mask: 0xffe00000,
                            u_and_result: 0,
                            nbytes_min: 5,
                            enable_filter: true,
                            state: ScannerState {offset: 0, completes_last_str: false}
            };
            let (res5,start, _) = Scanner::scan_window(&m, &start, &inp[..]);


            // To see println! output:  cargo test   -- --nocapture
            /*
            println!("Scan of random Bytes: {:?} {:?} {:?} {:?} {:?}",
                    res1.v, res2.v, res3.v, res4.v, res5.v);
            */
        };
    }

}
