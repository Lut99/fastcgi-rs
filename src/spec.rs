//  SPEC.rs
//    by Lut99
//
//  Created:
//    16 Nov 2024, 16:07:22
//  Last edited:
//    30 Nov 2024, 13:46:36
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the message structures used in the protocol.
//

use std::error::Error;

use stackvec::StackVec;
use thiserror::Error;


/***** ERRORS *****/
/// Defines errors occurring when [parsing](Version::from_bytes()) [`Version`]s.
#[derive(Debug, Error)]
pub enum VersionParseError {
    #[error("Empty buffer given")]
    Empty,
    #[error("Illegal version number {raw}")]
    IllegalVersion { raw: u8 },
}

/// Defines errors occurring when [parsing](Record::from_bytes()) [`Record`]s.
#[derive(Debug, Error)]
pub enum RecordParseError {}





/***** AUXILLARY *****/
/// Allows one of this lib's structs to be converted into bytes and vice versa.
pub trait Message: Sized {
    /// Defines the error that occurs when we failed to parse this message
    /// [from bytes](Message::from_bytes()).
    type Error: Error;


    /// Serializes this message as bytes in the given buffer.
    ///
    /// # Arguments
    /// - `buf`: The buffer to serialize in. It will automatically be resized.
    ///
    /// # Returns
    /// The number of bytes written.
    fn to_bytes(&self, buf: &mut Vec<u8>) -> usize;

    /// Attempts to parse this message from the given bytes.
    ///
    /// # Arguments
    /// - `buf`: The bytes to parse this message from.
    ///
    /// # Returns
    /// A pair of parsed Self and the number of bytes parsed from the given `buf`.
    ///
    /// # Errors
    /// This function may fail if the given `buf`fer contained invalid bytes for this message.
    fn from_bytes(buf: impl AsRef<[u8]>) -> Result<(Self, usize), Self::Error>;
}





/***** DATA TYPES *****/
/// Defines known protocol versions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Version {
    /// The first version of the protocol.
    V1 = 1,
}
impl Message for Version {
    type Error = VersionParseError;

    #[inline]
    fn to_bytes(&self, buf: &mut Vec<u8>) -> usize {
        match self {
            Self::V1 => {
                buf.push(1);
                1
            },
        }
    }

    #[inline]
    fn from_bytes(buf: impl AsRef<[u8]>) -> Result<(Self, usize), Self::Error> {
        let buf: &[u8] = buf.as_ref();
        match buf.first() {
            Some(1) => Ok((Self::V1, 1)),
            Some(raw) => Err(VersionParseError::IllegalVersion { raw: *raw }),
            None => Err(VersionParseError::Empty),
        }
    }
}

/// Defines possible record types.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RecordType {}





/***** MESSAGES *****/
/// Defines a FastCGI _record_, which is used to carry arbitrary data around.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct Record {
    /// The protocol version used.
    version: Version,
    /// The type of this record.
    ty:      RecordType,
    /// The ID of this record.
    id:      u16,
    /// The content of this record.
    content: StackVec<{ u16::MAX as usize }, u8>,
}
impl Message for Record {
    type Error = RecordParseError;

    fn to_bytes(&self, buf: &mut Vec<u8>) -> usize {}

    fn from_bytes(buf: impl AsRef<[u8]>) -> Result<(Self, usize), Self::Error> {}
}
