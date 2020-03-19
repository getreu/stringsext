//! `stringsext` searches for multi-byte encoded strings in binary data.\

//! `stringsext` is a Unicode enhancement of the GNU strings tool with
//! additional functionalities: stringsext recognizes Cyrillic, CJKV characters
//! and other scripts in all supported multi-byte-encodings, while GNU strings
//! fails in finding any of these scripts in UTF-16 and many other encodings.\

//! The role of the main-module is to launch the processing of the input stream in
//! batches with threads. It also receives, merges, sorts and prints the results.

//!  # Operating principle

//!  1. The iterator `input::Slicer` concatenates the input-files and cuts
//!  the input stream into slices called `main::slice`.
//!
//!  2. In `main::run()` these slices are feed in parallel to threads, where each has
//!  its own `Mission` configuration.
//!
//!  3. Each thread runs a search in `main::slice` == `scanner::input_buffer`. The
//!  search is performed by `scanner::scan()`, which cuts the `scanner::input_buffer`
//!  into smaller chunks of size 2*`output_line_char_nb_max` bytes hereafter called
//! `input_window`.
//!
//!  4. The `Decoder` runs through the `input_window`, searches for valid strings and
//!  decodes them into UTF-8-chunks.
//!
//!  5. Each UTF-8-chunk is then fed into the filter `helper::SplitStr` to be
//!  analyzed if parts of it satisfy certain filter conditions.
//!
//!  6. Doing so, the `helper::SplitStr` cuts the UTF-8-chunk into even smaller
//!  `SplitStr`-chunks not longer than `output_line_char_nb_max` and sends them back to the
//!  `scanner::scan()` loop.
//!
//!  7. There the `SplitStr`-chunk is packed into a `finding::Finding` object and
//!  then successively added to a `finding::FindingCollection`.
//!
//!  8. After finishing its run through the `input_window` the search continues with
//!  the next `input_window. Goto 5.
//!
//!  9. When all `input_window` s are processed, `scanner::scan()` returns the
//!  `finding::FindingCollection` to `main::run()` and exits.
//!
//!  10. `main::run()` waits for all threads to return their
//!  `finding::FindingCollection` s. Then, all `Findings` s are merged,
//!  sorted and finally print out by `finding::print()`.
//!
//!  11. While the print still running, the next `main::slice` ==
//!  `scanner::input_buffer` is sent to all threads for the next search.
//!  Goto 3.
//!
//!  12. `main::run()` exits when all `main::slice` s are processed.

extern crate encoding_rs;

mod finding;
mod help;
mod helper;
mod input;
mod mission;
mod options;
mod scanner;

use crate::finding::FindingCollection;
use crate::finding::OUTPUT_LINE_METADATA_LEN;
use crate::help::help;
use crate::input::Slicer;
use crate::mission::MISSIONS;
use crate::options::ARGS;
use crate::scanner::scan;
use crate::scanner::ScannerStates;
use itertools::kmerge;
use scoped_threadpool::Pool;
use std::fs::File;
use std::io;
use std::io::LineWriter;
use std::io::Write;
use std::path::Path;
use std::process;
use std::str;
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;

/// Use the version-number defined in `../Cargo.toml`.
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
/// (c) Jens Getreu
const AUTHOR: &str = "(c) Jens Getreu, 2016-2020";

/// Process the input stream in batches with threads. Then receive, merge, sort and
/// print the results.

fn run() -> Result<(), anyhow::Error> {
    let merger: JoinHandle<_>;
    // Scope for threads
    {
        let n_threads = MISSIONS.len();
        let (tx, rx) = mpsc::sync_channel(n_threads);

        //
        // Receiver thread:

        // Receive `FindingCollection`s from scanner threads.
        merger = thread::spawn(move || {
            // Set up output channel.
            let mut output = match ARGS.output {
                Some(ref fname) => {
                    let f = File::create(&Path::new(fname))?;
                    // There is at least one `Mission` in `MISSIONS`.
                    let output_line_len =
                        2 * MISSIONS[0].output_line_char_nb_max + OUTPUT_LINE_METADATA_LEN;
                    let f = LineWriter::with_capacity(output_line_len, f);
                    Box::new(f) as Box<dyn Write>
                }
                None => Box::new(io::stdout()) as Box<dyn Write>,
            };
            output.write_all("\u{feff}".as_bytes())?;

            'batch_receiver: loop {
                // collect
                let mut results: Vec<FindingCollection> = Vec::with_capacity(n_threads);
                for _ in 0..n_threads {
                    results.push(match rx.recv() {
                        Ok(fc) => fc,
                        Err(_) => break 'batch_receiver,
                    });
                }
                // merge
                for finding in kmerge(&results) {
                    finding.print(&mut output)?;
                }
            }
            //println!("Merger terminated.");
            output.write_all(&[b'\n'])?;
            output.flush()?;
            Ok(())
        });

        //
        // Sender threads:

        // Setting up the data slice producer.
        let input = Slicer::new();

        // We set up the processor.
        let mut sss = ScannerStates::new(&MISSIONS);
        let mut pool = Pool::new(MISSIONS.len() as u32);

        for (slice, input_file_id, is_last_input_buffer) in input {
            pool.scoped(|scope| {
                for mut ss in sss.v.iter_mut() {
                    let tx = tx.clone();
                    scope.execute(move || {
                        let fc = scan(&mut ss, input_file_id, slice, is_last_input_buffer);
                        // Send the result to the receiver thread.
                        tx.send(fc).expect(
                            "Error: Can not sent result through output channel. \
                             Write permissions? Is there enough space? ",
                        );
                    });
                }
            });
        }
    } // `tx` drops here, which breaks the `batch_receiver`-loop.

    // If everything goes well, we get `()` here.
    merger.join().unwrap()

    // All threads terminated.
}

/// Application entry point.
fn main() {
    help();

    if let Err(e) = run() {
        eprintln!("Error: `{:?}`.", e);
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use crate::finding::FindingCollection;
    use crate::finding::Precision;
    use crate::mission::Missions;
    use crate::options::{Args, Radix};
    use crate::scanner::scan;
    use crate::scanner::ScannerState;
    use itertools::Itertools;
    use lazy_static::lazy_static;
    use std::path::PathBuf;

    lazy_static! {
        pub static ref ARGS: Args = Args {
            inputs: vec![PathBuf::from("myfile.txt")],
            debug_option: false,
            encoding: vec!["ascii".to_string(), "utf-8".to_string()],
            list_encodings: false,
            version: false,
            chars_min: Some("5".to_string()),
            same_unicode_block: true,
            grep_char: None,
            radix: Some(Radix::X),
            output: None,
            output_line_len: Some("30".to_string()),
            no_metadata: false,
            counter_offset: Some("5000".to_string()),
            ascii_filter: None,
            unicode_block_filter: None,
        };
    }

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
        .unwrap();
    }

    /// Tests the concurrent scanning with 2 threads, while one thread merges and prints.
    #[test]
    fn test_merger() {
        let inp = "abcdefgÜhijklmn€opÜqrstuvwÜxyz".as_bytes();

        let missions = &MISSIONS;
        //println!("{:#?}", *MISSIONS);

        let mut ss0 = ScannerState::new(&missions.v[0]);
        let mut ss1 = ScannerState::new(&missions.v[1]);

        let mut resv: Vec<FindingCollection> = Vec::new();
        resv.push(scan(&mut ss0, Some(0), inp, true));
        resv.push(scan(&mut ss1, Some(0), inp, true));

        //println!("{:#?}", resv);

        assert_eq!(resv.len(), 2);
        assert_eq!(resv[0].v.len(), 3);
        assert_eq!(resv[0].v[0].s, "abcdefg");
        assert_eq!(resv[0].v[1].s, "hijklmn");
        assert_eq!(resv[0].v[2].s, "qrstuvw");
        assert_eq!(resv[1].v.len(), 2);
        assert_eq!(resv[1].v[0].s, "abcdefgÜhijklmn");
        assert_eq!(resv[1].v[1].s, "opÜqrstuvwÜxyz");

        // Merge the results.

        let mut iter = resv.iter().kmerge();
        // for res in iter {
        //     println!("Result {:#?}", res);
        // };

        // After merging and sorting the order is deterministic.
        // See implementation of `PartialOrd` for `Finding` for more
        // details.

        let f = iter.next().unwrap();
        assert_eq!(f.s, "abcdefg");
        assert_eq!(f.position, 5000);
        assert_eq!(f.position_precision, Precision::Exact);
        assert_eq!(f.mission.mission_id, 0);

        let f = iter.next().unwrap();
        assert_eq!(f.s, "hijklmn");
        assert_eq!(f.position, 5000);
        assert_eq!(f.position_precision, Precision::After);
        assert_eq!(f.mission.mission_id, 0);

        let f = iter.next().unwrap();
        assert_eq!(f.s, "qrstuvw");
        assert_eq!(f.position, 5000);
        assert_eq!(f.position_precision, Precision::After);
        assert_eq!(f.mission.mission_id, 0);

        let f = iter.next().unwrap();
        assert_eq!(f.s, "abcdefgÜhijklmn");
        assert_eq!(f.position, 5000);
        assert_eq!(f.position_precision, Precision::Exact);
        assert_eq!(f.mission.mission_id, 1);

        let f = iter.next().unwrap();
        assert_eq!(f.s, "opÜqrstuvwÜxyz");
        assert_eq!(f.position, 5000);
        assert_eq!(f.position_precision, Precision::After);
        assert_eq!(f.mission.mission_id, 1);

        let f = iter.next();
        assert_eq!(f, None);
    }
}
