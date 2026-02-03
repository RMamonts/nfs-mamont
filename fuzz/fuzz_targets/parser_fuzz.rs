#![no_main]

use libfuzzer_sys::fuzz_target;
use nfs_mamont::parser::Arguments;

fuzz_target!(|_data: Vec<Arguments>| {});
