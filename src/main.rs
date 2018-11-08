//! This `main` module uses the `options` module to read its command-line-arguments.
//! It defines code for spawning the _merger-thread_ who
//! collects the results produced by the worker threads.
//! The processing of the input-data is initiated by the `input`-module that itself uses
//! the `scanner` module in which the worker-threads are spawned.

use serde_derive::Deserialize;

mod mission;

mod input;
use crate::input::process_input;

mod options;
use crate::options::ARGS;

mod scanner;
use crate::scanner::ScannerPool;

mod finding;

mod helper;

mod codec {
    pub mod ascii;
}

use crate::mission::MISSIONS;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::process;
use std::str;
use std::thread::JoinHandle;

use std::sync::mpsc;

use std::thread;

use encoding::all;
use itertools::kmerge;
use itertools::Itertools;

// Version is defined in ../Cargo.toml
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
const AUTHOR: &'static str = "(c) Jens Getreu, 2016-2018";

/// This function spawns and defines the behaviour of the _merger-thread_ who
/// collects and prints the results produced by the worker threads.
fn main2() -> Result<(), Box<std::io::Error>> {
    if ARGS.flag_list_encodings {
        let list = all::encodings()
            .iter()
            .filter_map(|&e| e.whatwg_name())
            .sorted();
        // Available encodings
        for e in list {
            println!("{}", e);
        }
        return Ok(());
    }

    if ARGS.flag_version {
        println!("Version {}, {}", VERSION.unwrap_or("unknown"), AUTHOR);
        return Ok(());
    }

    let merger: JoinHandle<Result<(), Box<std::io::Error>>>;
    // Scope for threads
    {
        let n_threads = MISSIONS.len();
        let (tx, rx) = mpsc::sync_channel(n_threads);
        let mut sc = ScannerPool::new(&MISSIONS, &tx);

        // Receive `FindingCollection`s from scanner threads.
        merger = thread::spawn(move || {
            let mut output = match ARGS.flag_output {
                Some(ref fname) => {
                    let f = File::create(&Path::new(fname.as_str()))?;
                    Box::new(f) as Box<dyn Write>
                }
                None => Box::new(io::stdout()) as Box<dyn Write>,
            };
            output.write_all("\u{feff}".as_bytes())?;

            'outer: loop {
                // collect
                let mut results = Vec::with_capacity(n_threads);
                for _ in 0..n_threads {
                    results.push(match rx.recv() {
                        Ok(fc) => {
                            //fc.print(&mut output);
                            fc.v
                        }
                        Err(_) => break 'outer,
                    });
                }
                // merge
                for finding in kmerge(&results) {
                    finding.print(&mut output)?;
                }
            }
            //println!("Merger terminated.");
            Ok(())
        });

        // Default for <file> is stdin.
        if (ARGS.arg_FILE.len() == 0) || ((ARGS.arg_FILE.len() == 1) && ARGS.arg_FILE[0] == "-") {
            process_input(None, &mut sc)?;
        } else {
            for ref filename in ARGS.arg_FILE.iter() {
                if let Err(e) = process_input(Some(&filename), &mut sc) {
                    writeln!(
                        &mut std::io::stderr(),
                        "Warning: `{}` while scanning file `{}`.",
                        e,
                        filename
                    )
                    .unwrap();
                };
            }
        };
    } // `tx` drops here, which "break"s the merger-loop.
    merger.join().unwrap()

    //println!("All threads terminated.");
}

fn main() {
    if let Err(e) = main2() {
        writeln!(&mut std::io::stderr(), "Error: `{}`.", e).unwrap();
        process::exit(1);
    }
}
