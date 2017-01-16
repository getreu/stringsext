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


use std;
use std::str;
use std::io::Write;
use std::process;
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


use mission::Mission;
use mission::Missions;

use finding::FindingCollection;

/// As the `ScannerPool.scan_window()` function itself is stateless, the following variables
/// store some data that will be transfered from iteration to iteration.
/// Each thread has a unique `ScannerState` which holds a reference to a unique `Mission`.
#[derive(Debug)]
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

    /// This describes the mission of this scanner. It is a static unmutable reference.
    pub mission:&'static Mission,
}

pub struct ScannerStates {
    /// Vector of ScannerState
    v: Vec<ScannerState>
}

/// Holds the runtime environment for `ScannerPool::launch_scanner()`.
pub struct ScannerPool <'a> {
    /// Each thread `x` gets its own ScannerState.MISSION instance that it keeps
    /// until the end of the program. Unlike `Scannerstate.MISSIONS` the
    /// `ScannerState.offset` and `ScannerState.completes_last_str` are dynamically updated
    /// each iteration. `ScannerState` communicates the end position as start position to the
    /// next iteration ensuring that it starts exactly where the
    /// previous ended.
    pub pool: Pool,
    /// The sender used by all threads to report their results.
    pub tx:   &'a SyncSender<FindingCollection>,
    pub scanner_states:ScannerStates,
}



impl <'a> ScannerPool <'a> {
    /// Constructor: Prepare the runtime environment for `ScannerPool::launch_scanner()`.
    ///
    pub fn new(missions:&'static Missions, tx: &'a SyncSender<FindingCollection>) -> Self {

        let n_threads = missions.len();
        let v = Vec::new();
        let mut ms = ScannerStates{ v: v};
        for i in 0..n_threads {
            ms.v.push(ScannerState {
                          offset: 0,
                          completes_last_str: false,
                          mission:&missions.v[i],
                       }
            );
        }

        ScannerPool {
                             pool: Pool::new(n_threads as u32),
                             tx: &tx,
                             scanner_states : ms,
                 }

    }

    /// Takes an input slice, searches for valid strings according
    /// to the encoding specified in `ScannerPool::MISSIONS` and sends the results
    /// as a `FindingCollection` package to the Merger-thread using a `SyncSender`.
    /// As runtime environment `launch_scanner()` relies on an initialized `Missions` vector,
    /// as well as a thread pool and a `SyncSender` where it can push its results.
    ///
    pub fn launch_scanner<'b> (&mut self, filename: Option<&'static str>, byte_counter: &usize,
                            input_slice: &'b [u8])  {

        ScannerPool::launch_scanner2 (
                        filename, &byte_counter, &input_slice,
                        &mut self.pool, &self.tx, &mut self.scanner_states);
    }

    /// This method is only called by `launch_scanner()`.
    /// The redirection is necessary since the current version of `scoped_threadpool`
    /// does not allow threads to access the parent's member variables.
    /// Only the parents stack-frame can be accessed.
    ///
    fn launch_scanner2<'b> (
                            filename: Option<&'static str>,
                            byte_counter: &usize,
                            input_slice: &'b [u8],
                            pool: &mut Pool,
                            tx: &SyncSender<FindingCollection>,
                            scanner_states:&mut ScannerStates)  {
               pool.scoped(|scope| {
                   for scanner_state in scanner_states.v.iter_mut() {
                        let tx = tx.clone();
                        scope.execute(move || {
                            let (m, end_pos) = ScannerPool::scan_window (
                                                           scanner_state,
                                                           filename,
                                                           byte_counter,
                                                           input_slice );

                            // Update `mission.offset` to indicate the position
                            // Where the next iteration should resume the work.
                            scanner_state.offset = if end_pos >= WIN_STEP {
                                end_pos - WIN_STEP
                            } else {
                                0
                            };
                            scanner_state.completes_last_str = m.last_str_is_incomplete;
                            match tx.send(m) {
                                Ok(_)  => {},
                                Err(e) => {
                                    writeln!(&mut std::io::stderr(),
                                        "Error: `{}`. Is the output stream writeable? Is there \
                                         enough space? ",e).unwrap();
                                    process::exit(1);
                                },
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
    fn scan_window <'b> (scanner_state:&ScannerState,
                         filename: Option<&'static str>,
                         byte_counter: &usize,
                         input: &'b [u8]) -> (FindingCollection, usize) {
        // True if `mission.offset` is in the last UTF8_LEN_MAX Bytes of WIN_OVERLAP
        // (*mission.offset  >= WIN_OVERLAP as usize - UTF8_LEN_MAX as usize) ;
        // Above: human readable, below: equivalent and more secure

        //let mut unprocessed = mission.state.offset;
        let mut remaining = scanner_state.offset;
        let mut decoder = scanner_state.mission.encoding.raw_decoder();

        // This adds a first empty finding (nothing to close)
        let mut ret = Box::new(FindingCollection::new(filename,
                                                      byte_counter+remaining,
                                                      scanner_state.mission));
        // This should ever be true only for the first finding
        ret.completes_last_str = scanner_state.completes_last_str;


        loop {
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
            // Never start new search in overlapping space
            if remaining >= WIN_STEP {
                let _ = decoder.raw_finish(&mut *ret); // Is this really necessary? Why?
                break;
            }
            ret.close_old_init_new_finding(byte_counter+remaining);
            // only the first finding should have this true
            ret.completes_last_str = false;
        };

        // unprocessed points to the first erroneous byte, remaining 1 byte beyond:
        // -> remaining is a bit faster
        let end_pos = remaining;
        // If `end_pos` is between `WIN_OVERLAP + WIN_STEP-UTF8_LEN_MAX` and
        // `WIN_OVERLAP + WIN_STEP` then we know that the last string has been cut.
        let last_str_is_incomplete =
                     (end_pos + (UTF8_LEN_MAX as usize) >= WIN_OVERLAP + WIN_STEP)
                     && ret.v.len() != 0  ;

        // Tell the filter, that the last graphic string should not be omitted, even if too short.
        ret.last_str_is_incomplete = last_str_is_incomplete;

        // This closes the current finding strings and adds an
        // empty one we have to remove with `close_finding_collection()` later.
        // Before processing the last finding,
        ret.close_old_init_new_finding(byte_counter+remaining);

        // Remove empty surplus
        ret.close_finding_collection();
        // For debugging/testing we remember that `completes_last_str` was set.
        ret.completes_last_str = scanner_state.completes_last_str;


        (*ret, end_pos)
    }
}






#[cfg(test)]
mod tests {
    use super::*;
    use options::{Args, Radix, ControlChars};
    extern crate encoding;
    use std::str;
    extern crate rand;
    use finding::Finding;
    use finding::FindingCollection;

    pub const WIN_STEP: usize  = 17;
    pub const WIN_OVERLAP: usize  = 5 + 3; // flag_bytes + UTF8_LEN_MAX
    pub const WIN_LEN:  usize  = WIN_STEP + WIN_OVERLAP as usize; // =25
    pub const UTF8_LEN_MAX: u8 = 3;
    use mission::Missions;

    lazy_static! {
        pub static ref ARGS:Args = Args {
           arg_FILE: vec!["myfile.txt".to_string()],
           flag_control_chars: ControlChars::R,
           flag_encoding: vec!["ascii".to_string(), "utf8".to_string()],
           flag_list_encodings:false,
           flag_version: false,
           flag_bytes: Some(5),
           flag_split_bytes: Some(2),
           flag_radix:  Some(Radix::X),
           flag_output: None,
           flag_print_file_name: true,
        };
    }

    lazy_static! {
       pub static ref MISSIONS: Missions = Missions::new(&ARGS.flag_encoding,
                                                         &ARGS.flag_control_chars,
                                                         &ARGS.flag_bytes);
    }

    lazy_static! {
       pub static ref MISSIONS2: Missions =
            Missions::new(&vec![
                                    "ASCII".to_string(),
                                    "ASCII,10,80..ff".to_string(),
                                    "ASCII,10,400..7ff".to_string(),
                                    "UTF-16BE".to_string(),
                                    "UTF-16BE".to_string(),
                                    "UTF-16LE".to_string(),
                          ],
                          &ARGS.flag_control_chars, &ARGS.flag_bytes);
    }

    /// Are the `Ordering` traits implemented properly?
    #[test]
    fn test_compare_findings(){

        let smallest = Finding{ filename:None, ptr:5, mission:&MISSIONS2.v[0], s:"".to_string()};
        let smaller = Finding{ filename:None, ptr:5, mission:&MISSIONS2.v[1], s:"".to_string()};
        let small = Finding{ filename:None, ptr:5, mission:&MISSIONS2.v[2], s:"".to_string() };
        let big1 = Finding{ filename:None, ptr:12, mission:&MISSIONS2.v[3], s:"".to_string() };
        let big2 = Finding{ filename:None, ptr:12, mission:&MISSIONS2.v[4], s:"".to_string() };
        let big3 = Finding{ filename:None, ptr:12, mission:&MISSIONS2.v[5], s:"".to_string() };

        assert_eq!(big1, big2);
        assert!(big1 != big3);
        assert!(big3 > big2);
        assert!(smallest < smaller);
        assert!(smaller < small);
        assert!(smallest < small);
        assert!(small < big1);
        assert!(small < big3);
    }


    /// Does the `ScannerPool::scan_window()` respect the minimum constraint?
    #[test]
    fn test_scan_min_bytes(){

       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:0, mission:&MISSIONS.v[0], s:"Hello".to_string() },
                Finding{ filename:None, ptr:12, mission:&MISSIONS.v[0], s:"world!".to_string() }
                ], completes_last_str: false, last_str_is_incomplete: false};

       // The word "new" in "Helloünewüworld!" is too short (<5) and will be ommited.
       let start = 0;
       let inp = "Helloünewüworld!".as_bytes();

       let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[0] };

       let (res, start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 18);



       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:5, mission:&MISSIONS.v[0],
                         s:"How are you?".to_string() },
                ], completes_last_str: false, last_str_is_incomplete: false};

       // The words "Hi!" in "Hi!üHow are you?üHi!" are too short (<5) and will be ommited.
       let start = 0;
       let inp = "Hi!üHow are you?üHi!".as_bytes();

       let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[0] };

       let (res, start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);

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
       //println!("{:?}",inp);


       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:14, mission:&MISSIONS.v[0],
                         s:"\u{fffd}AWAVAUA\u{2691}".to_string() }
                ], completes_last_str: false, last_str_is_incomplete: true};

       let start = 0;

       let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[0] };

       let (res, mut start) = ScannerPool::scan_window(&ms, None, &start, &inp[..WIN_LEN]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, WIN_LEN);



       // Simulate next iteration.
       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:WIN_LEN, mission:&MISSIONS.v[0],
                         s:"\u{2691}TUSH".to_string() },
                ], completes_last_str: true, last_str_is_incomplete: false};

       start -= WIN_STEP;

       let ms = ScannerState{ offset:start, completes_last_str:true, mission:&MISSIONS.v[0] };

       let (res,start) = ScannerPool::scan_window(&ms, None, &WIN_STEP, &inp[WIN_STEP..]);
       assert_eq!(res, expected_fc);
       assert_eq!(start, 17);
    }


    /// Does FindingCollection::close_old_init_new_finding() checks
    /// `ARGS.flag_control_chars`  ?
    #[test]
    fn test_scan_control_chars(){
       // to see output of println on console:  cargo test -- --nocapture
       let inp = "\u{0}\u{0}34567890\u{0}\u{0}345678\u{0}0123\u{0}\u{0}".as_bytes();

       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:0, mission:&MISSIONS.v[0],
                         s:"\u{fffd}34567890\u{fffd}345678".to_string() }
                ], completes_last_str: false, last_str_is_incomplete: true};

       let start = 0;

       let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[0] };

       let (res,start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 25);



       // to see output of println on console:  cargo test -- --nocapture
       let inp = "\u{0}\u{0}\u{0}\u{0}".as_bytes();

       let expected_fc = FindingCollection{ v: vec![], completes_last_str: false,
                                            last_str_is_incomplete: false };

       let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[0] };

       let (res,start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 4);

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
                Finding{ filename:None, ptr:0, mission:&MISSIONS.v[1],
                         s:"12345678901234567890€\u{2691}".to_string() }
                ], completes_last_str: false, last_str_is_incomplete: true};

       let start = 0;

       let mut ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[1] };

       let (res, mut start) = ScannerPool::scan_window(&ms, None, &start, &inp[..WIN_LEN]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 23);

       // Simulate next iteration.
       // The following is problematic because "€" is shorter then ARGS.flag_bytes and
       // will be normally omitted.
       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:23, mission:&MISSIONS.v[1],
                         s:"\u{2691}€".to_string() },
                ], completes_last_str: true, last_str_is_incomplete: false};


       start -= WIN_STEP;
       ms.offset = start;
       ms.completes_last_str = res.last_str_is_incomplete;

       let (res,start) = ScannerPool::scan_window(&ms, None, &WIN_STEP, &inp[WIN_STEP..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 9);
    }


    /// Assume a valid string is cut in the middle and at the cutting edge was
    /// a little graphic string that is also cut in the middle. The little graphic
    /// strings first and second piece will be normally too short.
    /// Here we check if they are printed exceptionally how it should be.
    /// Both small split pieces are > than `ARGS.flag_split_bytes`.
    /// Otherwise they would not have been omitted.
    #[test]
    fn test_scan_incomplete_utf8_simulate_iterations2(){

       // to see output of println on console:  cargo test -- --nocapture
       // 20 Bytes and then [226, 130, 172, 226, 130, 172]
       let inp = "1234567890123456789\u{00}€€".as_bytes();

       // pic the first €
       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:0, mission:&MISSIONS.v[1],
                         s:"1234567890123456789\u{fffd}€\u{2691}".to_string() },
                ], completes_last_str: false, last_str_is_incomplete: true};

       let start = 0;

       let mut ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[1] };

       let (res, mut start) = ScannerPool::scan_window(&ms, None, &start, &inp[..WIN_LEN]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 23);

       // Simulate next iteration.
       // The following is problematic because "€" is shorter then ARGS.flag_bytes and
       // will be normally omitted.
       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:23, mission:&MISSIONS.v[1],
                         s:"\u{2691}€".to_string() },
                ], completes_last_str: true, last_str_is_incomplete: false};


       start -= WIN_STEP;
       ms.offset = start;
       ms.completes_last_str = res.last_str_is_incomplete;

       let (res,start) = ScannerPool::scan_window(&ms, None, &WIN_STEP, &inp[WIN_STEP..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 9);
    }


    /// Assume a valid string is cut in the middle and at the cutting edge was
    /// a little graphic string that is also cut in the middle. The little graphic
    /// strings first and second piece will be normally too short.
    /// Here we check if they are printed exceptionally how it should be.
    #[test]
    fn test_scan_incomplete_utf8_simulate_iterations3(){

       // to see output of println on console:  cargo test -- --nocapture
       // The cutting edge is between "a" anc "b".
       // "a" will not be printed because it is smaller then `ARGS.flag_split_bytes`.
       let inp = "12345678901234567890123\u{00}ab".as_bytes();

       // pic the first €
       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:0, mission:&MISSIONS.v[1],
                         s:"12345678901234567890123".to_string() },
                ], completes_last_str: false, last_str_is_incomplete: true};

       let start = 0;

       let mut ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[1] };

       let (res, mut start) = ScannerPool::scan_window(&ms, None, &start, &inp[..WIN_LEN]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 25);

       // Simulate next iteration.
       // The remaining "b" will not be printed because it is
       // is shorter then `ARGS.flag_split_bytes` (1<2).
       let expected_fc = FindingCollection{ v: vec![
                ], completes_last_str: true, last_str_is_incomplete: false};


       start -= WIN_STEP;
       ms.offset = start;
       ms.completes_last_str = res.last_str_is_incomplete;

       let (res,start) = ScannerPool::scan_window(&ms, None, &WIN_STEP, &inp[WIN_STEP..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 9);
    }


    /// Is mission.offset set properly in case a byte-sequence is overlaps WIN_LEN?
    #[test]
    fn test_scan_incomplete_utf8(){

       // to see output of println on console:  cargo test -- --nocapture
       // 19 Bytes and then [226, 130, 172, 226, 130, 172, 226, 130, 172]
       let inp = "1234567890123456789€€€".as_bytes();

       // pic the first 2 €
       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:0, mission:&MISSIONS.v[1],
                         s:"1234567890123456789€€\u{2691}".to_string() }
                ], completes_last_str: false, last_str_is_incomplete: true};

       let start = 0;

       let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[1] };

       let (res, start) = ScannerPool::scan_window(&ms, None, &start, &inp[..WIN_LEN]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 25);
    }


    /// Are valid strings correctly separated and listed?
    #[test]
    fn test_scan_erroneous_ascii(){

       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:0,  mission:&MISSIONS.v[0], s:"Hello".to_string() },
                Finding{ filename:None, ptr:11,  mission:&MISSIONS.v[0], s:"world!".to_string() }
                ], completes_last_str: false, last_str_is_incomplete: false};

       // Erroneous  Bytes are in the middle only
       let start = 0;
       let inp = "Helloüüüworld!".as_bytes();

       let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[0] };

       let (res, start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 17);


       // Erroneous  Bytes are in the middle and at the end
       let start = 0;
       let inp = "Helloüüüworld!üü".as_bytes();

       let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[0] };

       let (res, start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 18);

    }

    /// Does the loop in `ScannerPool::scan_window()` relaunching `Decoder::raw_feed()`
    /// terminate properly when `mission.offset` has reached WIN_OVERLAP?
    #[test]
    fn test_scan_no_new_search_in_overlapping_space(){

       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:0, mission:&MISSIONS.v[0], s:"Hello".to_string() },
                Finding{ filename:None, ptr:11, mission:&MISSIONS.v[0], s:"world!".to_string() }
                ], completes_last_str: false, last_str_is_incomplete: false};

       // "How are you?" started in overlapping space and should not be found.
       let start = 0;
       let inp = "Helloüüüworld!üüHow are you?".as_bytes();

       let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[0] };


       let (res, start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 18);



       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:0, mission:&MISSIONS.v[0], s:"Hello".to_string() },
                Finding{ filename:None, ptr:11, mission:&MISSIONS.v[0], s:"world!".to_string() }
                ], completes_last_str: false, last_str_is_incomplete: false};


       // "How are you?" starts in overlapping space. So is should be found.
       let start = 0;
       let inp = "Helloüüüworld!üHow are you?".as_bytes();

       let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[0] };

       let (res, start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 18);

    }

    /// Is `Mission::offset()` is interpreted correctly to
    /// instruct the next iteration to print the remaining part of a cut string
    /// even if it is too short?
    #[test]
    fn test_scan_do_not_forget_first_short_string_in_next_chunk(){

       // The next should be found and fill the whole WIN_LEN and 2 Bytes more
       let expected_fc = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:0, mission:&MISSIONS.v[0],
                         s:"Hello world! How do you d\u{2691}".to_string() },
                ], completes_last_str: false, last_str_is_incomplete: true};
       let start = 0;
       let inp = "Hello world! How do you do?".as_bytes();

       let mut ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS.v[0] };

       let (res, mut start) = ScannerPool::scan_window(&ms, None, &start, &inp[..WIN_LEN]);

       assert_eq!(res, expected_fc);
       assert_eq!(start, 25);


       // The following is problematic because "o?" is shorter then ARGS.flag_bytes and
       // will be normally omitted.
       let expected2 = FindingCollection{ v: vec![
                Finding{ filename:None, ptr:25, mission:&MISSIONS.v[0],
                         s:"\u{2691}o?".to_string() },
                ], completes_last_str: true, last_str_is_incomplete: false};

       // Prepare next iteration
       start -= WIN_STEP;
       ms.offset = start;
       ms.completes_last_str = res.last_str_is_incomplete;

       let (res,start) = ScannerPool::scan_window(&ms, None, &WIN_STEP, &inp[WIN_STEP..]);

       assert_eq!(res, expected2);
       assert_eq!(start, 10);

    }




    /// Test the overall behavior of threads used by `ScannerPool::launch_scanner()`.
    #[test]
    fn test_scan_scanner_threads(){
        use std::sync::mpsc;
        use std::thread::JoinHandle;
        use std::thread;

        let merger: JoinHandle<()>;

        {
            let (tx, rx) = mpsc::sync_channel(0);
            let mut sc = ScannerPool::new(&MISSIONS, &tx);

            merger = thread::spawn(move || {

                let expected1 = FindingCollection{
                    v: vec![
                        Finding{ filename:None, ptr:1000, mission:&MISSIONS.v[0],
                                 s:"hallo1234".to_string() },
                        Finding{ filename:None, ptr:1015, mission:&MISSIONS.v[0],
                                 s:"so567890\u{2691}".to_string() },
                    ],
                    completes_last_str: false, last_str_is_incomplete: true
                };

                let expected2 = FindingCollection{
                    v: vec![
                        Finding{ filename:None, ptr:1000, mission:&MISSIONS.v[1],
                                 s:"hallo1234üduüso567890\u{2691}".to_string() },
                    ],
                    completes_last_str: false, last_str_is_incomplete: true
                };

                let res1 = rx.recv().unwrap();
                let res2 = rx.recv().unwrap();
                /*
                println!("expected 1: {:?}",expected1);
                println!("res 1: {:?}",res1);
                println!("expected 2: {:?}",expected2);
                println!("res 2: {:?}",res2);
                */

                assert!((expected1 == res1) || (expected1 == res2));
                assert!((expected2 == res1) || (expected2 == res2));
                //println!("Merger terminated.");
            });

            sc.launch_scanner(None, &1000usize, &"hallo1234üduüso567890".as_bytes() );
            assert_eq!(sc.scanner_states.v[0].offset, 6);
            //assert_eq!(sc.scanner_states.v[1].offset, 6);

        } // tx drops here

        merger.join().unwrap();
    }


    lazy_static! {
       pub static ref MISSIONS3: Missions =
            Missions::new(&vec![
                                "ASCII".to_string(),
                                "utf-16be".to_string(),
                                "euc-jp".to_string(),
                                "koi8-u".to_string(),
                          ],
                          &ARGS.flag_control_chars, &ARGS.flag_bytes);
    }

    /// Feed `ScannerPool::scan_window()` with random data to test its robustness.
    #[test]
    #[allow(unused_variables)]
    fn test_scan_random_input(){
        use self::rand::Rng;

        let mut rng = rand::thread_rng();
        for _ in 0..0x1000 {
            let inp = (0..WIN_LEN).map(|_| rng.gen::<u8>()).collect::<Vec<u8>>();

            let start=0;
            let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS3.v[0] };
            let (res0,start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);


            let start=0;
            let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS3.v[1] };
            let (res1,start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);


            let start=0;
            let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS3.v[2] };
            let (res2,start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);


            let start=0;
            let ms = ScannerState{ offset:0, completes_last_str:false, mission:&MISSIONS3.v[3] };
            let (res3,start) = ScannerPool::scan_window(&ms, None, &start, &inp[..]);



            // To see println! output:  cargo test   -- --nocapture
            /*
            println!("Scan of random Bytes:{:?} {:?} {:?} {:?}",
                   res0.v, res1.v, res2.v, res3.v);
            */
        };
    }

}
