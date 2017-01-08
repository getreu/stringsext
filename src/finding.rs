//! This module defines data structures to store and process found strings (findings) in memory.
use std::io::prelude::*;
use std::str;
extern crate memmap;
extern crate itertools;


extern crate encoding;
use encoding::{StringWriter};
use std::fmt;
use std::cmp::{Ord,Eq};
use std::cmp;

use scanner::ScannerState;

#[cfg(not(test))]
use options::ARGS;
#[cfg(test)]
use self::tests::ARGS;

use options::Radix;
use options::ControlChars;

#[cfg(not(test))]
use input::WIN_STEP;
#[cfg(test)]
use self::tests::WIN_STEP;

#[cfg(test)]
use self::tests::CONTROL_REPLACEMENT_STR;
#[cfg(not(test))]
lazy_static! {
    /// Before printing a valid string, all its control characters are eliminated.
    /// This variable contains an optional marker indicating the location where data was
    /// deleted.
    pub static ref CONTROL_REPLACEMENT_STR : &'static str =
            if  ARGS.flag_control_chars == ControlChars::R { "\u{fffd}" } else { "\n" };
}


use Mission;

/// Initial capacity of the findings vector is
/// set to WIN_STEP / 32.
pub const FINDING_COLLECTION_CAPACITY: usize = WIN_STEP >> 5;
/// Initial capacity of finding string in bytes
pub const FINDING_STR_CAPACITY: usize = 100;




/// `Finding` represents a valid result string with it's found location and
/// original encoding.
pub struct Finding {
    /// A copy of the `byte_counter` pointing at the found location of the result string.
    pub ptr: usize,
    /// Mission associated with this finding
    pub mission: &'static Mission,
    /// Whatever the original encoding was the result string `s` is always stored as UTF-8.
    pub s: String,
}


/// Prints the meta information of a finding: e.g. "(ascii/U+40..U+7f)" or "(utf-8)"
macro_rules! enc_str {
    ($finding:ident) => {{
                // Check if the filter is restrictive
                format!("({}{}{})",
                        $finding.mission.encoding.name(),
                        if $finding.mission.filter1.is_some {
                            format!("/{:x}..{:x}",
                                 $finding.mission.filter1.and_result,
                                 $finding.mission.filter1.and_result|!(
                                             $finding.mission.filter1.and_mask))
                        } else { "".to_string() },
                        if $finding.mission.filter2.is_some {
                            format!("/{:x}..{:x}",
                                 $finding.mission.filter2.and_result,
                                 $finding.mission.filter2.and_result|!(
                                             $finding.mission.filter2.and_mask))
                        } else { "".to_string() }

                )
    }}
}

impl Finding {
    /// Format and dump a Finding to the output channel,
    /// usually stdout.
    pub fn print(&self, out: &mut Box<Write>) {

        if ARGS.flag_control_chars == ControlChars::R {
            let ptr_str = match ARGS.flag_radix {
                Some(Radix::X) => format!("{:0x}\t",  self.ptr),
                Some(Radix::D) => format!("{:0}\t",  self.ptr),
                Some(Radix::O) => format!("{:0o}\t", self.ptr),
                None           => "".to_string(),
            };

        let enc_str = if ARGS.flag_encoding.len() > 1 {
                enc_str!(self)+"\t"
        } else {
                "".to_string()
        };

            for l in  self.s.lines() {
                &out.write_all(format!("{}{}{}\n",ptr_str, enc_str, l).as_bytes() );
            }
        } else {
            let mut ptr_str = match ARGS.flag_radix {
                Some(Radix::X) => format!("{:7x} ",  self.ptr),
                Some(Radix::D) => format!("{:7} ",  self.ptr),
                Some(Radix::O) => format!("{:7o} ", self.ptr),
                None           => "".to_string(),
            };
            let ptr_str_ff = match ARGS.flag_radix {
                Some(_)        => "        ",
                None           => "",
            };

        let enc_str = if ARGS.flag_encoding.len() > 1 {
                format!("{:14}\t",enc_str!(self))
        } else {
                "".to_string()
        };

            for l in  self.s.lines() {
                &out.write_all(format!("{}{}{}\n",ptr_str, enc_str, l).as_bytes() );
                ptr_str = ptr_str_ff.to_string();
            }
        }
    }
}

impl Eq for Finding  {
}

// Useful to compare findings for debugging or testing.
impl PartialEq for Finding  {
    fn eq(&self, other: &Self) -> bool {
        (self.ptr == other.ptr) &&
        (self.mission.encoding.name() == other.mission.encoding.name()) &&
        (self.mission.filter1 == other.mission.filter1) &&
        (self.mission.filter2 == other.mission.filter2) &&
        (self.s == other.s)
    }
}

/// We first compare `ptr` then `enc`. `s` is disregarded because  (`ptr`, `enc`,
/// `u_and_result` and `u_and_result`) are unique in a finding collection.
// We need this to merge later
impl Ord for Finding {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self.ptr != other.ptr {
            self.ptr.cmp(&other.ptr)
        } else if self.mission.encoding.name() != other.mission.encoding.name() {
                    self.mission.encoding.name().cmp(&other.mission.encoding.name())
               } else if self.mission.filter1 != other.mission.filter1 {
                        self.mission.filter1.cmp(&other.mission.filter1)
                      } else {
                        self.mission.filter2.cmp(&other.mission.filter2)
                      }
    }
}


/// We first compare `ptr` then `enc`. `s` is disregarded because  (`ptr`, `enc`,
/// `u_and_result` and `u_and_result`) are unique in a finding collection.
// We need this to merge later
impl PartialOrd for Finding {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        if self.ptr != other.ptr {
            self.ptr.partial_cmp(&other.ptr)
        } else if self.mission.encoding.name() != other.mission.encoding.name() {
                    self.mission.encoding.name().partial_cmp(&other.mission.encoding.name())
               } else if self.mission.filter1 != other.mission.filter1 {
                        self.mission.filter1.partial_cmp(&other.mission.filter1)
                      } else {
                        self.mission.filter2.partial_cmp(&other.mission.filter2)
                      }
    }
}


impl fmt::Debug for Finding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "\n{}\t({})\t{}", self.ptr, self.mission.encoding.name(),
                self.s.replace("\n"," \\n ").replace("\r\n"," \\r\\n "))
    }
}



/// If `control_chars==true` transform a valid (current) string into a string
/// containing only graphical character sequences with a minimum length by deleting all
/// control characters.
/// Note that the current string is always the one
/// at the end of a `FindingCollection`.
///
/// We check if the current string satisfies the following:
/// 1. We list all strings between control chars.
/// 2. Are some of the strings between control chars shorter
///    then the required minimum length? If yes we delete them.
/// 3. All control chars are replaced with Unicode Character
///    'REPLACEMENT CHARACTER' (U+FFFD) and then grouped.
///
/// The case `enable_filter==false` is treated as special case:
/// No control character are filtered and the string is kept as a whole
/// (the list has only one chunk).
/// Notwithstanding, too short strings are dismissed.
///
/// This macro should be called when the string is complete, meaning before
/// starting a new finding. This macro panics when there is no string
/// in the `FindingCollection`.
///
/// The macro is used in FindingCollection::close_old_init_new_finding()
///
macro_rules! filter {
    ($fc:ident,
     $mission:ident) => {{
        let minsize = $mission.nbytes_min as usize;

        if ($fc.v.last().unwrap().s.len() < minsize) && !$fc.completes_last_str {
                $fc.v.last_mut().unwrap().s.clear();

        } else if $mission.enable_filter {
        // filter control chars, group and delete short ones in between.
            let len = $fc.v.last().unwrap().s.len();
            let mut out = String::with_capacity(len + 0x10); // a bit bigger, just in case
            {
                let mut chunks = (&$fc).v.last().unwrap().s
                    .split_terminator(|c: char|
                       c !=' '  &&  c !='\t'
                       &&( c.is_control()
                           ||
                           !(
                                (   // no filter
                                    !($mission.filter1.is_some || $mission.filter2.is_some)
                                )
                                ||
                                (
                                    $mission.filter1.is_some &&
                                    (((c as u32)& $mission.filter1.and_mask)
                                                 == $mission.filter1.and_result)
                                )
                                ||  // Union
                                (
                                    $mission.filter2.is_some &&
                                    (((c as u32)& $mission.filter2.and_mask)
                                                 == $mission.filter2.and_result)
                                )
                           )
                       )
                    )
                    .enumerate()
                    .filter(|&(n,s)| (s.len() >= minsize ) ||
                                     ((n == 0) && $fc.completes_last_str)
                           )
                    //.inspect(|&(n,s)| println!("n: {}, s: {}, len(s): {}",n,s,s.len()))
                    .map(|(_, s)| s );

                if let Some(first_chunk) = chunks.next() {
                    if !$fc.v.last().unwrap().s.starts_with(&first_chunk) {
                        out.push_str(&CONTROL_REPLACEMENT_STR); // only if Some(first_chunk)
                    }
                    out.push_str(first_chunk);  // push the first
                    for chunk in chunks {       // and the rest if there is
                        out.push_str(&CONTROL_REPLACEMENT_STR);
                        out.push_str(chunk);
                    }
                }
            };

            // Replace current string with filtered one
            $fc.v.last_mut().unwrap().s.clear();
            $fc.v.last_mut().unwrap().s.push_str(&*out);
        }

        // Apply `completes_last_str` exactly one time only
        $fc.completes_last_str = false;

    }};
}


/// Represents a list of findings, i.e. UTF-8 strings. The last position
/// in the list is referred as `current string` or `current finding string`.
/// The current string is edited in stages by `Scanner::StringWriter` functions.
/// The re-invocation of `close_old_init_new_finding()`
/// will freeze and close the current string.
///
#[derive(Debug,PartialEq)]
pub struct FindingCollection {
    /// List of `Finding`s. The last is referred as _current string_ or _current finding
    /// string_.
    pub v: Vec<Finding>,
    /// `Scanner::scan_window()` sets this flag to true when it recognizes that this scan
    /// continues an incomplete string from the previous scan.
    /// (It is possible to deduce this information from the start pointer).
    /// `close_old_init_new_finding()` will then retain the first finding even if
    /// it is normally too short according to `ARG.flag_bytes` instructions.
    pub completes_last_str: bool,
}


impl FindingCollection {
    pub fn new(ms: &ScannerState) -> FindingCollection{
        let mut fc = FindingCollection{
                v: Vec::with_capacity(FINDING_COLLECTION_CAPACITY),
                completes_last_str: ms.completes_last_str };
        fc.v.push( Finding{ ptr: 0,
                            mission: &(*ms).mission,
                            s: String::with_capacity(FINDING_STR_CAPACITY) } );
        fc
    }

    /// This method formats and dumps a `FindingCollection` to the output channel,
    /// usually `stdout`.
    #[allow(dead_code)]
    pub fn print(&self, out: &mut Box<Write>) {
        if (&self).v.len() == 0 { return };
        for finding in &self.v {
            finding.print(out);
        }
    }

    /// `close_old_init_new_finding` works closely together with `StringWriter` functions
    /// (see below) who append Bytes in stages at the end of the current finding string in a
    /// `FindingCollection`.  The next re-invocation of `close_old_init_new_finding()` freezes
    /// the current finding string and appends a new empty `Finding` to the
    /// `FindingCollection` that will contain the new current finding string. At the same
    /// time the `text_ptr` and `enc` are recorded. Note there is no actual content string yet
    /// in the new `Finding`. The actual content will be added with the next call of a
    /// `StringWriter` function (see below).

     pub fn close_old_init_new_finding(&mut self, text_ptr: usize, mission: &'static Mission) {

        if self.v.last().unwrap().s.len() != 0 {  // last is not empty

           filter!(self, mission);
        };

        // We have check again because len() may have changed in the line above
        if self.v.last().unwrap().s.len() != 0 {
            self.v.push(Finding{ ptr: text_ptr,
                                 mission: mission,
                                 s: String::with_capacity(FINDING_STR_CAPACITY) });
        } else {
                    // The current finding is empty, we do not
                    // push a new finding, instead we
                    // only update the pointer of the current
                    // one. Content will come later anyway.
            self.v.last_mut().unwrap().ptr = text_ptr;
        };
    }

    /// This method removes the last `Finding`. This method should called directly after
    /// the last `close_old_init_new_finding()` call.
    ///
    pub fn close_finding_collection(&mut self) {
            let l = self.v.len();
            self.v.remove(l-1);
    }
}



/// The `Encoding::StringWriter` trait is the way the `Encoding::raw_decoder()`
/// incrementally sends its output. Note that here all member functions operate
/// exclusively on the last string in a `FindingCollection` referred as "current string".
/// A current string can be closed calling `FindingCollection::close_old_init_new_finding()`.
/// This will append an empty string at then end of the `FindingCollection`
/// and `StringWriter` will use the new one from now on.
impl StringWriter for FindingCollection {
    fn writer_hint(&mut self, expectedlen: usize) {
        let newlen = self.v.last().unwrap().s.len() + expectedlen;
        self.v.last_mut().unwrap().s.reserve(newlen);
    }
    /// Appends a `char` to the current finding string.
    /// The "current finding string" is the string of the last `Finding` in this
    /// `FindingCollection` vector.
    fn write_char(&mut self, c: char) {
            self.v.last_mut().unwrap().s.push(c);
    }

    /// Appends a `&str` to the current finding string.
    /// The "current finding string" is the string of the last `Finding` in this
    /// `FindingCollection` vector.
    fn write_str(&mut self, s: &str) {
            self.v.last_mut().unwrap().s.push_str(s);
    }
}





#[cfg(test)]
mod tests {
    use super::*;
    use options::{Args, Radix, ControlChars};
    use UnicodeBlockFilter;
    extern crate encoding;
    use std::str;
    extern crate rand;

    pub const WIN_STEP: usize = 17;
    pub const CONTROL_REPLACEMENT_STR: &'static &'static str = &"\u{fffd}";

    lazy_static! {
        pub static ref ARGS:Args = Args {
           arg_FILE: vec!["myfile.txt".to_string()],
           flag_control_chars:  ControlChars::R,
           flag_encoding: vec!["ascii".to_string(), "utf8".to_string()],
           flag_list_encodings: false,
           flag_version: false,
           flag_bytes:  Some(5),
           flag_radix:  Some(Radix::X),
           flag_output: None,
        };
    }


    /// Test the filter macro
    #[test]
    fn test_scan_filter(){
       use Mission;
       // Replace mode: the last 1234 is too short

       static M1:Mission = Mission{ encoding: encoding::all::ASCII,
                        filter1: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        filter2: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        nbytes_min: 5,
                        enable_filter: true,
       };

       let mut input = FindingCollection{
            v: vec![
                Finding{ ptr:0, mission:&M1,
                    s:"\u{0}\u{0}34567890\u{0}\u{0}2345678\u{0}1234\u{0}\u{0}".to_string()
                },
            ],
            completes_last_str: false
       };

       let expected = FindingCollection{
            v: vec![
                Finding{ ptr:0, mission:&M1, s:"\u{fffd}34567890\u{fffd}2345678".to_string() },
            ], completes_last_str: false
       };

       filter!(input, M1); // Mode -c r (replace)
       assert_eq!(input, expected);


       // With completes_last_str set "ab" is printed (exception) but "1234" not
       static M2: Mission = Mission {encoding: encoding::all::ASCII,
                        filter1: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        filter2: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        nbytes_min: 5,
                        enable_filter: true,
       };

       let mut input = FindingCollection{ v: vec![
                Finding{ ptr:0, mission:&M2, s:"ab\u{0}1234\u{0}\u{0}".to_string() },
                ], completes_last_str: true};

       let expected = FindingCollection{ v: vec![
                Finding{ ptr:0, mission:&M2, s:"ab".to_string() },
                ], completes_last_str: false};

       filter!(input, M2); // Mode -c r (replace)

       assert_eq!(input, expected);



       // With completes_last_str unset "ab" is not printed
       static M3:Mission = Mission {encoding: encoding::all::ASCII,
                        filter1: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        filter2: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        nbytes_min: 5,
                        enable_filter: true,
       };

       let mut input = FindingCollection{ v: vec![
                Finding{ ptr:0, mission:&M3, s:"\u{0}ab\u{0}1234\u{0}\u{0}".to_string() },
                ], completes_last_str: false};

       let expected = FindingCollection{ v: vec![
                Finding{ ptr:0, mission:&M3, s:"".to_string() },
                ], completes_last_str: false};

       filter!(input, M3); // Mode -c r (replace)
       assert_eq!(input, expected);



       // Replace mode: the last 1234 is too short
       static M4:Mission = Mission {encoding: encoding::all::ASCII,
                        filter1: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        filter2: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        nbytes_min: 5,
                        enable_filter: true,
       };


       let mut input = FindingCollection{ v: vec![
                Finding{ ptr:0, mission:&M4, s:"\u{0}\u{0}\u{0}\u{0}1234\u{0}\u{0}".to_string() },
                ], completes_last_str: false};

       let expected = FindingCollection{ v: vec![
                Finding{ ptr:0, mission:&M4, s:"".to_string() },
                ], completes_last_str: false};
       filter!(input, M4); // Mode -c r (replace)

       assert_eq!(input, expected);


       // Replace mode: 12 is too short
       static M5:Mission = Mission {encoding: encoding::all::ASCII,
                        filter1: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                      is_some:false },
                        filter2: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        nbytes_min: 5,
                        enable_filter: true,
       };

       let mut input = FindingCollection{ v: vec![
                Finding{ ptr:  0, mission:&M5, s: "12\u{0}\u{0}34567\u{0}89012\u{0}".to_string() },
                ], completes_last_str: false};

       let expected = FindingCollection{ v: vec![
                Finding{ ptr:  0, mission:&M5, s: "\u{fffd}34567\u{fffd}89012".to_string() },
                ], completes_last_str: false};


       filter!(input, M5); // Mode -c r (replace)

       assert_eq!(input, expected);



       // Print all mode (-c p): all should pass
      static M6:Mission = Mission {encoding: encoding::all::ASCII,
                        filter1: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        filter2: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        nbytes_min: 5,
                        enable_filter: false,
      };

      let mut input = FindingCollection{
            v: vec![
                Finding{ ptr:0, mission:&M6,
                         s:"\u{0}\u{0}34567890\u{0}\u{0}2345678\u{0}1234\u{0}\u{0}".to_string()
                },
            ], completes_last_str: false
      };

      let expected = FindingCollection{
            v: vec![
                Finding{ ptr:0, mission:&M6,
                         s:"\u{0}\u{0}34567890\u{0}\u{0}2345678\u{0}1234\u{0}\u{0}".to_string() },
            ], completes_last_str: false
      };

      filter!(input, M6);
      assert_eq!(input, expected);



      // Print all mode (-c p): even though the input string is too short, print
      // because completes_last_str is set.
      static M7:Mission = Mission {encoding: encoding::all::ASCII,
                       filter1: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                       filter2: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                       nbytes_min: 5,
                       enable_filter: false,
      };
      let mut input = FindingCollection{
            v: vec![
               Finding{ ptr:  0, mission:&M7, s: "\u{0}\u{0}34".to_string() },
            ], completes_last_str: true
      };

      let expected = FindingCollection{
            v: vec![
               Finding{ ptr:  0, mission:&M7, s: "\u{0}\u{0}34".to_string() },
            ], completes_last_str: false
      };

      filter!(input, M7); // Mode -c p (print all)

      assert_eq!(input, expected);



      // Print all mode (-c p): the this input string is too short
      static M8:Mission = Mission {encoding: encoding::all::ASCII,
                       filter1: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                       filter2: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                       nbytes_min: 5,
                       enable_filter: false,
      };

      let mut input = FindingCollection{
            v: vec![ Finding{ ptr:0, mission:&M8, s:"\u{0}\u{0}34".to_string() }, ],
            completes_last_str: false
      };

      let expected = FindingCollection{
            v: vec![ Finding{ ptr:  0, mission:&M8, s: "".to_string() }, ],
            completes_last_str: false
      };


      filter!(input, M8); // Mode -c p (print all)

      assert_eq!(input, expected);
    }

    /// Test the Unicode filter in macro
    #[test]
    fn test_scan_unicode_filter(){
       use Mission;
       // This filter is not restrictive, everything should pass
       static M9:Mission = Mission {encoding: encoding::all::ASCII,
                        filter1: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        filter2: UnicodeBlockFilter { and_mask:0xffe00000, and_result: 0,
                                                          is_some:false },
                        nbytes_min: 5,
                        enable_filter: true,
       };

       let mut input = FindingCollection{
            v: vec![
                Finding{ ptr:0, mission:&M9,
                         s:"Hi, \u{0263a}how are{}++1234you++\u{0263a}doing?".to_string() },
                ],
            completes_last_str: false
       };

       let expected = FindingCollection{
            v: vec![
                Finding{ ptr:0, mission:&M9,
                         s:"Hi, \u{0263a}how are{}++1234you++\u{0263a}doing?".to_string() },
                ],
            completes_last_str: false
       };

       filter!(input, M9); // Mode -c r (replace)
       assert_eq!(input, expected);


       // This filter _is_ restrictive, only chars in range `U+60..U+7f` will pass:
       // "_`abcdefghijklmnopqrstuvwxyz{|}~DEL"
       // (space and tab pass always)
       static M10:Mission = Mission {encoding: encoding::all::ASCII,
                        filter1: UnicodeBlockFilter { and_mask: 0xffffffe0,
                                                      and_result: 0x60,
                                                      is_some: true },
                        filter2: UnicodeBlockFilter { and_mask: !0x0001f,
                                                      and_result: 0x00020,
                                                      is_some:true },
                        nbytes_min: 3,
                        enable_filter: true,
       };

       let mut input = FindingCollection{
            v: vec![
                Finding{ ptr:  0, mission:&M10,
                         s: "Hi \u{0263a}How are{}\tÜyou\u{0263a}doing?".to_string() },
                ], completes_last_str: false
       };

       let expected = FindingCollection{
            v: vec![
                Finding{ ptr:  0, mission:&M10,
                         s: "\u{fffd}ow are{}\t\u{fffd}you\u{fffd}doing?".to_string() },
                ],
            completes_last_str: false
       };

       filter!(input, M10); // Mode -c r (replace)

       assert_eq!(input, expected);


       // This filter _is_ restrictive, only chars in range `U+60..U+7f` will pass:
       // "_`abcdefghijklmnopqrstuvwxyz{|}~DEL"
       // (space and tab pass always)
       // Second filter prints also 0x20-0x3f.
       static M11:Mission = Mission {encoding: encoding::all::ASCII,
                        filter1: UnicodeBlockFilter { and_mask:0xffffffe0,
                                                      and_result: 0x60,
                                                      is_some:true },
                        filter2: UnicodeBlockFilter { and_mask: !0x0001f,
                                                      and_result: 0x00020,
                                                      is_some:true },
                        nbytes_min: 2,
                        enable_filter: true,
       };

       let mut input = FindingCollection{
            v: vec![
                Finding{ ptr:0, mission:&M11,
                         s:"Hi! \u{0263a}How are{}\t++1234you?++\u{0263a}doing?".to_string() },
            ],
            completes_last_str: false
       };

       let expected = FindingCollection{
            v: vec![
                Finding{ ptr:  0, mission:&M11,
                         s:"\u{fffd}i! \u{fffd}ow are{}\t++1234you?++\u{fffd}doing?".to_string() },
            ],
            completes_last_str: false
       };

       filter!(input, M11); // Mode -c r (replace)

       assert_eq!(input, expected);
    }
}
