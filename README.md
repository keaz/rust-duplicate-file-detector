# Duplicate file checker 

[![crates.io](https://img.shields.io/crates/v/duplicate-checker.svg)](https://crates.io/crates/duplicate-checker)
[![Crates.io](https://img.shields.io/crates/l/duplicate-checker)](https://crates.io/crates/duplicate-checker)
[![Crates.io](https://img.shields.io/crates/d/duplicate-checker)](https://crates.io/crates/duplicate-checker)
[![docs.rs](https://img.shields.io/docsrs/duplicate-checker/1.4.0)](https://crates.io/crates/duplicate-checker)

## About
This is a asynchronous duplicate file detector written in RUST. Has the capability to adjust the duplicate file detection using file name score and sha or file size.

## How to
Install using cargo `cargo install duplicate-checker`  
Run `duplicate-checker -r={path-to-check-duplicate} -s=90`  
CMD arguments  
`-r` Path to check duplicate  
`-s` Search score

