//  LIB.rs
//    by Lut99
//
//  Description:
//!   A Rust implementation of the FastCGI process control protocol.
//

#[cfg(feature = "connection")]
use std::io::{Read, Write};

// Declare the modules
#[cfg(feature = "connection")]
pub mod conn;
pub mod spec;


/***** HELPERS *****/
/// Aliases [`Read`] and [`Write`].
pub trait ReadWrite: Read + Write {}
impl<T: ?Sized + Read + Write> ReadWrite for T {}





/***** LIBRARY *****/
/// The main type that wraps a FastCGI connection to send it instructions.
#[cfg(feature = "connection")]
pub struct FastCGI {
    /// The socket address we're connecting to.
    addr: String,
    /// The stream that we use as connection.
    conn: Box<dyn ReadWrite>,
}
