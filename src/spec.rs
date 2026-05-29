//  SPEC.rs
//    by Lut99
//
//  Description:
//!   Defines the messages on the wire.
//

use std::cell::{Ref, RefMut};
use std::convert::Infallible;
use std::error::Error;
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::{Arc, MutexGuard, RwLockReadGuard, RwLockWriteGuard};

#[cfg(feature = "log")]
use log::trace;
use thiserror::Error;


/***** HELPER MACROS *****/
macro_rules! fast_cgi_bytes_ptr_impl {
    ('a, $ty:ty) => {
        impl<'a, T: ?Sized + ToFastCGIBytes> ToFastCGIBytes for $ty {
            #[inline]
            fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { <T as ToFastCGIBytes>::to_fcgi_bytes(self, output) }
        }
    };
    ($ty:ident<T>) => {
        impl<T: ?Sized + ToFastCGIBytes> ToFastCGIBytes for $ty<T> {
            #[inline]
            fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { <T as ToFastCGIBytes>::to_fcgi_bytes(self, output) }
        }
        impl<T: FromFastCGIBytes> FromFastCGIBytes for $ty<T> {
            type Error = <T as FromFastCGIBytes>::Error;

            #[inline]
            fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> {
                <T as FromFastCGIBytes>::from_fcgi_bytes(input).map(|r| r.map($ty::new))
            }
        }
    };
}

macro_rules! escape_none {
    ($e:expr) => {
        if let Some(res) = $e { res } else { return Ok(None) }
    };
}




/***** CONSTANTS *****/
/// Defines the name of the parameter defining the maximum number of concurrent transport
/// connections an application supports.
pub const PARAM_MAX_CONNS: &'static str = "FCGI_MAX_CONNS";
/// Defines the name of the parameter defining the maximum number of concurrent requests an
/// application supports.
pub const PARAM_MAX_REQS: &'static str = "FCGI_MAX_REQS";
/// Defines the name of the parameter defining whether an application multiplexes connections.
pub const PARAM_MPXS_CONNS: &'static str = "FCGI_MPXS_CONNS";





/***** ERRORS *****/
/// Error for failing to parse a string.
#[derive(Debug, Error)]
pub enum StringError {
    #[error("Failed to read from reader")]
    Read(#[from] std::io::Error),
    #[error("Missing null-byte when parsing string")]
    MissingNull,
    #[error("Got invalid UTF-8 when parsing string")]
    FromUtf8(#[from] std::string::FromUtf8Error),
}

/// Error for failing to parse a [`Version`].
#[derive(Debug, Error)]
pub enum VersionError {
    #[error("Failed to read from reader")]
    Read(#[from] std::io::Error),
    #[error("Unknown version byte 0x{0:02X}")]
    Unknown(u8),
}

/// Error for failing to parse a [`Pair`].
#[derive(Debug, Error)]
pub enum PairError<N, V> {
    #[error("Failed to read from reader")]
    Read(#[from] std::io::Error),
    #[error("Failed to read name")]
    Name(#[source] N),
    #[error("Failed to read value")]
    Value(#[source] V),
}

/// Error for failing to parse a [`Record`].
#[derive(Debug, Error)]
pub enum RecordError<E> {
    #[error("Failed to read from reader")]
    Read(#[from] std::io::Error),
    #[error("{0}")]
    Version(#[from] VersionError),
    #[error("Failed to read content")]
    Content(#[source] E),
}





/***** INTERFACES *****/
/// Defines that we can serialize it to bytes.
pub trait ToFastCGIBytes {
    /// Can reserialize self to a sequence of bytes.
    ///
    /// For efficiency purposes, takes anything [`Write`]able.
    ///
    /// # Arguments
    /// - `output`: Something `W`ritable that a serialization of Self is written to.
    ///
    /// # Errors
    /// This can only error if we failed to write to `W`.
    ///
    /// Note that as such, this does **not** return [`FastCGIBytes::Error`]!
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error>;
}
/// Defines that we read it from bytes.
pub trait FromFastCGIBytes: Sized {
    type Error: 'static + Error;

    /// Can construct self from a sequence of bytes.
    ///
    /// For efficiency purposes, takes anything [`Read`]able.
    ///
    /// # Arguments
    /// - `input`: Something `R`eadable that we attempt to parse a serialization of Self from.
    ///
    /// # Returns
    /// A new instance of Self, or [`None`] if there was no more `input`.
    ///
    /// # Errors
    /// This function can error if we failed to read from the `input`, or else if the input was not
    /// a valid serialization of `self`.
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error>;
}

// Standard impls
impl ToFastCGIBytes for () {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, _output: W) -> Result<(), std::io::Error> { Ok(()) }
}
impl FromFastCGIBytes for () {
    type Error = Infallible;

    #[inline]
    fn from_fcgi_bytes<R: Read>(_input: R) -> Result<Option<Self>, Self::Error> { Ok(Some(())) }
}
impl ToFastCGIBytes for u8 {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        // Write it, simply
        output.write_all(std::slice::from_ref(self))
    }
}
impl FromFastCGIBytes for u8 {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        // Read a byte
        let mut byte: u8 = 0;
        let n: usize = input.read(std::slice::from_mut(&mut byte))?;
        if n >= 1 { Ok(Some(byte)) } else { Ok(None) }
    }
}
impl ToFastCGIBytes for u16 {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> { output.write_all(&self.to_be_bytes()) }
}
impl FromFastCGIBytes for u16 {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        // Read two bytes
        let mut bytes: [u8; 2] = [0, 0];
        let n: usize = input.read(&mut bytes)?;
        if n >= 2 { Ok(Some(u16::from_be_bytes(bytes))) } else { Ok(None) }
    }
}
impl<T: ToFastCGIBytes> ToFastCGIBytes for [T] {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        for elem in self {
            elem.to_fcgi_bytes(&mut output)?;
        }
        Ok(())
    }
}
impl<T: ToFastCGIBytes> ToFastCGIBytes for Vec<T> {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { <[T]>::to_fcgi_bytes(self, output) }
}
impl<T: FromFastCGIBytes> FromFastCGIBytes for Vec<T> {
    type Error = T::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        let mut res = Vec::new();
        #[cfg(feature = "log")]
        let mut i: usize = 0;
        loop {
            #[cfg(feature = "log")]
            {
                log::trace!("Attempting {} entry {i}", std::any::type_name::<Self>());
                i += 1;
            }
            match T::from_fcgi_bytes(&mut input)? {
                Some(elem) => res.push(elem),
                None => return Ok(Some(res)),
            }
        }
    }
}
impl ToFastCGIBytes for str {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        // Write the bytes of the string
        output.write_all(self.as_bytes())?;
        // Then write the null-byte
        output.write_all(&[0x00])?;
        Ok(())
    }
}
impl ToFastCGIBytes for String {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { <str>::to_fcgi_bytes(self.as_str(), output) }
}
impl FromFastCGIBytes for String {
    type Error = StringError;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        const BUF_LEN: usize = 8192;

        // Read up to the first null-byte
        let mut buf = Vec::new();
        let mut buf_len: usize = 0;
        loop {
            // Load a chunk into the buffer
            buf.extend(std::iter::repeat(0).take(BUF_LEN));
            let read_len: usize = input.read(&mut buf[buf_len..BUF_LEN]).map_err(StringError::Read)?;
            if read_len == 0 {
                return Ok(None);
            }

            // Search it for the null-byte
            for (i, b) in buf[buf_len..BUF_LEN].iter().copied().enumerate() {
                if b == 0x00 {
                    // Found the end; everything up to here is the string
                    buf.truncate(buf_len + i);
                    return String::from_utf8(buf).map(Some).map_err(StringError::FromUtf8);
                }
            }

            // Continue searching
            buf_len += BUF_LEN;
        }
    }
}

// Pointer-like impls
fast_cgi_bytes_ptr_impl!('a, &'a T);
fast_cgi_bytes_ptr_impl!('a, &'a mut T);
fast_cgi_bytes_ptr_impl!(Box<T>);
fast_cgi_bytes_ptr_impl!(Rc<T>);
fast_cgi_bytes_ptr_impl!(Arc<T>);
fast_cgi_bytes_ptr_impl!('a, Ref<'a, T>);
fast_cgi_bytes_ptr_impl!('a, RefMut<'a, T>);
fast_cgi_bytes_ptr_impl!('a, MutexGuard<'a, T>);
fast_cgi_bytes_ptr_impl!('a, RwLockReadGuard<'a, T>);
fast_cgi_bytes_ptr_impl!('a, RwLockWriteGuard<'a, T>);





/***** AUXILLARY *****/
/// Defines the possible version numbers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Version {
    /// Akin to `FCGI_VERSION_1`
    ///
    /// Value: `0x01`
    One,
}
impl ToFastCGIBytes for Version {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        output.write_all(std::slice::from_ref(match self {
            Self::One => &0x01,
        }))
    }
}
impl FromFastCGIBytes for Version {
    type Error = VersionError;

    #[inline]
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> {
        // Read a byte
        match u8::from_fcgi_bytes(input).map_err(VersionError::Read)? {
            Some(0x01) => Ok(Some(Self::One)),
            Some(byte) => Err(VersionError::Unknown(byte)),
            None => Ok(None),
        }
    }
}

/// Defines the possible record types.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RecordTy {
    /// Start of a request.
    ///
    /// Value: `0x01` (`FCGI_BEGIN_REQUEST`)
    BeginRequest,
    /// Dirty exit of a request.
    ///
    /// Value: `0x02` (`FCGI_ABORT_REQUEST`)
    AbortRequest,
    /// Clean exit of a request.
    ///
    /// Value: `0x03` (`FCGI_END_REQUEST`)
    EndRequest,
    /// Send parameters to the binary.
    ///
    /// Value: `0x04` (`FCGI_PARAMS`)
    Params,
    /// Message to stream stdin bytes to the application.
    ///
    /// Value: `0x05` (`FCGI_STDIN`)
    Stdin,
    /// Message to stream stdout bytes back to the server.
    ///
    /// Value: `0x06` (`FCGI_STDOUT`)
    Stdout,
    /// Message to stream stderr bytes back to the server.
    ///
    /// Value: `0x07` (`FCGI_STDERR`)
    Stderr,
    /// TODO
    ///
    /// Value: `0x08` (`FCGI_DATA`)
    Data,
    /// TODO
    ///
    /// Value: `0x09` (`FCGI_GET_VALUES`)
    GetValues,
    /// TODO
    ///
    /// Value: `0x0A` (`FCGI_GET_VALUES_RESULT`)
    GetValuesResult,
    /// Leftover type we serialize to if we don't know.
    ///
    /// Value: `0x0B` (`FCGI_UNKNOWN_TYPE`)
    UnknownType,
}
impl ToFastCGIBytes for RecordTy {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        output.write_all(std::slice::from_ref(match self {
            Self::BeginRequest => &0x01,
            Self::AbortRequest => &0x02,
            Self::EndRequest => &0x03,
            Self::Params => &0x04,
            Self::Stdin => &0x05,
            Self::Stdout => &0x06,
            Self::Stderr => &0x07,
            Self::Data => &0x08,
            Self::GetValues => &0x09,
            Self::GetValuesResult => &0x0A,
            Self::UnknownType => &0x0B,
        }))
    }
}
impl FromFastCGIBytes for RecordTy {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> {
        // Read a byte
        match u8::from_fcgi_bytes(input)? {
            Some(0x01) => Ok(Some(Self::BeginRequest)),
            Some(0x02) => Ok(Some(Self::AbortRequest)),
            Some(0x03) => Ok(Some(Self::EndRequest)),
            Some(0x04) => Ok(Some(Self::Params)),
            Some(0x05) => Ok(Some(Self::Stdin)),
            Some(0x06) => Ok(Some(Self::Stdout)),
            Some(0x07) => Ok(Some(Self::Stderr)),
            Some(0x08) => Ok(Some(Self::Data)),
            Some(0x09) => Ok(Some(Self::GetValues)),
            Some(0x0A) => Ok(Some(Self::GetValuesResult)),
            Some(0x0B | _) => Ok(Some(Self::UnknownType)),
            None => Ok(None),
        }
    }
}





/***** GENERAL *****/
/// Defines a name/value pair for use in FastCGI data.
///
/// # Generics
/// - `N`: The type of the name. You can replace this with something implementing [`FastCGIBytes`]
///   to assume/enforce an encoding.
/// - `V`: The type of the value. You can replace this with something implementing [`FastCGIBytes`]
///   to assume/enforce an encoding.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Pair<N = Vec<u8>, V = Vec<u8>> {
    /// The name.
    pub name:  N,
    /// The value.
    pub value: V,
}
impl<N: ToFastCGIBytes, V: ToFastCGIBytes> ToFastCGIBytes for Pair<N, V> {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        // NOTE: The length of the nameLength/valueLength numbers varies!
        // typedef struct {
        //     unsigned char nameLengthB3;  /* nameLengthB3  >> 7 == 1 */
        //     unsigned char nameLengthB2;
        //     unsigned char nameLengthB1;
        //     unsigned char nameLengthB0;
        //     unsigned char valueLengthB3; /* valueLengthB3 >> 7 == 1 */
        //     unsigned char valueLengthB2;
        //     unsigned char valueLengthB1;
        //     unsigned char valueLengthB0;
        //     unsigned char nameData[nameLength
        //             ((B3 & 0x7f) << 24) + (B2 << 16) + (B1 << 8) + B0];
        //     unsigned char valueData[valueLength
        //             ((B3 & 0x7f) << 24) + (B2 << 16) + (B1 << 8) + B0];
        // } FCGI_NameValuePair44;

        let mut name: Vec<u8> = Vec::new();
        let mut value: Vec<u8> = Vec::new();
        self.name.to_fcgi_bytes(&mut name)?;
        self.value.to_fcgi_bytes(&mut value)?;
        let name_len: u32 = name.len() as u32;
        let value_len: u32 = value.len() as u32;

        //     unsigned char nameLengthB3;  /* nameLengthB3  >> 7 == 1 */
        //     unsigned char nameLengthB2;
        //     unsigned char nameLengthB1;
        //     unsigned char nameLengthB0;
        if name_len <= 127 {
            // Simple-length case; it's a 8-bit, <= 127 number (MSB is 0)
            output.write_all(&name_len.to_be_bytes()[3..])?;
        } else {
            // Expanded-length case; it's a 32-bit length number (MSB is 1)
            output.write_all(&name_len.to_be_bytes())?;
        }
        //     unsigned char valueLengthB3; /* valueLengthB3 >> 7 == 1 */
        //     unsigned char valueLengthB2;
        //     unsigned char valueLengthB1;
        //     unsigned char valueLengthB0;
        if value_len <= 127 {
            // Simple-length case; it's a 8-bit, <= 127 number (MSB is 0)
            output.write_all(&value_len.to_be_bytes()[3..])?;
        } else {
            // Expanded-length case; it's a 32-bit length number (MSB is 1)
            output.write_all(&value_len.to_be_bytes())?;
        }
        //     unsigned char nameData[nameLength
        //             ((B3 & 0x7f) << 24) + (B2 << 16) + (B1 << 8) + B0];
        output.write_all(&name)?;
        //     unsigned char valueData[valueLength
        //             ((B3 & 0x7f) << 24) + (B2 << 16) + (B1 << 8) + B0];
        output.write_all(&value)?;

        Ok(())
    }
}
impl<N: FromFastCGIBytes, V: FromFastCGIBytes> FromFastCGIBytes for Pair<N, V> {
    type Error = PairError<N::Error, V::Error>;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        fn read_8_or_31_bit_number<R: Read>(mut input: R) -> Result<Option<u32>, std::io::Error> {
            // Parse the length of the name buffer
            let mut length_bytes: [u8; 4] = [0; 4];
            length_bytes[0] = escape_none!(u8::from_fcgi_bytes(&mut input)?);
            if length_bytes[0] <= 127 {
                // Simple-length case; it's a 8-bit, <= 127 number (MSB is 0)
                Ok(Some(length_bytes[0] as u32))
            } else {
                // Expanded-length case; it's a 32-bit length number (MSB is 1)
                let mut length_bytes_i: usize = 1;
                while length_bytes_i < 4 {
                    let len: usize = input.read(&mut length_bytes[length_bytes_i..])?;
                    if len == 0 {
                        return Ok(None);
                    }
                    length_bytes_i += len;
                }
                // NOTE: Before we return, don't forget to mask the telling MSB, as it's still the
                // MSB (i.e., it's no longer representing 2^7, but rather, 2^31)
                length_bytes[0] = length_bytes[0] & 0x7F;
                Ok(Some(u32::from_be_bytes(length_bytes)))
            }
        }

        #[cfg(feature = "log")]
        log::trace!("Attempting {}", std::any::type_name::<Self>());
        // NOTE: The length of the nameLength/valueLength numbers varies!
        // typedef struct {
        //     unsigned char nameLengthB3;  /* nameLengthB3  >> 7 == 1 */
        //     unsigned char nameLengthB2;
        //     unsigned char nameLengthB1;
        //     unsigned char nameLengthB0;
        //     unsigned char valueLengthB3; /* valueLengthB3 >> 7 == 1 */
        //     unsigned char valueLengthB2;
        //     unsigned char valueLengthB1;
        //     unsigned char valueLengthB0;
        //     unsigned char nameData[nameLength
        //             ((B3 & 0x7f) << 24) + (B2 << 16) + (B1 << 8) + B0];
        //     unsigned char valueData[valueLength
        //             ((B3 & 0x7f) << 24) + (B2 << 16) + (B1 << 8) + B0];
        // } FCGI_NameValuePair44;

        //     unsigned char nameLengthB3;  /* nameLengthB3  >> 7 == 1 */
        //     unsigned char nameLengthB2;
        //     unsigned char nameLengthB1;
        //     unsigned char nameLengthB0;
        let name_len: u32 = escape_none!(read_8_or_31_bit_number(&mut input).map_err(PairError::Read)?);
        #[cfg(feature = "log")]
        log::trace!("Parsed name length: {name_len} bytes");
        //     unsigned char valueLengthB3; /* valueLengthB3 >> 7 == 1 */
        //     unsigned char valueLengthB2;
        //     unsigned char valueLengthB1;
        //     unsigned char valueLengthB0;
        let value_len: u32 = escape_none!(read_8_or_31_bit_number(&mut input).map_err(PairError::Read)?);
        #[cfg(feature = "log")]
        log::trace!("Parsed value length: {name_len} bytes");
        //     unsigned char nameData[nameLength
        //             ((B3 & 0x7f) << 24) + (B2 << 16) + (B1 << 8) + B0];
        let mut name_i: usize = 0;
        let mut name: Vec<u8> = vec![0; name_len as usize];
        while name_i < name_len as usize {
            let len: usize = input.read(&mut name[name_i..]).map_err(PairError::Read)?;
            if len == 0 {
                return Ok(None);
            }
            name_i += len;
        }
        let name = escape_none!(N::from_fcgi_bytes(name.as_slice()).map_err(PairError::Name)?);
        //     unsigned char valueData[valueLength
        //             ((B3 & 0x7f) << 24) + (B2 << 16) + (B1 << 8) + B0];
        let mut value_i: usize = 0;
        let mut value: Vec<u8> = vec![0; value_len as usize];
        while value_i < value_len as usize {
            let len: usize = input.read(&mut value[value_i..]).map_err(PairError::Read)?;
            if len == 0 {
                return Ok(None);
            }
            value_i += len;
        }
        let value = escape_none!(V::from_fcgi_bytes(value.as_slice()).map_err(PairError::Value)?);

        Ok(Some(Self { name, value }))
    }
}



/// Defines the main FastCGI record.
///
/// # Generics
/// - `C`: The type of the content. You can replace this with something implementing
///   [`FastCGIBytes`] to assume/enforce an encoding.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Record<C = Vec<u8>> {
    /// The version number of the record.
    pub version: Version,
    /// The type of the record.
    pub ty: RecordTy,
    /// The request/stream ID.
    pub request_id: u16,
    /// The amount of padding that was applied when sending this record.
    pub padding_length: Option<u8>,
    /// The reserved-byte from the header.
    pub reserved: Option<u8>,
    /// The content, potentially something parsed already.
    pub content: C,
}
impl<C: ToFastCGIBytes> ToFastCGIBytes for Record<C> {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        // typedef struct {
        //     unsigned char version;
        //     unsigned char type;
        //     unsigned char requestIdB1;
        //     unsigned char requestIdB0;
        //     unsigned char contentLengthB1;
        //     unsigned char contentLengthB0;
        //     unsigned char paddingLength;
        //     unsigned char reserved;
        //     unsigned char contentData[contentLength];
        //     unsigned char paddingData[paddingLength];
        // } FCGI_Record;

        let mut content: Vec<u8> = Vec::new();
        self.content.to_fcgi_bytes(&mut content)?;
        let padding_len: u8 = self.padding_length.unwrap_or_else(|| if content.len() % 8 > 0 { 8u8 - (content.len() % 8) as u8 } else { 0 });

        //     unsigned char version;
        self.version.to_fcgi_bytes(&mut output)?;
        //     unsigned char type;
        self.ty.to_fcgi_bytes(&mut output)?;
        //     unsigned char requestIdB1;
        //     unsigned char requestIdB0;
        self.request_id.to_fcgi_bytes(&mut output)?;
        //     unsigned char contentLengthB1;
        //     unsigned char contentLengthB0;
        (content.len() as u16).to_fcgi_bytes(&mut output)?;
        //     unsigned char paddingLength;
        // NOTE: Padded to a multiple of eight
        padding_len.to_fcgi_bytes(&mut output)?;
        //     unsigned char reserved;
        self.reserved.unwrap_or(0u8).to_fcgi_bytes(&mut output)?;
        //     unsigned char contentData[contentLength];
        output.write_all(&content)?;
        //     unsigned char paddingData[paddingLength];
        for _ in 0..padding_len {
            0u8.to_fcgi_bytes(&mut output)?;
        }

        Ok(())
    }
}
impl<C: FromFastCGIBytes> FromFastCGIBytes for Record<C> {
    type Error = RecordError<C::Error>;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        // typedef struct {
        //     unsigned char version;
        //     unsigned char type;
        //     unsigned char requestIdB1;
        //     unsigned char requestIdB0;
        //     unsigned char contentLengthB1;
        //     unsigned char contentLengthB0;
        //     unsigned char paddingLength;
        //     unsigned char reserved;
        //     unsigned char contentData[contentLength];
        //     unsigned char paddingData[paddingLength];
        // } FCGI_Record;

        #[cfg(feature = "log")]
        log::trace!("Attempting {}", std::any::type_name::<Self>());
        //     unsigned char version;
        let version = escape_none!(Version::from_fcgi_bytes(&mut input).map_err(RecordError::Version)?);
        #[cfg(feature = "log")]
        log::trace!("Parsed version: {version:?}");
        //     unsigned char type;
        let ty = escape_none!(RecordTy::from_fcgi_bytes(&mut input).map_err(RecordError::Read)?);
        #[cfg(feature = "log")]
        log::trace!("Parsed type: {ty:?}");
        //     unsigned char requestIdB1;
        //     unsigned char requestIdB0;
        let request_id = escape_none!(u16::from_fcgi_bytes(&mut input).map_err(RecordError::Read)?);
        #[cfg(feature = "log")]
        log::trace!("Parsed request ID: {request_id:?}");
        //     unsigned char contentLengthB1;
        //     unsigned char contentLengthB0;
        let content_len = escape_none!(u16::from_fcgi_bytes(&mut input).map_err(RecordError::Read)?);
        #[cfg(feature = "log")]
        log::trace!("Parsed content length: {content_len} bytes");
        //     unsigned char paddingLength;
        let padding_length = escape_none!(u8::from_fcgi_bytes(&mut input).map_err(RecordError::Read)?);
        #[cfg(feature = "log")]
        log::trace!("Parsed padding length: {padding_length} bytes");
        //     unsigned char reserved;
        let reserved = escape_none!(u8::from_fcgi_bytes(&mut input).map_err(RecordError::Read)?);
        #[cfg(feature = "log")]
        log::trace!("Reserved byte: {reserved:?}");
        //     unsigned char contentData[contentLength];
        let mut content_i: usize = 0;
        let mut content: Vec<u8> = vec![0; content_len as usize];
        while content_i < content_len as usize {
            let len: usize = input.read(&mut content[content_i..]).map_err(RecordError::Read)?;
            if len == 0 {
                return Ok(None);
            }
            content_i += len;
        }
        let content = escape_none!(C::from_fcgi_bytes(content.as_slice()).map_err(RecordError::Content)?);
        //     unsigned char paddingData[paddingLength];
        // NOTE: We just pop this
        for _ in 0..padding_length {
            if u8::from_fcgi_bytes(&mut input).map_err(RecordError::Read)?.is_none() {
                return Ok(None);
            }
        }

        Ok(Some(Self { version, ty, request_id, padding_length: Some(padding_length), reserved: Some(reserved), content }))
    }
}





/***** MANAGEMENT RECORD TYPES *****/
/// Represents a [`RecordError`] instantiated to parse [`GetvaluesRecord`]s.
pub type GetValuesRecordError = RecordError<PairError<StringError, Infallible>>;

/// Represents a [`Record`] instantiated to request a sequence of parameter values from the
/// application.
pub type GetValuesRecord<'p> = Record<Vec<Pair<&'p str, ()>>>;

impl<'p> GetValuesRecord<'p> {
    /// Constructor for a [`GetValuesRecord`].
    ///
    /// # Arguments
    /// - `params`: An exhaustive list of parameters to request the value of in the application. To
    ///   request the factory parameters, give [`PARAM_MAX_CONNS`], [`PARAM_MAX_REQS`] and
    ///   [`PARAM_MPXS_CONNS`].
    ///
    /// # Returns
    /// A new Record that represents a GetValuesRecord for the given `params`.
    #[inline]
    pub fn new_get_values_record(params: impl IntoIterator<Item = &'p str>) -> Self {
        Self {
            version: Version::One,
            ty: RecordTy::GetValues,
            request_id: 0,
            padding_length: None,
            reserved: None,
            content: params.into_iter().map(|p| Pair { name: p, value: () }).collect(),
        }
    }
}



/// Represents a [`RecordError`] instantiated to parse [`GetvaluesRecord`]s.
pub type GetValuesResultRecordError = RecordError<PairError<StringError, StringError>>;

/// Represents a [`Record`] instantiated as response to a [`GetValuesRecord`].
pub type GetValuesResultRecord = Record<Vec<Pair<String, String>>>;





/***** TESTS *****/
#[cfg(test)]
mod tests {
    use super::*;

    fn vectorize<T: ToFastCGIBytes>(obj: T) -> Vec<u8> {
        let mut res = Vec::new();
        obj.to_fcgi_bytes(&mut res).unwrap();
        res
    }
    fn devectorize<T: FromFastCGIBytes>(vec: &[u8]) -> Option<T> {
        match T::from_fcgi_bytes(vec) {
            Ok(res) => res,
            Err(err) => panic!("{err}"),
        }
    }

    #[test]
    fn test_assert_to_fcgi_bytes() {
        #[inline]
        const fn assert_to_fcgi_bytes<T: ToFastCGIBytes>() {}


        assert_to_fcgi_bytes::<()>();
        assert_to_fcgi_bytes::<&'static str>();
        assert_to_fcgi_bytes::<Pair<&'static str, ()>>();
        assert_to_fcgi_bytes::<Vec<Pair<&'static str, ()>>>();
        assert_to_fcgi_bytes::<GetValuesRecord<'static>>();
    }

    #[test]
    fn test_string_to_fcgi_bytes() {
        assert_eq!(vectorize(String::new()), b"\0");
        assert_eq!(vectorize(String::from("Hello, world!")), b"Hello, world!\0");
    }
    #[test]
    fn test_string_from_fcgi_bytes() {
        assert_eq!(devectorize(b"\0"), Some(String::new()));
        assert_eq!(devectorize(b"Hello, world!\0"), Some(String::from("Hello, world!")));
        assert_eq!(devectorize(b"Hello\0, world!\0"), Some(String::from("Hello")));
    }

    #[test]
    fn test_pair_to_fcgi_bytes() {
        assert_eq!(vectorize(Pair { name: String::new(), value: () }), b"\x01\0\0");
        assert_eq!(vectorize(Pair { name: String::from("foo"), value: String::from("bar") }), b"\x04\x04foo\0bar\0");
        assert_eq!(
            vectorize(Pair {
                name:  String::from(
                    "Did you ever hear the tragedy of Darth Plagueis The Wise? I thought not. It's not a story the Jedi would tell you. It's a Sith \
                     legend. Darth Plagueis was a Dark Lord of the Sith, so powerful and so wise he could use the Force to influence the \
                     midichlorians to create life... He had such a knowledge of the dark side that he could even keep the ones he cared about from \
                     dying. The dark side of the Force is a pathway to many abilities some consider to be unnatural. He became so powerful... the \
                     only thing he was afraid of was losing his power, which eventually, of course, he did. Unfortunately, he taught his apprentice \
                     everything he knew, then his apprentice killed him in his sleep. Ironic. He could save others from death, but not himself."
                ),
                value: String::from("bar"),
            }),
            b"\0\0\x02\xE8\x04Did you ever hear the tragedy of Darth Plagueis The Wise? I thought not. It's not a story the Jedi would tell you. It's a Sith legend. Darth Plagueis was a Dark Lord of the Sith, so powerful and so wise he could use the Force to influence the midichlorians to create life... He had such a knowledge of the dark side that he could even keep the ones he cared about from dying. The dark side of the Force is a pathway to many abilities some consider to be unnatural. He became so powerful... the only thing he was afraid of was losing his power, which eventually, of course, he did. Unfortunately, he taught his apprentice everything he knew, then his apprentice killed him in his sleep. Ironic. He could save others from death, but not himself.\0bar\0"
        );
    }

    #[test]
    fn test_record_to_fcgi_bytes() {
        assert_eq!(
            vectorize(Record {
                version: Version::One,
                ty: RecordTy::GetValues,
                request_id: 0,
                padding_length: None,
                reserved: None,
                content: vec![Pair { name: "FCGI_MAX_CONNS".to_string(), value: () }, Pair { name: "FCGI_MAX_REQS".to_string(), value: () }, Pair {
                    name:  "FCGI_MPXS_CONNS".to_string(),
                    value: (),
                }],
            }),
            b"\x01\x09\0\0\0\x33\x05\0\x0f\0FCGI_MAX_CONNS\0\x0e\0FCGI_MAX_REQS\0\x10\0FCGI_MPXS_CONNS\0\0\0\0\0\0"
        );
    }
    #[test]
    fn test_record_from_fcgi_bytes() {
        assert_eq!(
            devectorize(b"\x01\x09\0\0\0\x33\x05\0\x0f\0FCGI_MAX_CONNS\0\x0e\0FCGI_MAX_REQS\0\x10\0FCGI_MPXS_CONNS\0\0\0\0\0\0"),
            Some(Record {
                version: Version::One,
                ty: RecordTy::GetValues,
                request_id: 0,
                padding_length: Some(5),
                reserved: Some(0),
                content: vec![Pair { name: "FCGI_MAX_CONNS".to_string(), value: () }, Pair { name: "FCGI_MAX_REQS".to_string(), value: () }, Pair {
                    name:  "FCGI_MPXS_CONNS".to_string(),
                    value: (),
                }],
            },)
        );
    }
}
