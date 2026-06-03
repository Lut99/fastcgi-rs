//  DATA TYPES.rs
//    by Lut99
//
//  Description:
//!   Specs some more complex data types used by the FastCGI-spec.
//

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};


/***** ERRORS *****/
#[derive(Debug, Error, Eq, PartialEq)]
pub enum RecordHeaderError {
    #[error("Not enough bytes left for a record header (got {got}, expected 8)")]
    NotEnoughBytes { got: usize },
    #[error("Unknown version byte {0:?}")]
    UnknownVersion(u8),
    #[error("Unknown record type byte {0:?}")]
    UnknownTy(u8),
}





/***** FUNCTIONS *****/
/// Serializes a [`u32`] in FCGI's compact style.
pub async fn write_u32_compact<W: AsyncWrite + Unpin>(value: u32, mut output: W) -> Result<(), std::io::Error> {
    if value <= 127 {
        // Simple-length case; it's a 8-bit, <= 127 number (MSB is 0)

        //     unsigned char numB0;  /* numB0  >> 7 == 0 */
        output.write_all(&value.to_be_bytes()[3..]).await
    } else {
        // Expanded-length case; it's a 32-bit length number (MSB is 1)

        //     unsigned char numB3;  /* numB3  >> 7 == 1 */
        //     unsigned char numB2;
        //     unsigned char numB1;
        //     unsigned char numB0;
        let mut res: [u8; 4] = value.to_be_bytes();
        res[0] |= 0x80; // Don't forget to mark this is a big byte
        output.write_all(&res).await
    }
}

/// Deserializes a [`u32`] from FCGI's compact style.
///
/// It returns [`None`] if we attempted to read from a stream that was empty at the start of the
/// function.
pub async fn read_u32_compact<R: AsyncRead + Unpin>(mut input: R) -> Result<Option<u32>, std::io::Error> {
    // Parse the number as a 32-bit number - but start at the first byte
    let mut num: [u8; 4] = [0; 4];
    if input.read(&mut num[..1]).await? == 0 {
        return Ok(None);
    };
    if num[0] <= 127 {
        // Simple-length case; it's a 8-bit, <= 127 number (MSB is 0)
        Ok(Some(num[0] as u32))
    } else {
        // Expanded-length case; it's a 32-bit length number (MSB is 1)
        input.read_exact(&mut num[1..]).await?;
        // NOTE: Before we return, don't forget to mask the telling MSB, as it's still the
        // MSB (i.e., it's no longer representing 2^7, but rather, 2^31)
        num[0] = num[0] & 0x7F;
        Ok(Some(u32::from_be_bytes(num)))
    }
}





/***** DATA TYPES *****/
/// Defines the possible version numbers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Version {
    /// Akin to `FCGI_VERSION_1`
    ///
    /// Value: `0x01`
    One,
}
impl TryFrom<u8> for Version {
    type Error = u8;

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Version::One),
            byte => Err(byte),
        }
    }
}
impl From<Version> for u8 {
    #[inline]
    fn from(value: Version) -> Self {
        match value {
            Version::One => 0x01,
        }
    }
}



/// Defines the possible record types.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RecordTy {
    /// Indicates the start of a request to an application.
    ///
    /// Value: `FCGI_BEGIN_REQUEST` (`0x01`)
    BeginRequest,
    /// Indicates the abortion of a request to an application.
    ///
    /// Value: `FCGI_ABORT_REQUEST` (`0x02`)
    AbortRequest,
    /// Indicates the conclusion of a request from the application side.
    ///
    /// Value: `FCGI_END_REQUEST` (`0x03`)
    EndRequest,
    /// Sends zero or more name/value pairs to the application.
    ///
    /// Value: `FCGI_PARAMS` (`0x04`)
    Params,
    /// Sends arbitrary bytes over the application's stdin.
    ///
    /// Value: `FCGI_STDIN` (`0x05`)
    Stdin,
    /// Receives arbitrary bytes over the application's stdout.
    ///
    /// Value: `FCGI_STDOUT` (`0x06`)
    Stdout,
    /// Receives arbitrary bytes over the application's stderr.
    ///
    /// Value: `FCGI_STDERR` (`0x07`)
    Stderr,
    /// Sends arbitrary bytes over the application's data channel.
    ///
    /// Value: `FCGI_DATA` (`0x08`)
    Data,
    /// Requests the value of various parameters from the application.
    ///
    /// Value: `FCGI_GET_VALUES` (`0x09`)
    GetValues,
    /// Returns the value of various parameters from the application.
    ///
    /// Value: `FCGI_GET_VALUES_RESULT` (`0x0A`)
    GetValuesResult,
    /// The application did not support the given record type.
    ///
    /// Value: `FCGI_UNKNOWN_TYPE` (`0x0B`)
    UnknownType,
}
impl TryFrom<u8> for RecordTy {
    type Error = u8;

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(RecordTy::BeginRequest),
            0x02 => Ok(RecordTy::AbortRequest),
            0x03 => Ok(RecordTy::EndRequest),
            0x04 => Ok(RecordTy::Params),
            0x05 => Ok(RecordTy::Stdin),
            0x06 => Ok(RecordTy::Stdout),
            0x07 => Ok(RecordTy::Stderr),
            0x08 => Ok(RecordTy::Data),
            0x09 => Ok(RecordTy::GetValues),
            0x0A => Ok(RecordTy::GetValuesResult),
            0x0B => Ok(RecordTy::UnknownType),
            byte => Err(byte),
        }
    }
}
impl From<RecordTy> for u8 {
    #[inline]
    fn from(value: RecordTy) -> Self {
        match value {
            RecordTy::BeginRequest => 0x01,
            RecordTy::AbortRequest => 0x02,
            RecordTy::EndRequest => 0x03,
            RecordTy::Params => 0x04,
            RecordTy::Stdin => 0x05,
            RecordTy::Stdout => 0x06,
            RecordTy::Stderr => 0x07,
            RecordTy::Data => 0x08,
            RecordTy::GetValues => 0x09,
            RecordTy::GetValuesResult => 0x0A,
            RecordTy::UnknownType => 0x0B,
        }
    }
}



/// Defines an interpretation of the header-part of a record.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordHeader {
    /// The version number of the record.
    pub version: Version,
    /// The type of the record.
    pub ty: RecordTy,
    /// The request/stream ID.
    pub request_id: u16,
    /// The amount of bytes in the content.
    pub content_len: u16,
    /// The amount of bytes in the padding.
    pub padding_len: u8,
    /// The reserved-byte.
    pub reserved: u8,
}
impl RecordHeader {
    /// Automatically compute the padding based on the content length.
    ///
    /// The padding will align the total record length to 8 bytes.
    ///
    /// # Returns
    /// Self for chaining.
    #[inline]
    pub const fn with_auto_padding(mut self) -> Self {
        let rem = (self.content_len % 8) as u8;
        if rem != 0 {
            self.padding_len = 8 - rem;
        }
        self
    }
}
impl<'a> TryFrom<&'a [u8]> for RecordHeader {
    type Error = RecordHeaderError;

    #[inline]
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        if value.len() < 8 {
            return Err(RecordHeaderError::NotEnoughBytes { got: value.len() });
        }

        // Parse the version & types
        let version = Version::try_from(value[0]).map_err(RecordHeaderError::UnknownVersion)?;
        let ty = RecordTy::try_from(value[1]).map_err(RecordHeaderError::UnknownTy)?;

        // Parse the request ID & lengths.
        let request_id = u16::from_be_bytes(*(value[2..4].as_array().unwrap()));
        let content_len = u16::from_be_bytes(*(value[4..6].as_array().unwrap()));
        let padding_len = value[6];
        let reserved = value[7];

        // Done
        Ok(Self { version, ty, request_id, content_len, padding_len, reserved })
    }
}
impl<const LEN: usize> TryFrom<[u8; LEN]> for RecordHeader {
    type Error = RecordHeaderError;

    #[inline]
    fn try_from(value: [u8; LEN]) -> Result<Self, Self::Error> { Self::try_from(value.as_slice()) }
}
impl TryFrom<Vec<u8>> for RecordHeader {
    type Error = RecordHeaderError;

    #[inline]
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> { Self::try_from(value.as_slice()) }
}
impl From<RecordHeader> for [u8; 8] {
    #[inline]
    fn from(value: RecordHeader) -> Self {
        let mut res = [0u8; 8];
        res[0] = value.version.into();
        res[1] = value.ty.into();
        res[2..4].clone_from_slice(&value.request_id.to_be_bytes());
        res[4..6].clone_from_slice(&value.content_len.to_be_bytes());
        res[6] = value.padding_len;
        res[7] = value.reserved;
        res
    }
}
impl From<RecordHeader> for Vec<u8> {
    #[inline]
    fn from(value: RecordHeader) -> Self { Vec::from(<[u8; 8]>::from(value)) }
}




/***** TESTS *****/
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_record_header() {
        assert_eq!(RecordHeader::try_from([0u8, 0, 0, 0, 0, 0, 0]), Err(RecordHeaderError::NotEnoughBytes { got: 7 }));
        assert_eq!(RecordHeader::try_from([0u8, 0, 0, 0, 0, 0, 0, 0]), Err(RecordHeaderError::UnknownVersion(0x00)));
        assert_eq!(RecordHeader::try_from([1u8, 0, 0, 0, 0, 0, 0, 0]), Err(RecordHeaderError::UnknownTy(0x00)));
        assert_eq!(
            RecordHeader::try_from([1u8, 1, 0, 0, 0, 0, 0, 0]),
            Ok(RecordHeader { version: Version::One, ty: RecordTy::BeginRequest, request_id: 0, content_len: 0, padding_len: 0, reserved: 0 })
        );
        assert_eq!(
            RecordHeader::try_from([1u8, 1, 1, 0, 0, 0, 0, 0]),
            Ok(RecordHeader { version: Version::One, ty: RecordTy::BeginRequest, request_id: 256, content_len: 0, padding_len: 0, reserved: 0 })
        );
        assert_eq!(
            RecordHeader::try_from([1u8, 1, 1, 1, 0, 0, 0, 0]),
            Ok(RecordHeader { version: Version::One, ty: RecordTy::BeginRequest, request_id: 257, content_len: 0, padding_len: 0, reserved: 0 })
        );
        assert_eq!(
            RecordHeader::try_from([1u8, 1, 1, 1, 1, 0, 0, 0]),
            Ok(RecordHeader { version: Version::One, ty: RecordTy::BeginRequest, request_id: 257, content_len: 256, padding_len: 0, reserved: 0 })
        );
        assert_eq!(
            RecordHeader::try_from([1u8, 1, 1, 1, 1, 1, 0, 0]),
            Ok(RecordHeader { version: Version::One, ty: RecordTy::BeginRequest, request_id: 257, content_len: 257, padding_len: 0, reserved: 0 })
        );
        assert_eq!(
            RecordHeader::try_from([1u8, 1, 1, 1, 1, 1, 1, 0]),
            Ok(RecordHeader { version: Version::One, ty: RecordTy::BeginRequest, request_id: 257, content_len: 257, padding_len: 1, reserved: 0 })
        );
        assert_eq!(
            RecordHeader::try_from([1u8, 1, 1, 1, 1, 1, 1, 1]),
            Ok(RecordHeader { version: Version::One, ty: RecordTy::BeginRequest, request_id: 257, content_len: 257, padding_len: 1, reserved: 1 })
        );
    }

    #[test]
    fn test_serialize_record_header() {
        assert_eq!(
            <[u8; 8]>::from(RecordHeader {
                version: Version::One,
                ty: RecordTy::BeginRequest,
                request_id: 0,
                content_len: 0,
                padding_len: 0,
                reserved: 0,
            }),
            [1u8, 1, 0, 0, 0, 0, 0, 0],
        );
        assert_eq!(
            <[u8; 8]>::from(RecordHeader {
                version: Version::One,
                ty: RecordTy::BeginRequest,
                request_id: 256,
                content_len: 0,
                padding_len: 0,
                reserved: 0,
            }),
            [1u8, 1, 1, 0, 0, 0, 0, 0],
        );
        assert_eq!(
            <[u8; 8]>::from(RecordHeader {
                version: Version::One,
                ty: RecordTy::BeginRequest,
                request_id: 257,
                content_len: 0,
                padding_len: 0,
                reserved: 0,
            }),
            [1u8, 1, 1, 1, 0, 0, 0, 0],
        );
        assert_eq!(
            <[u8; 8]>::from(RecordHeader {
                version: Version::One,
                ty: RecordTy::BeginRequest,
                request_id: 257,
                content_len: 256,
                padding_len: 0,
                reserved: 0,
            }),
            [1u8, 1, 1, 1, 1, 0, 0, 0],
        );
        assert_eq!(
            <[u8; 8]>::from(RecordHeader {
                version: Version::One,
                ty: RecordTy::BeginRequest,
                request_id: 257,
                content_len: 257,
                padding_len: 0,
                reserved: 0,
            }),
            [1u8, 1, 1, 1, 1, 1, 0, 0],
        );
        assert_eq!(
            <[u8; 8]>::from(RecordHeader {
                version: Version::One,
                ty: RecordTy::BeginRequest,
                request_id: 257,
                content_len: 257,
                padding_len: 1,
                reserved: 0,
            }),
            [1u8, 1, 1, 1, 1, 1, 1, 0],
        );
        assert_eq!(
            <[u8; 8]>::from(RecordHeader {
                version: Version::One,
                ty: RecordTy::BeginRequest,
                request_id: 257,
                content_len: 257,
                padding_len: 1,
                reserved: 1,
            }),
            [1u8, 1, 1, 1, 1, 1, 1, 1],
        );
    }
}
