//! Help the user with command-line-arguments.

use crate::mission::ASCII_FILTER_ALIASSE;
use crate::mission::UNICODE_BLOCK_FILTER_ALIASSE;
use crate::mission::{Missions, MISSIONS};
use crate::options::ARGS;
use crate::options::ASCII_ENC_LABEL;
use crate::AUTHOR;
use crate::VERSION;
use std::process;
use std::str;

/// Function called at the beginning of `stringsext`. When help is printed to the
/// user, the program exits.

pub fn help() {
    if ARGS.version {
        println!("Version {}, {}", VERSION.unwrap_or("unknown"), AUTHOR);
        process::exit(0);
    };

    if ARGS.debug_option {
        println!("GIVEN COMMANDLINE-ARGUMENTS\n");
        println!("Input files\n-----------");
        for (n, name) in ARGS.inputs.iter().enumerate() {
            println!("{} = {:?}", char::from((n + 65) as u8), name);
        }

        println!("\nEncoding and filter definitions\n-------------------------------");
        for (n, name) in ARGS.encoding.iter().enumerate() {
            println!("{} = {}", char::from((n + 97) as u8), name);
        }

        println!("\n\nPARSED COMMANDLINE-ARGUMENTS\n");

        let ms: &'static Missions = &MISSIONS;
        for (i, m) in ms.v.iter().enumerate() {
            println!(
                "Scanner ({})\n-----------\n{:#?}\n",
                char::from((i + 97) as u8),
                m
            );
        }
        process::exit(0);
    };

    if ARGS.list_encodings {
        // Is there a way to programmatically query a list from `Encoding`?
        // This list is taken from the `Encoding` source file (2019-12-11)
        // and may  not be up to date.
        println!("LIST OF AVAILABLE ENCODINGS AND PREDEFINED FILTERS\n");
        println!("Format: --encoding=[ENC_NAME],[MIN],[AF,UBF],[GREP]\n\n");
        println!("ENC_NAME (Encoding)=");
        let list: [&'static str; 41] = [
            ASCII_ENC_LABEL,
            "Big5",
            "EUC-JP",
            "EUC-KR",
            "GBK",
            "IBM866",
            "ISO-2022-JP",
            "ISO-8859-10",
            "ISO-8859-13",
            "ISO-8859-14",
            "ISO-8859-15",
            "ISO-8859-16",
            "ISO-8859-2",
            "ISO-8859-3",
            "ISO-8859-4",
            "ISO-8859-5",
            "ISO-8859-6",
            "ISO-8859-7",
            "ISO-8859-8",
            "ISO-8859-8-I",
            "KOI8-R",
            "KOI8-U",
            "Shift_JIS",
            "UTF-16BE",
            "UTF-16LE",
            "UTF-8",
            "gb18030",
            "macintosh",
            "replacement",
            "windows-1250",
            "windows-1251",
            "windows-1252",
            "windows-1253",
            "windows-1254",
            "windows-1255",
            "windows-1256",
            "windows-1257",
            "windows-1258",
            "windows-874",
            "x-mac-cyrillic",
            "x-user-defined",
        ];

        // Available encodings
        for e in list.iter() {
            println!("\t{}", e);
        }
        println!("\tWarning: this list may be outdated.");
        println!(
            "\tPlease consult the library `encoding_rs` documentation \
             for more available encodings.\n\n"
        );

        println!("MIN = <number>");
        println!("\tOnly strings with at least <number> characters are printed.\n\n");

        println!("AF (ASCII-Filter) = <filter name> or <hexadecimal number>");
        for (e, b, c) in &ASCII_FILTER_ALIASSE {
            let b = format!("{:#x}", b);
            println!(
                "\t{} = {:>35} ({})",
                str::from_utf8(e).unwrap(),
                b,
                str::from_utf8(c).unwrap().trim()
            );
        }
        println!(
            "\tUse predefined filter names above or your own filter starting with `0x...`.\n\n"
        );

        println!("UBF (Unicode-Block-Filter) = <filter name> or <hexadecimal number>");
        for (e, b, c) in &UNICODE_BLOCK_FILTER_ALIASSE {
            let b = format!("{:#x}", b);
            println!(
                "\t{} = {:>18} ({})",
                str::from_utf8(e).unwrap(),
                b,
                str::from_utf8(c).unwrap().trim()
            );
        }
        println!(
            "\tUse predefined filter names above or your own filter starting with `0x...`.\n\n"
        );

        println!("GREP = <ASCII code>");
        println!("\tPrint only lines having at least one character with <ASCII-code>.");
        println!("\tUseful values are `47` (/) or `92` (\\) for path search.");
        println!("\t<ASCII code> can be decimal or hexadecimal and must be < 128.");

        process::exit(0);
    }
}
