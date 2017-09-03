//! This `main` module uses the `options` module to read its command-line-arguments.
//! It defines code for spawning the _merger-thread_ who
//! collects the results produced by the worker threads.
//! The processing of the input-data is initiated by the `input`-module that itself uses
//! the `scanner` module in which the worker-threads are spawned.
mod mission;

mod input;
use input::{process_input};

extern crate rustc_serialize;
extern crate docopt;
#[macro_use]
extern crate lazy_static;

mod options;
use options::ARGS;

mod scanner;
use scanner::ScannerPool;

mod finding;

mod helper;

mod codec {
    pub mod ascii;
}

use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::str;
use std::process;
use std::thread::JoinHandle;
use std::io;
use mission::MISSIONS;

extern crate itertools;
use std::sync::mpsc;

extern crate scoped_threadpool;
use std::thread;

extern crate encoding;
use encoding::all;
use itertools::kmerge;
use itertools::Itertools;

// Version is defined in ../Cargo.toml
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
const AUTHOR: &'static str = "(c) Jens Getreu, 2016";



/// This function spawns and defines the behaviour of the _merger-thread_ who
/// collects and prints the results produced by the worker threads.
fn main2() -> Result<(), Box<std::io::Error>> {

    if ARGS.flag_list_encodings  {
        let list = all::encodings().iter().filter_map(|&e|e.whatwg_name()).sorted();
        // Available encodings
        for e in list {
            println!("{}",e);
        }
        return Ok(());
    }

    if ARGS.flag_version {
        println!("Version {}, {}", VERSION.unwrap_or("unknown"), AUTHOR );
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
                            let f = try!(File::create(&Path::new(fname.as_str())));
                            Box::new(f) as Box<Write>
                        },
               None  => Box::new(io::stdout()) as Box<Write>,
            };
            try!(output.write_all("\u{feff}".as_bytes()));

            'outer: loop {
                // collect
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
                // merge
                for finding in kmerge(&results) {
                    try!(finding.print(&mut output));
                };
            };
            //println!("Merger terminated.");
            Ok(())
        });

        // Default for <file> is stdin.
        if (ARGS.arg_FILE.len() == 0) ||
           ( (ARGS.arg_FILE.len() == 1) && ARGS.arg_FILE[0] == "-") {
            try!(process_input(None, &mut sc));
        } else {
            for ref filename in ARGS.arg_FILE.iter() {
                if let Err(e) = process_input(Some(&filename), &mut sc) {
                    writeln!(&mut std::io::stderr(),
                             "Warning: `{}` while scanning file `{}`.",e,filename).unwrap();
                };
            };
        };
    } // `tx` drops here, which "break"s the merger-loop.
    merger.join().unwrap()

    //println!("All threads terminated.");
}

fn main() {
    if let Err(e) = main2() {
        writeln!(&mut std::io::stderr(), "Error: `{}`.",e).unwrap();
        process::exit(1);
    }
}
