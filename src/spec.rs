//  SPEC.rs
//    by Lut99
//
//  Description:
//!   Defines some spec-defined constants etc.
//


/***** CONSTANTS *****/
/// Defines the name of the parameter defining the maximum number of concurrent transport
/// connections an application supports.
pub const PARAM_MAX_CONNS: &'static str = "FCGI_MAX_CONNS";
/// Defines the name of the parameter defining the maximum number of concurrent requests an
/// application supports.
pub const PARAM_MAX_REQS: &'static str = "FCGI_MAX_REQS";
/// Defines the name of the parameter defining whether an application multiplexes connections.
pub const PARAM_MPXS_CONNS: &'static str = "FCGI_MPXS_CONNS";
