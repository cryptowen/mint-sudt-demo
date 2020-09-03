#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(alloc_error_handler)]
#![feature(panic_info_message)]

mod types;

use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    debug, default_alloc, entry,
    error::SysError,
    high_level::{
        load_cell_capacity, load_cell_data, load_cell_lock, load_cell_lock_hash, load_cell_type,
        load_cell_type_hash, load_input_out_point, load_script, load_script_hash,
        load_witness_args, QueryIter,
    },
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
    InvalidTx,
    InvalidID,
    InvalidInitOutputData,
    WitnessMissInputType,
    InvalidWitnessEncoding,
    InvalidOutput,
    InvalidToLockHash,
    InvalidMintAmount,
    InvalidInputData,
    InvalidOutputData,
    RecordHandled,
    RecordsNotMatch,
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

const SUDT_CODE_HASH: [u8; 32] = [
    225, 227, 84, 214, 214, 67, 173, 66, 114, 77, 64, 150, 126, 51, 73, 132, 83, 78, 3, 103, 64,
    92, 90, 228, 42, 157, 125, 99, 215, 125, 244, 25,
];
const LOCKSCRIPT_CODE_HASH: [u8; 32] = [
    225, 227, 84, 214, 214, 67, 173, 66, 114, 77, 64, 150, 126, 51, 73, 132, 83, 78, 3, 103, 64,
    92, 90, 228, 42, 157, 125, 99, 215, 125, 244, 25,
];

fn verify_init() -> Result<(), Error> {
    // when create this cell, use the first outpoint as the args of the typescript,
    // avoid someone creating an exactly same cell because the outpoint can not be used as input for twice.
    let first_outpoint = load_input_out_point(0, Source::Input)?;
    let script = load_script()?;
    let args = script.args();
    if args.as_slice() != first_outpoint.as_slice() {
        return Err(Error::InvalidID);
    }
    // verify output data is well initialized
    let output_data = load_cell_data(0, Source::Output)?;
    let records = types::Records::default();
    if records.as_slice() != &output_data[..] {
        return Err(Error::InvalidInitOutputData);
    }
    Ok(())
}

fn verify_proof(_data: &[u8], _witness: &[u8]) -> Result<(), Error> {
    // you can change it to any logic you want to verify the proof
    Ok(())
}

fn verify_transfer() -> Result<(), Error> {
    // load and parse witness
    let witness_args = load_witness_args(0, Source::Input)?.input_type();
    if witness_args.is_none() {
        return Err(Error::WitnessMissInputType);
    }
    let witness_args: Bytes = witness_args.to_opt().unwrap().unpack();
    if types::ETHSpvProofReader::verify(&witness_args, false).is_err() {
        return Err(Error::InvalidWitnessEncoding);
    }
    let proof_reader = types::ETHSpvProofReader::new_unchecked(&witness_args);
    debug!("proof_reader: {:?}", proof_reader);

    // load cell data transfer is valid.
    // The proof in this tx was not handled before, and after this tx, we add the record into the cell data
    let input_data = load_cell_data(0, Source::Input)?;
    if types::CellDataReader::verify(&input_data, false).is_err() {
        return Err(Error::InvalidInputData);
    }
    let input_data_parsed = types::CellDataReader::new_unchecked(&input_data);
    let output_data = load_cell_data(0, Source::Output)?;
    if types::CellDataReader::verify(&output_data, false).is_err() {
        return Err(Error::InvalidOutputData);
    }
    let output_data_parsed = types::CellDataReader::new_unchecked(&output_data);

    // check the record in witness is not handled yet and the new record will be pushed to the records.
    let current_record = proof_reader.record();
    for (i, record) in input_data_parsed.records().iter().enumerate() {
        if current_record.as_slice() == record.as_slice() {
            return Err(Error::RecordHandled);
        }
        if record.as_slice() != output_data_parsed.records().get_unchecked(i).as_slice() {
            return Err(Error::RecordsNotMatch);
        }
    }
    if current_record.as_slice()
        != output_data_parsed
            .records()
            .get_unchecked(input_data_parsed.records().len())
            .as_slice()
    {
        return Err(Error::RecordsNotMatch);
    }

    // verify proof is valid
    verify_proof(&input_data, &witness_args)?;

    // verify mint token amount
    let lock_hash = load_cell_lock_hash(0, Source::GroupOutput)?;
    let ceth_typescript =
        load_cell_type(1, Source::Output)?.expect("should be cETH token typescript");
    if !(ceth_typescript.code_hash().raw_data().as_ref() == SUDT_CODE_HASH.as_ref()
        && ceth_typescript.args().raw_data().as_ref() == lock_hash.as_ref()
        && ceth_typescript.hash_type() == 0u8.into())
    {
        return Err(Error::InvalidOutput);
    }
    let mint_token_lock_hash = load_cell_lock_hash(1, Source::Output)?;
    if mint_token_lock_hash != proof_reader.to_lockscript_hash().raw_data() {
        return Err(Error::InvalidToLockHash);
    }
    let mint_token_data = load_cell_data(1, Source::Output)?;
    if mint_token_data != proof_reader.amount().raw_data() {
        return Err(Error::InvalidMintAmount);
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    // You can only create the cell, and then transfer the state under the verification of this typescript
    let group_output_num = QueryIter::new(load_cell_capacity, Source::GroupOutput).count();
    if group_output_num != 1 {
        return Err(Error::InvalidTx);
    }
    // The first output must be eth-bridge cell whose type is this script, and lock is the accociated lock
    let script_hash = load_script_hash()?;
    let output_typescript_hash =
        load_cell_type_hash(0, Source::Output)?.expect("should be a typescript");
    if script_hash != output_typescript_hash {
        return Err(Error::InvalidOutput);
    }
    let output_lock = load_cell_lock(0, Source::Output)?;
    if !(output_lock.code_hash().raw_data().as_ref() == LOCKSCRIPT_CODE_HASH.as_ref()
        && output_lock.args().raw_data().as_ref() == output_typescript_hash.as_ref()
        && output_lock.hash_type() == 0u8.into())
    {
        return Err(Error::InvalidOutput);
    }
    // check input num to determine whether this is a init or transfer transaction
    let group_input_num = QueryIter::new(load_cell_capacity, Source::GroupInput).count();
    match group_input_num {
        0 => verify_init()?,
        1 => verify_transfer()?,
        _ => return Err(Error::InvalidTx),
    };
    Ok(())
}
