//! This module abstracts the data-input channels i.e. file and stdin.
use crate::scanner::ScannerPool;

use std;
use std::path::Path;
use std::io::prelude::*;
use std::io::stdin;
use std::fs::File;

use memmap;
use self::memmap::MmapOptions;



/// `WIN_LEN` is the length of the memory chunk in which strings are searched in
/// parallel.
///
/// # Memory map:
///
/// ```text

/// |<WIN_STEP1 -------------->|<WIN_STEP2 --------------->|<WIN_STEP3 -----
///                            |<WIN_OVERLAP1>|            |<WIN_OVERLAP2>|
/// |<WIN_LEN1  ---------------------------- >|
///                            |<WIN_LEN2 ------------------------------->|
/// ```
///
/// As shown above `WIN_LEN` defines an overlapping window that advances `WIN_STEP`
/// Bytes each iteration.
///
/// `WIN_LEN = WIN_STEP + WIN_OVERLAP` is the size of the memory chunk that
/// is processed during one iteration. A string is only found when it starts
/// within the `WIN_STEP` interval.
/// The remaining Bytes can reach into `WIN_OVERLAP` or even beyond `WIN_LEN`.
/// In the latter case the string is split.
pub const WIN_LEN: usize = WIN_STEP + WIN_OVERLAP;



/// `WIN_OVERLAP` is the overlapping fragment of the window. The
/// overlapping fragment is used to read some Bytes ahead when the string is not
/// finished. `WIN_OVERLAP` is subject
/// to certain conditions: For example the overlapping part must be smaller
/// than `WIN_STEP`. Furthermore, the size of
/// `FINISH_STR_BUF = WIN_OVERLAP - UTF8_LEN_MAX` determines the number of
/// Bytes at the beginning of a string that are guaranteed not to be spit.
///
/// This size matters because the scanner counts the length of its findings.
/// If a string is too short (< `ARG.flag_bytes`) it will be skiped.
/// To avoid that a string with the required size gets too short because
/// of splitting, we claim the following condition:
///
/// ```text
///  1 <=  FLAG_BYTES_MAX <=  FINISH_STR_BUF
/// ```
/// In practice we chose for `FINISH_STR_BUF` a bigger size than the minimum to avoid
/// splitting of strings as much as possible.
/// Please refer to the test function `test_constants()` for more details about
/// constraints on constants. The test checks all the necessary conditions on
/// constants to guarantee the correct functioning of the program.
pub const FINISH_STR_BUF: usize = 0x1800;

///
/// The scanner tries to read strings in `WIN_LEN` as far as it can.
/// The first invalid Byte indicates the end of a string and the scanner
/// holds for a moment to store its finding. Then it starts searching further
/// until the next string is found.
/// Once `WIN_OVERLAP` is entered the search ends and the `start` variable
/// is updated so that it now points to `restart-at-invalid` as shown in the
/// next figure. This way the next iteration can continue at the same place
/// the previous had stopped.
///
/// The next iteration can identify this situation because the `start` pointer
/// points into the previous `FINISH_STR_BUF` interval.
///
/// # Memory map:
///
/// ```text

/// |<WIN_STEP1 ------------------------------->|<FINISH_STR_BUF>|<UTF8_LEN_MAX>|
///                                             |<WIN_OVERLAP1>---------------->|
/// |<WIN_LEN1 ---------------------------------------------------------------->|
///
///       <==string==><invalid Bytes><=====string===><invalid Bytes>
///                                                  ^
///                                                  |
///                                       `restart-at-invalid`
/// ```
///
/// A special treatment is required when a sting extends slightly beyond
/// `WIN_LEN`. In this case the scanner most likely runs into an incomplete
/// multi-Byte character just before the end of `WIN_LEN`.
/// The cut surface _restart-at-cut_ is then somewhere in the `UTF8_LEN_MAX`
/// interval as  the following figure shows.
///
/// The remaining part will be printed later during the next iteration.  But how
/// does the following iteration know if a string had been cut by the previous
/// iteration?  In the next interval the scanner first checks if the previous scan
/// ended in the `UTF8_LEN_MAX` interval. If yes, we know the string has been cut
/// and we the remaining Bytes at the beginning of the new interval regardless of
/// their size.

///
///
/// # Memory map:
///
/// ```text

/// |<WIN_STEP1 ------------------------------->|<FINISH_STR_BUF>|<UTF8_LEN_MAX>|
///                                             |<WIN_OVERLAP1>---------------->|
/// |<WIN_LEN1 ---------------------------------------------------------------->|
///
///       <==string==><invalid Bytes><=====string===================|===========....>
///                                                                 ^ incomplete
///                                                                 | valid Multi-
///                                                                 | Byte-Char
///                                                                 |
///                                                          `restart-at-cut`
/// ```
///
///
/// To satisfy all the above constraints `WIN_OVERLAP` must satisfy two conditions
/// concurrently:
///
/// # Constraint:
///
///
/// ```text
///                                    WIN_OVERLAP <= WIN_STEP
/// FINISH_STR_BUF  + UTF8_LEN_MAX  =  WIN_OVERLAP
/// ```
///
pub const WIN_OVERLAP: usize = FINISH_STR_BUF + (UTF8_LEN_MAX as usize);


/// As Files are accessed through 4KiB memory pages we choose `WIN_STEP` to be a multiple of
/// 4096 Bytes.
///
pub const WIN_STEP: usize = 0x2000; // = 2*4096


/// The `from_stdin()` function implements its own reader buffer `BUF_LEN` to allow
/// stepping with overlapping windows.
/// The algorithm requires that `BUF_LEN` is greater or equal than `WIN_LEN`
/// (the greater the better the performance).
///
/// # Constraint:
/// ```text
///      WIN_LEN <= BUF_LEN
/// ```
///
/// Every time `BUF_LEN` is full, the last `WIN_OVERLAP` part must be copied from the end
/// to the beginning of `BUF_LEN`. As copying is an expensive operation we choose:
///
/// # Constraint:
/// ```text
///      BUF_LEN = 4 * WIN_STEP + WIN_OVERLAP
/// ```
///
/// to reduce the copying to every 4th iteration.
///
pub const BUF_LEN: usize = 4 * WIN_STEP + WIN_OVERLAP;


/// In Unicode the maximum number of Bytes a multi-Byte-character can occupy
/// in memory is 6 Bytes.
pub const UTF8_LEN_MAX: u8 = 6;


/// Read the appropriate input chunk by chunk and launch the scanners on each
/// Chunk.
/// If `file_path_str` == None read from `stdin`, otherwise
/// read from file.
pub fn process_input(filename: Option<&'static str>, mut sc: &mut ScannerPool<'_>)
                                            -> Result<(), Box<std::io::Error>> {
    match filename {
        Some(filename) => from_file(&mut sc, &filename),
        None           => from_stdin(&mut sc)
    }
}


/// Streams a file by cutting the input into overlapping chunks and feeds the `ScannerPool`.
/// After each iteration the `byte_counter` is updated.
/// In order to avoid additional copying the trait `memmap` is used to access
/// the file contents. See:
/// https://en.wikipedia.org/wiki/Memory-mapped_file
pub fn from_file(sc: &mut ScannerPool<'_>, filename: &'static str) -> Result<(), Box<std::io::Error>> {

    let file = File::open(&Path::new(filename))?;
    let len = file.metadata()?.len() as usize;
    let mut byte_counter: usize = 0;
    let mmap = unsafe { self::MmapOptions::new().map(&file)? };
    while byte_counter + WIN_LEN <= len {
        let chunk = &mmap[byte_counter..byte_counter+WIN_LEN];
        sc.launch_scanner(Some(&filename), &byte_counter, &chunk);
        byte_counter += WIN_STEP;
    }
    // The last is shorter than WIN_LEN bytes, but can be longer than WIN_STEP
    if byte_counter < len {
        let chunk = &mmap[byte_counter..len];
        sc.launch_scanner(Some(&filename), &byte_counter, &chunk);
        byte_counter += WIN_STEP;
    }

    // Now there can be still some bytes left (maximum WIN_OVERLAP bytes)
    if byte_counter < len {
        let chunk = &mmap[byte_counter..len];
        sc.launch_scanner(Some(&filename), &byte_counter, &chunk);
    }
    Ok(())
}


/// Streams the input pipe by cutting it into overlapping chunks and feeds the `ScannerPool`.
/// This functions implements is own rotating input buffer.
/// After each iteration the `byte_counter` is updated.
fn from_stdin(sc: &mut ScannerPool<'_>) -> Result<(), Box<std::io::Error>> {
    let mut byte_counter: usize = 0;
    let stdin = stdin();
    let mut stdin = stdin.lock();
    let mut buf = [0; BUF_LEN];
    let mut data_start: usize = 0;
    let mut data_end: usize = 0;
    let mut done = false;
    while !done {
        // Rotate the buffer if there isn't enough space
        if data_start + WIN_LEN > BUF_LEN {
            let (a, b) = buf.split_at_mut(data_start);
            let len = data_end - data_start;
            a[..len].copy_from_slice(&b[..len]);
            data_start = 0;
            data_end = len;
        }
        // Read from stdin
        while data_end < data_start + WIN_LEN {

            let bytes = stdin.read(&mut buf[data_end..])?;
            if bytes == 0 {
                done = true;
                break;
            }
            else {data_end += bytes; }
        }
        // Handle data.
        while data_start + WIN_LEN <= data_end {
            sc.launch_scanner(None, &byte_counter, &buf[data_start..data_start + WIN_LEN]);
            data_start += WIN_STEP;
            byte_counter += WIN_STEP;
        }
    }
    // The last is shorter than WIN_LEN bytes, but can be longer than WIN_STEP
    if data_start < data_end {
        sc.launch_scanner(None, &byte_counter, &buf[data_start..data_end]);
        data_start += WIN_STEP;
        byte_counter += WIN_STEP;
    }

    // Now there can be still some bytes left (maximum WIN_OVERLAP bytes)
    if data_start < data_end {
        sc.launch_scanner(None, &byte_counter, &buf[data_start..data_end]);
    }
    Ok(())
}






#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{Args, Radix, ControlChars};
    use tempdir;

    use std::fs::File;
    use std::io::Write;
    use self::tempdir::TempDir;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use crate::mission::Missions;
    use crate::finding::FindingCollection;
    use crate::finding::Finding;
    use crate::scanner::ScannerPool;
    use std::thread;
    use crate::options::FLAG_BYTES_MAX;


    lazy_static! {
        pub static ref ARGS:Args = Args {
           arg_FILE: vec!["myfile.txt".to_string()],
           flag_control_chars:  ControlChars::R,
           flag_encoding: vec!["ascii".to_string(), "utf8".to_string()],
           flag_list_encodings: false,
           flag_version: false,
           flag_bytes:  Some(5),
           flag_split_bytes:  Some(2),
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

    /// Test the constraints for constants. The test checks all the necessary conditions on
    /// constants to guarantee a correct functionality of the program.
    #[test]
    fn test_constants(){
        assert!(WIN_STEP % 0x1000 == 0);
        assert!(WIN_STEP + WIN_OVERLAP <= WIN_LEN );
        assert!(FLAG_BYTES_MAX < FINISH_STR_BUF );
        assert!(WIN_OVERLAP <= WIN_STEP);
        assert!(FINISH_STR_BUF  + UTF8_LEN_MAX as usize <=  WIN_OVERLAP);
        assert!(WIN_LEN <= BUF_LEN);
    }


    lazy_static! {
       pub static ref PATH: PathBuf = TempDir::new("test_from_file")
                                      .expect("Can not create tempdir.")
                                      .path()
                                      .to_path_buf();
    }



    /// Test the reading and processing of file data.
    #[test]
    fn test_from_file(){
        {
            let mut f = File::create(&PATH.as_path()).unwrap();
            let inp = "hallo1234üduüso567890".as_bytes();
            f.write_all(&inp[..]).unwrap();
        }

        {
            let (tx, rx) = mpsc::sync_channel(0);
            let mut sc = ScannerPool::new(&MISSIONS,&tx);


            let _ = thread::spawn(move || {

                let expected1 = FindingCollection{ v: vec![
                    Finding{ filename: None, ptr: 0,
                             mission: &MISSIONS.v[0] ,
                             s: "hallo1234".to_string() },
                    Finding{ filename: None, ptr: 15,
                             mission: &MISSIONS.v[0] ,
                             s: "so567890\u{2691}".to_string() },
                    ], completes_last_str: false, last_str_is_incomplete: true};

                let expected2 = FindingCollection{ v: vec![
                    Finding{ filename: None, ptr: 0,
                             mission: &MISSIONS.v[1],
                             s: "hallo1234üduüso567890\u{2691}".to_string() },
                    ], completes_last_str: false, last_str_is_incomplete: true};

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
            from_file(&mut sc, PATH.as_path().to_str().unwrap()).unwrap();

        }
    }
}
