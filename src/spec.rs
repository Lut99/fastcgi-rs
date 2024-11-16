//  SPEC.rs
//    by Lut99
//
//  Created:
//    16 Nov 2024, 16:07:22
//  Last edited:
//    16 Nov 2024, 16:26:18
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the message structures used in the protocol.
//

use stackvec::StackVec;


/***** AUXILLARY *****/





/***** DATA TYPES *****/
/// Defines known protocol versions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Version {
    /// The first version of the protocol.
    V1 = 1,
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
