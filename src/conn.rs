//  CONNECTION.rs
//    by Lut99
//
//  Description:
//!   Defines functions that handle the physical connection to the FastCGI server.
//

use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs as _};
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;

#[cfg(feature = "log")]
use log::{debug, info};
use thiserror::Error;

use super::FastCGI;
use crate::spec::{FromFastCGIBytes, GetValuesRecord, GetValuesResultRecord, ToFastCGIBytes as _};


/***** ERRORS *****/
/// Defines the errors created by the FastCGI connection creation.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to resolve address {addr:?}")]
    AddrResolve {
        addr: String,
        #[source]
        err:  std::io::Error,
    },
    #[error("Unknown host in address {addr:?}")]
    UnknownHost { addr: String },
    #[error("Failed to connect to address {addr:?}")]
    AddrConnect {
        addr: SocketAddr,
        #[source]
        err:  std::io::Error,
    },
    #[cfg(unix)]
    #[error("Failed to connect to socket {path:?}")]
    UnixConnect {
        path: PathBuf,
        #[source]
        err:  std::io::Error,
    },
    #[error("Parameter {param:?} contained a null-byte")]
    ParamNull { param: String },
    #[error("Failed to write socket to {addr:?}")]
    SocketWrite {
        addr: String,
        #[source]
        err:  std::io::Error,
    },
    #[error("Failed to wait for a GetValuesResponse record from {addr:?}")]
    GetValuesResponse {
        addr: String,
        #[source]
        err:  crate::spec::GetValuesResultRecordError,
    },
}





/***** LIBRARY *****/
/// Represents a single FCGI request.
pub struct Request {}





/***** IMPLS *****/
impl FastCGI {
    /// Constructor for the FastCGI struct that connects to a FastCGI server using a string
    /// hostname.
    ///
    /// If you already have a resolved [`SocketAddr`], consider calling [`FastCGI::connect()`]
    /// instead.
    ///
    /// # Arguments
    /// - `addr`: A `hostname:port`-pair to connect to.
    ///
    /// # Returns
    /// A new FastCGI instance that represents the active connection to the server.
    ///
    /// # Errors
    /// This function can error if making the connection failed.
    #[inline]
    pub fn connect_addr(addr: &str) -> Result<Self, Error> {
        // Resolve the address first
        let saddr: SocketAddr = addr
            .to_socket_addrs()
            .map_err(|err| Error::AddrResolve { addr: addr.into(), err })?
            .next()
            .ok_or_else(|| Error::UnknownHost { addr: addr.into() })?;
        #[cfg(feature = "log")]
        debug!("Resolved {addr:?} as {saddr:?}");

        // Then delegate to the regular function
        Self::connect(saddr)
    }

    /// Constructor for the FastCGI struct that connects to a FastCGI server.
    ///
    /// If you have a hostname instead of address, consider using [`FastCGI::connect_addr()`]
    /// instead.
    ///
    /// # Arguments
    /// - `addr`: A [`SocketAddr`] describing what to connect to.
    ///
    /// # Returns
    /// A new FastCGI instance that represents the active connection to the server.
    ///
    /// # Errors
    /// This function can error if making the connection failed.
    #[inline]
    pub fn connect(addr: SocketAddr) -> Result<Self, Error> {
        // Open a TCP stream to the client
        let conn = TcpStream::connect(addr).map_err(|err| Error::AddrConnect { addr, err })?;
        #[cfg(feature = "log")]
        info!("Connected to {addr:?}");
        Ok(Self { addr: format!("{addr:?}"), conn: Box::new(conn) })
    }

    /// Constructor for the FastCGI struct that connects to a FastCGI server over a Unix socket.
    ///
    /// # Arguments
    /// - `path`: The path to the Unix socket that we connect to.
    ///
    /// # Returns
    /// A new FastCGI instance that represents the active connection to the server.
    ///
    /// # Errors
    /// This function can error if making the connection failed.
    #[cfg(unix)]
    #[inline]
    pub fn connect_unix(path: impl AsRef<Path>) -> Result<Self, Error> {
        use std::os::unix::net::UnixStream;

        // Open a TCP stream to the client
        let path: &Path = path.as_ref();
        let conn = UnixStream::connect(path).map_err(|err| Error::UnixConnect { path: path.into(), err })?;
        #[cfg(feature = "log")]
        info!("Connected to {path:?}");
        Ok(Self { addr: format!("{path:?}"), conn: Box::new(conn) })
    }
}

// FastCGI
impl FastCGI {
    /// Requests the client to list the values of the given parameters.
    ///
    /// Returned is an iterator over the response. Every response is a <string, value> pair, but it
    /// may be incomplete if the application didn't recognize one of your requested parameters.
    ///
    /// To request the default parameters, see [`PARAM_MAX_CONNS`](crate::spec::PARAM_MAX_CONNS),
    /// [`PARAM_MAX_REQS`](crate::spec::PARAM_MAX_REQS) and
    /// [`PARAM_MPXS_CONNS`](crate::spec::PARAM_MPXS_CONNS).
    ///
    /// # Arguments
    /// - `params`: A list of parameter names to request.
    ///
    /// # Returns
    /// A map of returned names with their values. Note that it may be smaller than the requested
    /// set of `params` if the application didn't recognize any of them.
    ///
    /// # Errors
    /// This function may error if the connection or the application fails.
    #[inline]
    pub fn get_values<'p>(&mut self, params: impl IntoIterator<Item = &'p str>) -> Result<HashMap<String, String>, Error> {
        // Create the record to send over the wire
        // NOTE: Ensure the string doesn't contain null-bytes!
        // (We check this here to avoid panics)
        let params: Vec<&'p str> = params
            .into_iter()
            .map(|p| if !p.as_bytes().contains(&b'\0') { Ok(p) } else { Err(Error::ParamNull { param: p.into() }) })
            .collect::<Result<Vec<&'p str>, Error>>()?;

        // Build a record and send it over the wire to the application
        let rec = GetValuesRecord::new_get_values_record(params);
        #[cfg(feature = "log")]
        info!("Sending GetValues request to {:?}...", self.addr);
        if let Err(err) = rec.to_fcgi_bytes(&mut self.conn) {
            return Err(Error::SocketWrite { addr: self.addr.clone(), err });
        }

        // Await it's response
        #[cfg(feature = "log")]
        info!("Receiving GetValuesResult response from {:?}...", self.addr);
        // let res = GetValuesResultRecord::from_fcgi_bytes(&mut self.conn).map_err(|err| Error::GetValuesResponse { addr: self.addr.clone(), err })?;
        // #[cfg(feature = "log")]
        // debug!("{res:?}");
        let mut res = [0u8; 8192];
        let mut res_i: usize = 0;
        while res_i == 0 {
            res_i += std::io::Read::read(&mut self.conn, &mut res).unwrap();
        }
        println!("{:?}", &res[..res_i]);
        let res = GetValuesResultRecord::from_fcgi_bytes(&res[..res_i]).map_err(|err| Error::GetValuesResponse { addr: self.addr.clone(), err })?;
        #[cfg(feature = "log")]
        debug!("{res:?}");
        todo!()
    }



    /// Initializes a new request.
    #[inline]
    pub fn start_request(&mut self) -> Result<Request, Error> { todo!() }
}
