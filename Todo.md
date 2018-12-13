---
title:    'Todo'
subtitle: ''
author:   Jens Getreu
date:     2018-09-24
revision: 1.0
fileext:  md
---


*  Optimize code by reducing copying: use cow where possible.

*  Migrate to [encoding_rs - Rust](https://docs.rs/encoding_rs/0.8.0/encoding_rs/)

   Concerned functions are: 
   *   file: `scanner.rs`: `scan_window()`
   *   file: `finding.rs`: `writer_hint()`, `write_char()`, `write_str()`


*  Preformance in: `finding.rs`: `macro_rules! enc_str`: 
  
   avoid `format!`, use something like [numtoa - Cargo: packages for
   Rust](https://crates.io/crates/numtoa/0.0.7) instead
