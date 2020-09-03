#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(alloc_error_handler)]
#![feature(panic_info_message)]

use ckb_std::{
    ckb_constants::Source,
    ckb_types::prelude::*,
    debug, default_alloc, entry,
    error::SysError,
    high_level::{load_cell_type_hash, load_script},
};
use core::result::Result;

entry!(entry);
default_alloc!();

/// Program entry
fn entry() -> i8 {
    // Call main function and return error code
    match main() {
        Ok(_) => 0,
        Err(err) => err as i8,
    }
}

/// Error
#[repr(i8)]
enum Error {
    IndexOutOfBound = 1,
    ItemMissing,
    LengthNotEnough,
    Encoding,
    // Add customized errors here...
    UnlockFailed,
}

impl From<SysError> for Error {
    fn from(err: SysError) -> Self {
        use SysError::*;
        match err {
            IndexOutOfBound => Self::IndexOutOfBound,
            ItemMissing => Self::ItemMissing,
            LengthNotEnough(_) => Self::LengthNotEnough,
            Encoding => Self::Encoding,
            Unknown(err_code) => panic!("unexpected sys error {}", err_code),
        }
    }
}

fn main() -> Result<(), Error> {
    // remove below examples and write your code here

    let script = load_script()?;
    let args = script.args().as_bytes();
    debug!("script args is {:?}", args);

    // check the script arg is equal with the first output typescript hash
    // in this way, we can delegate the verify logic to the typescript
    let script_hash = load_cell_type_hash(0, Source::Output)?.unwrap();
    if args.as_ref() != &script_hash {
        return Err(Error::UnlockFailed);
    }

    Ok(())
}
