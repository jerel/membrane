mod advanced;
mod simple;

// used to test interaction with a C library's threading, feature flagged
// so it doesn't run by default on machines that aren't set up for C
#[cfg(feature = "c-example")]
pub mod c_example;
