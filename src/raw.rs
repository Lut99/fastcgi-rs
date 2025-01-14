//  RAW.rs
//    by Lut99
//
//  Created:
//    14 Jan 2025, 22:15:17
//  Last edited:
//    14 Jan 2025, 22:19:10
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the raw messages on the wire.
//


/***** LIBRARY *****/
/// Defines the main FastCGI record.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Record {
    version: u8,
    ty: u8,
    request_id: [u8; 2],
    content_length: [u8; 2],
    padding_length: u8,
    reserved: u8,
    content_data: 
}
