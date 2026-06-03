//  SPEC.rs
//    by Lut99
//
//  Description:
//!   Defines the messages on the wire.
//

use std::borrow::Cow;
use std::cell::{Ref, RefMut};
use std::convert::Infallible;
use std::error::Error;
use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::rc::Rc;
use std::sync::{Arc, MutexGuard, RwLockReadGuard, RwLockWriteGuard};

use thiserror::Error;


/***** HELPER MACROS *****/
macro_rules! fast_cgi_bytes_ptr_impl {
    ('a, Cow<'a, T>) => {
        impl<'a, T: ?Sized + ToOwned + ToFastCGIBytes> ToFastCGIBytes for Cow<'a, T> {
            #[inline]
            fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { <T as ToFastCGIBytes>::to_fcgi_bytes(self, output) }
        }
        impl<T: ?Sized + ToOwned> FromFastCGIBytes for Cow<'static, T>
        where
            T::Owned: FromFastCGIBytes,
        {
            type Error = <T::Owned as FromFastCGIBytes>::Error;

            #[inline]
            fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> {
                <T::Owned as FromFastCGIBytes>::from_fcgi_bytes(input).map(|r| r.map(Cow::Owned))
            }
        }
    };
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
/// Error for failing to parse a [`u16`].
#[derive(Debug, Error)]
#[allow(non_camel_case_types)]
pub enum u16Error {
    #[error("Failed to read from reader")]
    Read(#[from] std::io::Error),
    #[error("Not enough bytes were present (got {0}, expected 2)")]
    NotEnough(usize),
}

/// Error for failing to parse an array.
#[derive(Debug, Error)]
pub enum ArrayError<E> {
    #[error("Failed to read element of type {what:?}")]
    Elem { what: &'static str, err: E },
    #[error("Not enough elements (expected {expected}, got {got})")]
    NotEnough { expected: usize, got: usize },
}

/// Error for failing to parse a string.
#[derive(Debug, Error)]
pub enum StringError {
    #[error("Failed to read from reader")]
    Read(#[from] std::io::Error),
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

/// Error for failing to parse a [`RecordBody`]
#[derive(Debug, Error)]
pub enum RecordBodyError {
    #[error("Failed to read an FCGI_BEGIN_REQUEST record")]
    BeginRequest(#[source] std::io::Error),
    #[error("Failed to read an FCGI_ABORT_REQUEST record")]
    AbortRequest(#[source] std::io::Error),
    #[error("Failed to read an FCGI_END_REQUEST record")]
    EndRequest(#[source] std::io::Error),
    #[error("Failed to read an FCGI_PARAMS record")]
    Params(#[source] std::io::Error),
    #[error("Failed to read an FCGI_STDIN record")]
    Stdin(#[source] std::io::Error),
    #[error("Failed to read an FCGI_STDOUT record")]
    Stdout(#[source] std::io::Error),
    #[error("Failed to read an FCGI_STDERR record")]
    Stderr(#[source] std::io::Error),
    #[error("Failed to read an FCGI_DATA record")]
    Data(#[source] std::io::Error),
    #[error("Failed to read an FCGI_GET_VALUES record")]
    GetValues(#[source] PairError<StringError, Infallible>),
    #[error("Failed to read an FCGI_GET_VALUES_RESULT record")]
    GetValuesResult(#[source] PairError<StringError, StringError>),
    #[error("Failed to read an FCGI_UNKNOWN_TYPE record")]
    UnknownType(#[source] RecordUnknownTypeError),
}

/// Error for failing to parse a [`RecordUnknownType`].
#[derive(Debug, Error)]
pub enum RecordUnknownTypeError {
    #[error("Failed to read the type-byte")]
    Ty(#[from] std::io::Error),
    #[error("Failed to read the reserved-bytes")]
    Reserved(#[from] ArrayError<std::io::Error>),
}

/// Error for failing to parse a [`Record`].
#[derive(Debug, Error)]
pub enum RecordError {
    #[error("Failed to read from reader")]
    Read(#[from] std::io::Error),
    #[error("{0}")]
    #[allow(non_camel_case_types)]
    u16(#[from] u16Error),
    #[error("{0}")]
    Version(#[from] VersionError),
    #[error("Failed to read body of record")]
    Body(#[from] RecordBodyError),
}





/***** HELPERS *****/
/// A [`Read`]er that fixes the amount of bytes read.
#[derive(Debug)]
struct FixedLenReader<R> {
    reader: R,
    i:      usize,
    max:    usize,
}
impl<R> FixedLenReader<R> {
    #[inline]
    fn new(max: usize, reader: R) -> Self { Self { reader, i: 0, max } }
}
impl<R: Read> Read for FixedLenReader<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Check how many bytes we've read
        let togo: usize = self.max - self.i;
        let n: usize = if buf.len() >= togo { self.reader.read(&mut buf[..togo as usize])? } else { self.reader.read(buf)? };
        self.i += n;
        Ok(n)
    }
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
    type Error = u16Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        // Read two bytes
        let mut bytes_i: usize = 0;
        let mut bytes: [u8; 2] = [0, 0];
        while bytes_i < 2 {
            let n: usize = input.read(&mut bytes[bytes_i..])?;
            if n == 0 {
                if bytes_i == 0 {
                    return Ok(None);
                }
                return Err(u16Error::NotEnough(bytes_i));
            }
            bytes_i += n;
        }
        Ok(Some(u16::from_be_bytes(bytes)))
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
impl<const LEN: usize, T: ToFastCGIBytes> ToFastCGIBytes for [T; LEN] {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { <[T]>::to_fcgi_bytes(self, output) }
}
impl<const LEN: usize, T: FromFastCGIBytes> FromFastCGIBytes for [T; LEN] {
    type Error = ArrayError<T::Error>;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        let mut res: [MaybeUninit<T>; LEN] = [const { MaybeUninit::uninit() }; LEN];
        for i in 0..LEN {
            match T::from_fcgi_bytes(&mut input) {
                Ok(Some(elem)) => res[i].write(elem),
                Ok(None) => return Err(ArrayError::NotEnough { got: i, expected: LEN }),
                Err(err) => return Err(ArrayError::Elem { what: std::any::type_name::<T>(), err }),
            };
        }
        // SAFETY: This is OK because we initialize *all* elements. Hence, we can assume the array
        // as a whole as initialized.
        Ok(Some(unsafe { MaybeUninit::<[T; LEN]>::from(res).assume_init() }))
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
        let mut buf = Vec::new();
        input.read_to_end(&mut buf).map_err(StringError::Read)?;
        String::from_utf8(buf).map(Some).map_err(StringError::FromUtf8)
    }
}

// Pointer-like impls
fast_cgi_bytes_ptr_impl!('a, &'a T);
fast_cgi_bytes_ptr_impl!('a, &'a mut T);
fast_cgi_bytes_ptr_impl!('a, Cow<'a, T>);
fast_cgi_bytes_ptr_impl!(Box<T>);
fast_cgi_bytes_ptr_impl!(Rc<T>);
fast_cgi_bytes_ptr_impl!(Arc<T>);
fast_cgi_bytes_ptr_impl!('a, Ref<'a, T>);
fast_cgi_bytes_ptr_impl!('a, RefMut<'a, T>);
fast_cgi_bytes_ptr_impl!('a, MutexGuard<'a, T>);
fast_cgi_bytes_ptr_impl!('a, RwLockReadGuard<'a, T>);
fast_cgi_bytes_ptr_impl!('a, RwLockWriteGuard<'a, T>);





/***** AUXILLARY *****/
/// Defines a 32-bit number that's either written compact or lengthy.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[allow(non_camel_case_types)]
pub struct u32Compact(pub u32);
impl ToFastCGIBytes for u32Compact {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        if self.0 <= 127 {
            // Simple-length case; it's a 8-bit, <= 127 number (MSB is 0)

            //     unsigned char numB0;  /* numB0  >> 7 == 0 */
            output.write_all(&self.0.to_be_bytes()[3..])
        } else {
            // Expanded-length case; it's a 32-bit length number (MSB is 1)

            //     unsigned char numB3;  /* numB3  >> 7 == 1 */
            //     unsigned char numB2;
            //     unsigned char numB1;
            //     unsigned char numB0;
            let mut res: [u8; 4] = self.0.to_be_bytes();
            res[0] |= 0x80; // Don't forget to mark this is a big byte
            output.write_all(&res)
        }
    }
}
impl FromFastCGIBytes for u32Compact {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        // Parse the number as a 32-bit number - but start at the first byte
        let mut num: [u8; 4] = [0; 4];
        num[0] = escape_none!(u8::from_fcgi_bytes(&mut input)?);
        if num[0] <= 127 {
            // Simple-length case; it's a 8-bit, <= 127 number (MSB is 0)
            Ok(Some(Self(num[0] as u32)))
        } else {
            // Expanded-length case; it's a 32-bit length number (MSB is 1)
            let mut num_i: usize = 1;
            while num_i < 4 {
                let len: usize = input.read(&mut num[num_i..])?;
                if len == 0 {
                    return Ok(None);
                }
                num_i += len;
            }
            // NOTE: Before we return, don't forget to mask the telling MSB, as it's still the
            // MSB (i.e., it's no longer representing 2^7, but rather, 2^31)
            num[0] = num[0] & 0x7F;
            Ok(Some(Self(u32::from_be_bytes(num))))
        }
    }
}
impl From<u32> for u32Compact {
    #[inline]
    fn from(value: u32) -> Self { Self(value) }
}
impl From<u32Compact> for u32 {
    #[inline]
    fn from(value: u32Compact) -> Self { value.0 }
}



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
        u32Compact(name_len).to_fcgi_bytes(&mut output)?;
        //     unsigned char valueLengthB3; /* valueLengthB3 >> 7 == 1 */
        //     unsigned char valueLengthB2;
        //     unsigned char valueLengthB1;
        //     unsigned char valueLengthB0;
        u32Compact(value_len).to_fcgi_bytes(&mut output)?;
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
        let name_len: u32 = escape_none!(u32Compact::from_fcgi_bytes(&mut input).map_err(PairError::Read)?).0;
        #[cfg(feature = "log")]
        log::trace!("Parsed name length: {name_len} bytes");
        //     unsigned char valueLengthB3; /* valueLengthB3 >> 7 == 1 */
        //     unsigned char valueLengthB2;
        //     unsigned char valueLengthB1;
        //     unsigned char valueLengthB0;
        let value_len: u32 = escape_none!(u32Compact::from_fcgi_bytes(&mut input).map_err(PairError::Read)?).0;
        #[cfg(feature = "log")]
        log::trace!("Parsed value length: {name_len} bytes");
        //     unsigned char nameData[nameLength
        //             ((B3 & 0x7f) << 24) + (B2 << 16) + (B1 << 8) + B0];
        let name = escape_none!(N::from_fcgi_bytes(FixedLenReader::new(name_len as usize, &mut input)).map_err(PairError::Name)?);
        //     unsigned char valueData[valueLength
        //             ((B3 & 0x7f) << 24) + (B2 << 16) + (B1 << 8) + B0];
        let value = escape_none!(V::from_fcgi_bytes(FixedLenReader::new(value_len as usize, &mut input)).map_err(PairError::Value)?);

        Ok(Some(Self { name, value }))
    }
}



/// Defines the possible contents of the FastCFG records.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum RecordBody<'a> {
    /// Start of a request.
    ///
    /// Value: `0x01` (`FCGI_BEGIN_REQUEST`)
    BeginRequest(RecordBeginRequest),
    /// Dirty exit of a request.
    ///
    /// Value: `0x02` (`FCGI_ABORT_REQUEST`)
    AbortRequest(RecordAbortRequest),
    /// Clean exit of a request.
    ///
    /// Value: `0x03` (`FCGI_END_REQUEST`)
    EndRequest(RecordEndRequest),
    /// Send parameters to the binary.
    ///
    /// Value: `0x04` (`FCGI_PARAMS`)
    Params(RecordParams),
    /// Message to stream stdin bytes to the application.
    ///
    /// Value: `0x05` (`FCGI_STDIN`)
    Stdin(RecordStdin),
    /// Message to stream stdout bytes back to the server.
    ///
    /// Value: `0x06` (`FCGI_STDOUT`)
    Stdout(RecordStdout),
    /// Message to stream stderr bytes back to the server.
    ///
    /// Value: `0x07` (`FCGI_STDERR`)
    Stderr(RecordStderr),
    /// TODO
    ///
    /// Value: `0x08` (`FCGI_DATA`)
    Data(RecordData),
    /// TODO
    ///
    /// Value: `0x09` (`FCGI_GET_VALUES`)
    GetValues(RecordGetValues<'a>),
    /// TODO
    ///
    /// Value: `0x0A` (`FCGI_GET_VALUES_RESULT`)
    GetValuesResult(RecordGetValuesResult<'a>),
    /// Leftover type we serialize to if we don't know.
    ///
    /// Value: `0x0B` (`FCGI_UNKNOWN_TYPE`)
    UnknownType(RecordUnknownType),
}
impl<'a> ToFastCGIBytes for RecordBody<'a> {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> {
        match self {
            Self::BeginRequest(r) => r.to_fcgi_bytes(output),
            Self::AbortRequest(r) => r.to_fcgi_bytes(output),
            Self::EndRequest(r) => r.to_fcgi_bytes(output),
            Self::Params(r) => r.to_fcgi_bytes(output),
            Self::Stdin(r) => r.to_fcgi_bytes(output),
            Self::Stdout(r) => r.to_fcgi_bytes(output),
            Self::Stderr(r) => r.to_fcgi_bytes(output),
            Self::Data(r) => r.to_fcgi_bytes(output),
            Self::GetValues(r) => r.to_fcgi_bytes(output),
            Self::GetValuesResult(r) => r.to_fcgi_bytes(output),
            Self::UnknownType(r) => r.to_fcgi_bytes(output),
        }
    }
}
impl<'a> RecordBody<'a> {
    /// Like [`ToFCGIBytes::to_fcgi_bytes()`], but then for the type-byte only.
    #[inline]
    fn ty_to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        output.write_all(std::slice::from_ref(match self {
            Self::BeginRequest(_) => &0x01,
            Self::AbortRequest(_) => &0x02,
            Self::EndRequest(_) => &0x03,
            Self::Params(_) => &0x04,
            Self::Stdin(_) => &0x05,
            Self::Stdout(_) => &0x06,
            Self::Stderr(_) => &0x07,
            Self::Data(_) => &0x08,
            Self::GetValues(_) => &0x09,
            Self::GetValuesResult(_) => &0x0A,
            Self::UnknownType(_) => &0x0B,
        }))
    }
}
impl RecordBody<'static> {
    /// Like [`FromFCGIBytes::from_fcgi_bytes()`], but with type knowledge.
    #[inline]
    fn from_fcgi_bytes_and_ty<R: Read>(ty: u8, input: R) -> Result<Option<Self>, RecordBodyError> {
        // Read a byte
        match ty {
            0x01 => Ok(RecordBeginRequest::from_fcgi_bytes(input).map_err(RecordBodyError::BeginRequest)?.map(RecordBody::BeginRequest)),
            0x02 => Ok(RecordAbortRequest::from_fcgi_bytes(input).map_err(RecordBodyError::AbortRequest)?.map(RecordBody::AbortRequest)),
            0x03 => Ok(RecordEndRequest::from_fcgi_bytes(input).map_err(RecordBodyError::EndRequest)?.map(RecordBody::EndRequest)),
            0x04 => Ok(RecordParams::from_fcgi_bytes(input).map_err(RecordBodyError::Params)?.map(RecordBody::Params)),
            0x05 => Ok(RecordStdin::from_fcgi_bytes(input).map_err(RecordBodyError::Stdin)?.map(RecordBody::Stdin)),
            0x06 => Ok(RecordStdout::from_fcgi_bytes(input).map_err(RecordBodyError::Stdout)?.map(RecordBody::Stdout)),
            0x07 => Ok(RecordStderr::from_fcgi_bytes(input).map_err(RecordBodyError::Stderr)?.map(RecordBody::Stderr)),
            0x08 => Ok(RecordData::from_fcgi_bytes(input).map_err(RecordBodyError::Data)?.map(RecordBody::Data)),
            0x09 => Ok(RecordGetValues::from_fcgi_bytes(input).map_err(RecordBodyError::GetValues)?.map(RecordBody::GetValues)),
            0x0A => Ok(RecordGetValuesResult::from_fcgi_bytes(input).map_err(RecordBodyError::GetValuesResult)?.map(RecordBody::GetValuesResult)),
            /* 0x0B */
            ty => Ok(RecordUnknownType::from_fcgi_bytes(input).map_err(RecordBodyError::UnknownType)?.map(RecordBody::UnknownType)),
        }
    }
}

/// Defines the body of [`RecordBody::BeginRequest`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordBeginRequest {}
impl ToFastCGIBytes for RecordBeginRequest {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { todo!() }
}
impl FromFastCGIBytes for RecordBeginRequest {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> { todo!() }
}

/// Defines the body of [`RecordBody::AbortRequest`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordAbortRequest {}
impl ToFastCGIBytes for RecordAbortRequest {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { todo!() }
}
impl FromFastCGIBytes for RecordAbortRequest {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> { todo!() }
}

/// Defines the body of [`RecordBody::EndRequest`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordEndRequest {}
impl ToFastCGIBytes for RecordEndRequest {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { todo!() }
}
impl FromFastCGIBytes for RecordEndRequest {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> { todo!() }
}

/// Defines the body of [`RecordBody::Params`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordParams {}
impl ToFastCGIBytes for RecordParams {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { todo!() }
}
impl FromFastCGIBytes for RecordParams {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> { todo!() }
}

/// Defines the body of [`RecordBody::Stdin`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordStdin {}
impl ToFastCGIBytes for RecordStdin {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { todo!() }
}
impl FromFastCGIBytes for RecordStdin {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> { todo!() }
}

/// Defines the body of [`RecordBody::Stdout`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordStdout {}
impl ToFastCGIBytes for RecordStdout {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { todo!() }
}
impl FromFastCGIBytes for RecordStdout {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> { todo!() }
}

/// Defines the body of [`RecordBody::Stderr`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordStderr {}
impl ToFastCGIBytes for RecordStderr {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { todo!() }
}
impl FromFastCGIBytes for RecordStderr {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> { todo!() }
}

/// Defines the body of [`RecordBody::Data`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordData {}
impl ToFastCGIBytes for RecordData {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, output: W) -> Result<(), std::io::Error> { todo!() }
}
impl FromFastCGIBytes for RecordData {
    type Error = std::io::Error;

    #[inline]
    fn from_fcgi_bytes<R: Read>(input: R) -> Result<Option<Self>, Self::Error> { todo!() }
}

/// Defines the body of [`RecordBody::GetValues`].
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RecordGetValues<'a> {
    /// A list of parameters to send.
    pub params: Vec<Cow<'a, str>>,
}
impl<'a> ToFastCGIBytes for RecordGetValues<'a> {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        for p in &self.params {
            // Serialize as a pair
            let pair: Pair<&Cow<'a, str>, ()> = Pair { name: p, value: () };
            pair.to_fcgi_bytes(&mut output)?;
        }
        Ok(())
    }
}
impl FromFastCGIBytes for RecordGetValues<'static> {
    type Error = PairError<StringError, Infallible>;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        // Keep parsing pairs until we reach end-of-file
        let mut params = Vec::<Cow<'static, str>>::new();
        loop {
            match Pair::<Cow<'static, str>, ()>::from_fcgi_bytes(&mut input) {
                Ok(Some(p)) => params.push(p.name),
                Ok(None) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(Some(Self { params }))
    }
}

/// Defines the body of [`RecordBody::GetValuesResult`].
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RecordGetValuesResult<'a> {
    /// A list of parameters retrieved.
    pub params: Vec<Pair<Cow<'a, str>, Cow<'a, str>>>,
}
impl<'a> ToFastCGIBytes for RecordGetValuesResult<'a> {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        for p in &self.params {
            p.to_fcgi_bytes(&mut output)?;
        }
        Ok(())
    }
}
impl FromFastCGIBytes for RecordGetValuesResult<'static> {
    type Error = PairError<StringError, StringError>;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        // Keep parsing pairs until we reach end-of-file
        let mut params = Vec::<Pair<Cow<'static, str>, Cow<'static, str>>>::new();
        loop {
            match Pair::<Cow<'static, str>, Cow<'static, str>>::from_fcgi_bytes(&mut input) {
                Ok(Some(p)) => params.push(p),
                Ok(None) => break,
                Err(err) => return Err(err),
            }
        }
        Ok(Some(Self { params }))
    }
}

/// Defines the body of [`RecordBody::UnknownType`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordUnknownType {
    pub ty: u8,
    pub reserved: Option<[u8; 7]>,
}
impl ToFastCGIBytes for RecordUnknownType {
    #[inline]
    fn to_fcgi_bytes<W: Write>(&self, mut output: W) -> Result<(), std::io::Error> {
        self.ty.to_fcgi_bytes(&mut output)?;
        if let Some(res) = &self.reserved { res.to_fcgi_bytes(output) } else { [0u8; 7].to_fcgi_bytes(output) }
    }
}
impl FromFastCGIBytes for RecordUnknownType {
    type Error = RecordUnknownTypeError;

    #[inline]
    fn from_fcgi_bytes<R: Read>(mut input: R) -> Result<Option<Self>, Self::Error> {
        let ty: u8 = escape_none!(u8::from_fcgi_bytes(&mut input)?);
        let reserved: [u8; 7] = escape_none!(<[u8; 7]>::from_fcgi_bytes(&mut input)?);
        Ok(Some(Self { ty, reserved: Some(reserved) }))
    }
}



/// Defines the main FastCGI record.
///
/// # Generics
/// - `C`: The type of the content. You can replace this with something implementing
///   [`FastCGIBytes`] to assume/enforce an encoding.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Record<'a> {
    /// The version number of the record.
    pub version: Version,
    /// The request/stream ID.
    pub request_id: u16,
    /// The amount of padding that was applied when sending this record.
    pub padding_length: Option<u8>,
    /// The reserved-byte from the header.
    pub reserved: Option<u8>,
    /// The content, potentially something parsed already.
    pub content: RecordBody<'a>,
}
impl<'a> Record<'a> {
    /// Constructor for a [`Record`] such that it becomes a `FCGI_GET_VALUES` record.
    #[inline]
    pub fn new_get_values_record(params: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            version: Version::One,
            request_id: 0,
            padding_length: None,
            reserved: None,
            content: RecordBody::GetValues(RecordGetValues { params: params.into_iter().map(Cow::Borrowed).collect() }),
        }
    }
}
impl<'a> ToFastCGIBytes for Record<'a> {
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
        self.content.ty_to_fcgi_bytes(&mut output)?;
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
impl FromFastCGIBytes for Record<'static> {
    type Error = RecordError;

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
        let ty = escape_none!(u8::from_fcgi_bytes(&mut input).map_err(RecordError::Read)?);
        #[cfg(feature = "log")]
        log::trace!("Parsed type: {ty:?}");
        //     unsigned char requestIdB1;
        //     unsigned char requestIdB0;
        let request_id = escape_none!(u16::from_fcgi_bytes(&mut input).map_err(RecordError::u16)?);
        #[cfg(feature = "log")]
        log::trace!("Parsed request ID: {request_id:?}");
        //     unsigned char contentLengthB1;
        //     unsigned char contentLengthB0;
        let content_len = escape_none!(u16::from_fcgi_bytes(&mut input).map_err(RecordError::u16)?);
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
        let content =
            escape_none!(RecordBody::from_fcgi_bytes_and_ty(ty, FixedLenReader::new(content_len as usize, &mut input)).map_err(RecordError::Body)?);
        //     unsigned char paddingData[paddingLength];
        // NOTE: We just pop this
        for _ in 0..padding_length {
            if u8::from_fcgi_bytes(&mut input).map_err(RecordError::Read)?.is_none() {
                return Ok(None);
            }
        }

        Ok(Some(Self { version, request_id, padding_length: Some(padding_length), reserved: Some(reserved), content }))
    }
}





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
        assert_to_fcgi_bytes::<Record<'static>>();
    }

    #[test]
    fn test_string_to_fcgi_bytes() {
        assert_eq!(vectorize(String::new()), b"");
        assert_eq!(vectorize(String::from("Hello, world!")), b"Hello, world!");
    }
    #[test]
    fn test_string_from_fcgi_bytes() {
        assert_eq!(devectorize(b""), Some(String::new()));
        assert_eq!(devectorize(b"Hello, world!"), Some(String::from("Hello, world!")));
        assert_eq!(devectorize(b"Hello\0, world!"), Some(String::from("Hello\0, world!")));
    }

    #[test]
    fn test_pair_to_fcgi_bytes() {
        assert_eq!(vectorize(Pair { name: String::new(), value: () }), b"\0\0");
        assert_eq!(vectorize(Pair { name: String::from("foo"), value: String::from("bar") }), b"\x03\x03foobar");
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
            b"\x80\0\x02\xE7\x03Did you ever hear the tragedy of Darth Plagueis The Wise? I thought not. It's not a story the Jedi would tell you. It's a Sith legend. Darth Plagueis was a Dark Lord of the Sith, so powerful and so wise he could use the Force to influence the midichlorians to create life... He had such a knowledge of the dark side that he could even keep the ones he cared about from dying. The dark side of the Force is a pathway to many abilities some consider to be unnatural. He became so powerful... the only thing he was afraid of was losing his power, which eventually, of course, he did. Unfortunately, he taught his apprentice everything he knew, then his apprentice killed him in his sleep. Ironic. He could save others from death, but not himself.bar"
        );
    }
    #[test]
    fn test_pair_from_fcgi_bytes() {
        assert_eq!(devectorize(b"\0\0"), Some(Pair { name: String::new(), value: () }));
        assert_eq!(devectorize(b"\x03\x03foobar"), Some(Pair { name: String::from("foo"), value: String::from("bar") }));
        assert_eq!(
            devectorize(b"\x80\0\x02\xE7\x03Did you ever hear the tragedy of Darth Plagueis The Wise? I thought not. It's not a story the Jedi would tell you. It's a Sith legend. Darth Plagueis was a Dark Lord of the Sith, so powerful and so wise he could use the Force to influence the midichlorians to create life... He had such a knowledge of the dark side that he could even keep the ones he cared about from dying. The dark side of the Force is a pathway to many abilities some consider to be unnatural. He became so powerful... the only thing he was afraid of was losing his power, which eventually, of course, he did. Unfortunately, he taught his apprentice everything he knew, then his apprentice killed him in his sleep. Ironic. He could save others from death, but not himself.bar"), Some(Pair {
                name:  String::from(
                    "Did you ever hear the tragedy of Darth Plagueis The Wise? I thought not. It's not a story the Jedi would tell you. It's a Sith \
                     legend. Darth Plagueis was a Dark Lord of the Sith, so powerful and so wise he could use the Force to influence the \
                     midichlorians to create life... He had such a knowledge of the dark side that he could even keep the ones he cared about from \
                     dying. The dark side of the Force is a pathway to many abilities some consider to be unnatural. He became so powerful... the \
                     only thing he was afraid of was losing his power, which eventually, of course, he did. Unfortunately, he taught his apprentice \
                     everything he knew, then his apprentice killed him in his sleep. Ironic. He could save others from death, but not himself."
                ),
                value: String::from("bar"),
            })
        );
    }

    #[test]
    fn test_record_to_fcgi_bytes() {
        assert_eq!(
            vectorize(Record {
                version: Version::One,
                request_id: 0,
                padding_length: None,
                reserved: None,
                content: RecordBody::GetValues(RecordGetValues {
                    params: vec![Cow::Borrowed("FCGI_MAX_CONNS"), Cow::Borrowed("FCGI_MAX_REQS"), Cow::Borrowed("FCGI_MPXS_CONNS")],
                }),
            }),
            b"\x01\x09\0\0\0\x30\0\0\x0e\0FCGI_MAX_CONNS\x0d\0FCGI_MAX_REQS\x0f\0FCGI_MPXS_CONNS"
        );
    }
    #[test]
    fn test_record_from_fcgi_bytes() {
        assert_eq!(
            devectorize(b"\x01\x09\0\0\0\x30\x02\0\x0e\0FCGI_MAX_CONNS\x0d\0FCGI_MAX_REQS\x0f\0FCGI_MPXS_CONNS\0\0"),
            Some(Record {
                version: Version::One,
                request_id: 0,
                padding_length: Some(2),
                reserved: Some(0),
                content: RecordBody::GetValues(RecordGetValues {
                    params: vec![Cow::Owned("FCGI_MAX_CONNS".into()), Cow::Owned("FCGI_MAX_REQS".into()), Cow::Owned("FCGI_MPXS_CONNS".into())],
                }),
            },)
        );
    }
}
