//  FAST CGI.rs
//    by Lut99
//
//  Description:
//!   Defines the FastCGI stream type, which hijacks a stream to do FastCGI
//!   communication.
//

use std::collections::HashMap;

use parking_lot::Mutex;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::wire::{RecordHeader, RecordTy, Version, read_u32_compact, write_u32_compact};


/***** ERRORS *****/
#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to write on connection")]
    Write(#[source] std::io::Error),
    #[error("Failed to read from connection")]
    Read(#[source] std::io::Error),

    #[error("Overflowed content length (got {got} bytes, max {} bytes)", u16::MAX)]
    ContentLenOverflow { got: usize },
    #[error("Overflowed parameter length for parameter {i} (got {got} bytes, max {} bytes)", u32::MAX)]
    ParamLenOverflow { i: usize, got: usize },
    #[error("Input bytes were no valid record header")]
    RecordHeader(#[source] crate::wire::RecordHeaderError),
    #[error("Got unexpected record in reply (got {got:?}, expected {expected:?})")]
    UnexpectedRecord { got: RecordTy, expected: RecordTy },
    #[error("Got record with unexpected request ID in reply (got {got:?}, expected {expected:?})")]
    UnexpectedRequestId { got: u16, expected: u16 },
    #[error("Missing name length while reading a name/value pair")]
    MissingNameLen,
    #[error("Missing value length while reading a name/value pair")]
    MissingValueLen,
    #[error("Expected valid UTF-8")]
    Utf8(std::string::FromUtf8Error),
}





/***** LIBRARY *****/
/// Wrap any [`AsyncRead`]/[`AsyncWrite`]-stream to start communicating over it using FastCGI.
#[derive(Debug)]
pub struct FastCGI<S>(Mutex<S>);

// Constructors
impl<S> FastCGI<S> {
    /// Constructor for the FastCGI that wraps a stream to enable FastCGI-style communication over
    /// it.
    ///
    /// This is only really useful if your type implements [`AsyncRead`] and [`AsyncWrite`].
    ///
    /// # Returns
    /// A new FastCGI that can do FastCGI.
    #[inline]
    pub const fn new(stream: S) -> Self { Self(Mutex::new(stream)) }
}

// Management
impl<S: AsyncRead + AsyncWrite + Unpin> FastCGI<S> {
    /// Sends an `FCGI_GET_VALUES` record to the application to read some of its parameter values.
    ///
    /// Typically, applications support at least the following three:
    /// [`PARAM_MAX_CONNS`](crate::spec::PARAM_MAX_CONNS),
    /// [`PARAM_MAX_REQS`](crate::spec::PARAM_MAX_REQS) and
    /// [`PARAM_MPXS_CONNS`](crate::spec::PARAM_MPXS_CONNS). However, applications may also define
    /// their own.
    ///
    /// # Arguments
    /// - `params`: Something iterable yielding the names of the parameters to try.
    ///
    /// # Returns
    /// A [`HashMap`] describing the values for each of the given parameters.
    ///
    /// Note that not all input parameters are necessarily present in the output; if the
    /// application wasn't aware of any of them, it omits the value from the response.
    ///
    /// # Errors
    /// This function can error if the stream fails or if the application did not respond with a
    /// record.
    #[inline]
    pub async fn get_values<'p>(&self, params: impl IntoIterator<Item = &'p str>) -> Result<HashMap<String, String>, Error> {
        let mut conn = self.0.lock();

        // Pre-compute the record length
        let params: Vec<&'p str> = params.into_iter().collect();
        let content_len: usize = params.iter().map(|p| if p.len() <= 127 { 2 + p.len() } else { 5 + p.len() }).sum();
        if content_len > u16::MAX as usize {
            return Err(Error::ContentLenOverflow { got: content_len });
        }
        let content_len: u16 = content_len as u16;
        #[cfg(feature = "log")]
        log::trace!("Predicted content length: {content_len} bytes");

        // Construct the header and write it
        let header = RecordHeader { version: Version::One, ty: RecordTy::GetValues, request_id: 0, content_len, padding_len: 0, reserved: 0 }
            .with_auto_padding();
        let mut header: [u8; 8] = header.into();
        #[cfg(feature = "log")]
        log::trace!("Serialized request header: {header:?}");
        conn.write_all(&header).await.map_err(Error::Write)?;

        // Write the parameter pairs
        for (i, p) in params.into_iter().enumerate() {
            // Write the lengths first (length of the name + empty for value)
            if p.len() > u32::MAX as usize {
                return Err(Error::ParamLenOverflow { i, got: p.len() });
            }
            write_u32_compact(p.len() as u32, &mut *conn).await.map_err(Error::Write)?;
            conn.write_all(&[0x00]).await.map_err(Error::Write)?;

            // Then write the raw name bytes
            conn.write_all(p.as_bytes()).await.map_err(Error::Write)?;
        }
        conn.flush().await.map_err(Error::Write)?;
        #[cfg(feature = "log")]
        log::trace!("Wrote entire record");

        // OK, now await a record header back
        #[cfg(feature = "log")]
        log::trace!("Awaiting reply...");
        conn.read_exact(&mut header).await.map_err(Error::Read)?;
        #[cfg(feature = "log")]
        log::trace!("Reply header: {header:?}");
        let header: RecordHeader = header.try_into().map_err(Error::RecordHeader)?;
        #[cfg(feature = "log")]
        log::trace!("Deserialized reply header: {header:?}");
        if header.ty != RecordTy::GetValuesResult {
            return Err(Error::UnexpectedRecord { got: header.ty, expected: RecordTy::GetValuesResult });
        }
        if header.request_id != 0x00 {
            return Err(Error::UnexpectedRequestId { got: header.request_id, expected: 0x00 });
        }

        // Read the rest of the content as pairs
        let mut read: usize = 0;
        let mut res: HashMap<String, String> = HashMap::new();
        loop {
            if read == header.content_len as usize {
                break;
            } else if read > header.content_len as usize {
                panic!("Read bytes exceeded content length");
            }

            // Pop two compacted length versions
            let name_len: u32 = match read_u32_compact(&mut *conn).await.map_err(Error::Read)? {
                Some(len) => len,
                None => return Err(Error::MissingNameLen),
            };
            let value_len: u32 = match read_u32_compact(&mut *conn).await.map_err(Error::Read)? {
                Some(len) => len,
                None => return Err(Error::MissingValueLen),
            };

            // Pop the name & value as strings
            let mut buf: Vec<u8> = vec![0; name_len as usize];
            conn.read_exact(&mut buf).await.map_err(Error::Read)?;
            let name = String::from_utf8(buf).map_err(Error::Utf8)?;

            let mut buf: Vec<u8> = vec![0; value_len as usize];
            conn.read_exact(&mut buf).await.map_err(Error::Read)?;
            let value = String::from_utf8(buf).map_err(Error::Utf8)?;

            // Push 'em
            #[cfg(feature = "log")]
            log::trace!("Parsed name/value pair {name:?}/{value:?} ({name_len} bytes/{value_len} bytes)");
            res.insert(name, value);
            read += if name_len <= 127 { 1 } else { 4 } + if value_len <= 127 { 1 } else { 4 } + name_len as usize + value_len as usize;
        }
        #[cfg(feature = "log")]
        log::trace!("Read {read} content bytes");

        // Pop the padding before we're done
        for _ in 0..header.padding_len {
            conn.read_u8().await.map_err(Error::Read)?;
        }
        #[cfg(feature = "log")]
        log::trace!("Popped {} bytes padding", header.padding_len);

        // Done!
        Ok(res)
    }
}
