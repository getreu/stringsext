//! This `main` module uses the `options` module to read its command-line-arguments.
//! It defines code for spawning the _merger-thread_ who
//! collects the results produced by the worker threads.
//! The processing of the input-data is initiated by the `input`-module that itself uses
//! the `scanner` module in which the worker-threads are spawned.

mod input;
use input::{process_input};

extern crate rustc_serialize;
extern crate docopt;
#[macro_use]
extern crate lazy_static;

mod options;
use options::ARGS;
use options::ControlChars;

mod scanner;
use scanner::Scanner;

mod finding;

mod codec {
    pub mod ascii;
}
use codec::ascii::ASCII_GRAPHIC;

use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::str;
use std::fmt;
use std::process;


extern crate memmap;
extern crate itertools;
use std::sync::mpsc;

extern crate scoped_threadpool;
use std::thread;

extern crate encoding;
use std::thread::JoinHandle;
use std::io;
use std::num::ParseIntError;
use std::str::FromStr;
use encoding::EncodingRef;
use encoding::label::encoding_from_whatwg_label;
use encoding::all;
use itertools::kmerge;
use itertools::Itertools;

// Version is defined in ../Cargo.toml
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
const AUTHOR: &'static str = "(c) Jens Getreu, 2016";


/// `Mission` represents the instruction data provided individually to each thread in
/// `Scanner::scan()`. Unlike the constant `encoding`, the offset variable is updated
/// dynamically each iteration in order to always point to the starting
/// Byte in the following WIN_LEN window. This will be used by the next
/// `Scanner::scan_window()` call.
pub struct Mission {
    /// Every thread gets a constant encoding to search for.
    encoding : EncodingRef,

    /// Unicode character filter: if ( c && filter_and_mask ) == filter_and_result then print c
    u_and_mask: u32,

    /// Unicode character filter: if ( c && filter_and_mask ) == filter_and_result then print c
    u_and_result: u32,

    /// Minimum required string length in Bytes for a finding to be printed.
    nbytes_min: u8,

    /// Some decoders return strings containing also control characters.
    /// These decoders need a special post treatment filtering like:
    /// scanner::filter!()
    enable_filter: bool,

    /// The position relative to a WIN_LEN window used to start the next iteration search at.
    /// This value is updated after each iteration to ensuring that the next
    /// iteration starts the scanning exactly where the previous stopped.
    offset: usize,
}

impl fmt::Debug for Mission {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Encoding: {}, Offset:{}", self.encoding.name(), self.offset)
    }
}

/// Every `--encoding` option is stored in a `Mission` object which are bound together in a
/// `Missions` object. This is used later in `Scanner::launch_scanner()` where every
/// `Mission` will be assigned to one thread for execution.
#[derive(Debug)]
pub struct Missions {
    /// Vector of `Mission`s.
    v: Vec<Mission>
}

impl Missions {
    /// Constructor. We assume that at least one encoding exist.
    pub fn new(encodings: &Vec<String>, control_chars: &ControlChars) -> Self {
        let mut v = Vec::new();

        let control_char_filtering = match *control_chars {
            ControlChars::R       => true,
            ControlChars::P       => false,
            ControlChars::I       => true,
        };

        for enc_opt in encodings.iter() {
            let (enc_name, nbytes_min, unicode_and_mask, unicode_and_result, unicode_filtering) =
                match Self::parse_enc_opt(&enc_opt, ARGS.flag_bytes.unwrap() ) {
                    Ok(r)  => r,
                    Err(e) => {
                        writeln!(&mut std::io::stderr(),
                            "Error: {} parsing `{}`.",e,&enc_opt).unwrap();
                        process::exit(1);
                    }
                };



            // The next if is a workaround for a bug in EncodingRef:
            // ASCII is translated into windows-1252 instead of pure ASCII.
            if enc_name == "ascii" {
                if *control_chars == ControlChars::I {
                    v.push(Mission {
                        // For ASCII with `-c i` we use our own decoder
                        encoding: ASCII_GRAPHIC as encoding::EncodingRef,
                        u_and_mask: unicode_and_mask,
                        u_and_result: unicode_and_result,
                        nbytes_min: nbytes_min,
                        enable_filter: unicode_filtering,
                        offset: 0,
                    })
                } else {
                    v.push(Mission {
                        encoding: encoding::all::ASCII as encoding::EncodingRef,
                        u_and_mask: unicode_and_mask,
                        u_and_result: unicode_and_result,
                        nbytes_min: nbytes_min,
                        enable_filter: control_char_filtering || unicode_filtering,
                        offset: 0,
                    })
                }
                continue;
            }


            v.push(match encoding_from_whatwg_label(enc_name) {

                Some(enc) => Mission {
                            encoding: enc,
                            u_and_mask: unicode_and_mask,
                            u_and_result: unicode_and_result,
                            nbytes_min: nbytes_min,
                            enable_filter: control_char_filtering || unicode_filtering,
                            offset: 0
                        },
                None => {
                    writeln!(&mut std::io::stderr(),
                          "Error: Invalid input encoding name '{}', try option -l.",
                          enc_name).unwrap();
                    process::exit(1);
                }
            });
        };

        Missions{v: v}
    }

    /// Return the number of `Mission`s stored.
    pub fn len(&self) -> usize {
        self.v.len()
    }

    /// Helper function to parse enc_opt.
    fn parse_enc_opt <'a>(enc_opt:&'a str, nbytes_min_default:u8)
                     -> Result<(&'a str,u8,u32,u32,bool),ParseIntError> {
    
        // Parse ',' separated strings
        let mut i = enc_opt.split_terminator(',');
        let enc_name = i.next().unwrap_or("");
        let opt = i.next();
        let nbytes_min = match opt {
            Some(s) => try!(u8::from_str(s)),
            None    => nbytes_min_default
        };
        let range:&str = i.next().unwrap_or("");
    
        // Separate and parse the range string
        let mut j = range.split_terminator("..")
                         .map(|s|s.trim_left_matches("U+"))
                         .map(|s|  u32::from_str_radix(s,16) );
    
        let u_lower:u32 = try!(j.next().unwrap_or(Ok(0)));
        let u_upper:u32 = try!(j.next().unwrap_or(Ok(std::char::MAX as u32)));
    
    
        // CALCULATE FILTER PARAMETERS
    
        // u_and_mask is 0 from right up to the highest bit that has changed
        let u_changed_bits:u32 = u_upper ^ u_lower;
        let u_next_pow = u_changed_bits.next_power_of_two();
        let u_and_mask = !(u_next_pow -1);
    
        // enlarge boundaries to fit u_and_mask
        let u_lower_ext = u_lower & u_and_mask;
        let u_upper_ext = u_upper | !u_and_mask;
    
        // if enlarged, print a warning
        if !((u_lower == 0) && (u_upper == std::char::MAX as u32)) &&
            ((u_lower != u_lower_ext) || (u_upper != u_upper_ext)) {
            writeln!(&mut std::io::stderr(),
                     "Warning: range in `{}` extended to range `U+{:x}..U+{:x}`.",
                     enc_opt, u_lower_ext, u_upper_ext).unwrap();
        }

        // Check if the filter is restrictive
        let filtering = u_and_mask != !((std::char::MAX as u32).next_power_of_two()-1);
    
        // these 2 are need by filter
        let filter_and_mask = u_and_mask;
        let filter_and_result = u_lower_ext;
    
        // Return the results
        Ok((enc_name, nbytes_min, filter_and_mask, filter_and_result, filtering))
    }
}


/// This function spawns and defines the behaviour of the _merger-thread_ who
/// collects and prints the results produced by the worker threads.
fn main() {

    if ARGS.flag_list_encodings  {
        let list = all::encodings().iter().filter_map(|&e|e.whatwg_name()).sorted();
        // Available encodings
        for e in list {
            println!("{}",e);
        }
        return;
    }

    if ARGS.flag_version {
        println!("Version {}, {}", VERSION.unwrap_or("unknown"), AUTHOR );
        return;
    }


    let missions = Missions::new(&ARGS.flag_encoding, &ARGS.flag_control_chars);

    let merger: JoinHandle<()>;
    // Scope for threads
    {
        let n_threads = missions.len();
        let (tx, rx) = mpsc::sync_channel(n_threads);
        let mut sc = Scanner::new(missions, &tx);

        // Receive `FindingCollection`s from scanner threads.
        merger = thread::spawn(move || {
            let mut output = match ARGS.flag_output {
               Some(ref fname) => {
                            let f = File::create(&Path::new(fname.as_str())).unwrap();
                            Box::new(f) as Box<Write>
                        },
               None  => Box::new(io::stdout()) as Box<Write>,
            };

            'outer: loop {
                let mut results = Vec::with_capacity(n_threads);
                for _ in 0..n_threads {
                    results.push(match  rx.recv() {
                        Ok(fc)  => {
                            //fc.print(&mut output);
                            fc.v
                        },
                        Err(_) => {break 'outer},
                    });
                };
                //   merge
                for finding in kmerge(&results) {
                    finding.print(&mut output);
                };
            }
            //println!("Merger terminated.");
        });

        // Default for <file> is stdin.
        let file_path_str = if let Some(ref s) = ARGS.arg_FILE {
            s
        } else {
            "-"
        };

        match process_input(&file_path_str, &mut sc) {
            Err(e)=> {
                    writeln!(&mut std::io::stderr(),
                          "Error: {}.",
                          e.to_string()).unwrap();
                    process::exit(2);
            },
            _ => {},
        }

    } // `tx` drops here, which "break"s the merger-loop.
    merger.join().unwrap();

    //println!("All threads terminated.");
}



#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use std::num::ParseIntError;

    #[test]
    fn test_enc_opt_parser () {
        //let pie = ParseIntError {kind: std::num::InvalidDigit} //is private
        let pie_invalid_digit: ParseIntError = u32::from_str("x").unwrap_err();
        //let pie = ParseIntError {kind: std::num::Overflow} //is private
        let pie_overflow: ParseIntError = u8::from_str("257").unwrap_err();


        assert_eq!(super::Missions::parse_enc_opt("",8),
           Ok(("",8,0xffe00000,0x0,false)));

        assert_eq!(super::Missions::parse_enc_opt("ascii",8),
           Ok(("ascii",8,0xffe00000,0x0,false)));

        // range in `ascii,U+41..U+67` extended to range `U+40..U+7f`
        assert_eq!(super::Missions::parse_enc_opt("ascii,10,U+41..U+67",8),
           Ok(("ascii",10,0xffffffc0,0x40,true)));

        // small letters, range is extended to `U+60..U+7f`
        assert_eq!(super::Missions::parse_enc_opt("ascii,10,U+61..U+7a",8),
           Ok(("ascii",10,0xffffffe0,0x60,true)));

        assert_eq!(super::Missions::parse_enc_opt("ascii,10,U+4?1..U+67",8).unwrap_err(),
           pie_invalid_digit );

        assert_eq!(super::Missions::parse_enc_opt("ascii,10,U+41..U+6?7",8).unwrap_err(),
           pie_invalid_digit );

        assert_eq!(super::Missions::parse_enc_opt("ascii,10,U+4?1..U+67",8).unwrap_err(),
           pie_invalid_digit );

        // range in `ascii,U+401..U+482,10` extended to range `U+400..U+4ff`
        assert_eq!(super::Missions::parse_enc_opt("ascii,10,U+401..U+482",8),
           Ok(("ascii",10,0xffffff00,0x400,true)));

        // range in `ascii,10,U+40e..U+403,10` extended to range `U+400..U+40f`
        assert_eq!(super::Missions::parse_enc_opt("ascii,10,U+40e..U+403",8),
           Ok(("ascii",10,0xfffffff0,0x400,true)));

        assert_eq!(super::Missions::parse_enc_opt("ascii,256,U+41..U+67",8).unwrap_err(),
           pie_overflow );

        assert_eq!(super::Missions::parse_enc_opt("ascii,10,U+fffffffff..U+67",8).unwrap_err(),
           pie_overflow );

    }
}
