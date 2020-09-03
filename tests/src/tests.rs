use super::*;
use ckb_testtool::{builtin::ALWAYS_SUCCESS, context::Context};
use ckb_tool::ckb_types::{bytes::Bytes, core::TransactionBuilder, packed::*, prelude::*};

const MAX_CYCLES: u64 = 10_000_000;

#[test]
fn test_mint_ceth() {
    // deploy contract
    let mut context = Context::default();
    let typescript_bin: Bytes = Loader::default().load_binary("eth-bridge-typescript");
    let typescript_out_point = context.deploy_cell(typescript_bin);
    let lockscript_bin: Bytes = Loader::default().load_binary("eth-bridge-lockscript");
    let lockscript_out_point = context.deploy_cell(lockscript_bin);
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    let sudt_bin = include_bytes!("../deps/simple_udt");
    let sudt_out_point = context.deploy_cell(Bytes::from(sudt_bin.as_ref()));

    // prepare scripts
    let type_script = context
        .build_script(&typescript_out_point, Default::default())
        .expect("script");
    let typescript_hash = type_script.calc_script_hash();
    let type_script_dep = CellDep::new_builder()
        .out_point(typescript_out_point)
        .build();
    let lock_script = context
        .build_script(&lockscript_out_point, typescript_hash.as_bytes())
        .expect("script");
    let lockscript_hash = lock_script.calc_script_hash();
    let lock_script_dep = CellDep::new_builder()
        .out_point(lockscript_out_point)
        .build();
    let sudt_script = context
        .build_script(&sudt_out_point, Bytes::from(lockscript_hash.as_bytes()))
        .expect("script");
    let sudt_script_dep = CellDep::new_builder().out_point(sudt_out_point).build();
    let always_success_script = context
        .build_script(&always_success_out_point, Default::default())
        .expect("script");
    let always_success_script_hash = always_success_script.calc_script_hash();
    let always_success_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point)
        .build();

    // inputs
    let provide_capacity_cell_input = CellInput::new_builder()
        .previous_output(
            context.create_cell(
                CellOutput::new_builder()
                    .capacity(1000u64.pack())
                    .lock(always_success_script.clone())
                    .build(),
                Bytes::new(),
            ),
        )
        .build();
    let eth_bridge_cell_input_data = types::CellData::default();
    let eth_bridge_cell_input = CellInput::new_builder()
        .previous_output(
            context.create_cell(
                CellOutput::new_builder()
                    .capacity(1000u64.pack())
                    .lock(lock_script.clone())
                    .type_(
                        ScriptOpt::new_builder()
                            .set(Some(type_script.clone()))
                            .build(),
                    )
                    .build(),
                eth_bridge_cell_input_data.as_bytes(),
            ),
        )
        .build();

    // data
    let block_hash = types::Hash::from_slice(&[0u8; 32]).unwrap();
    let tx_index = types::Uint64::from_slice(&[0u8; 8]).unwrap();
    let record = types::Record::new_builder()
        .block_hash(block_hash)
        .tx_index(tx_index)
        .build();
    let raw_amount = 1u128;
    let amount = types::Uint128::from_slice(&&raw_amount.to_le_bytes().to_vec()).unwrap();
    let proof = types::Bytes::default();
    let to_lockscript_hash =
        types::Hash::from_slice(always_success_script_hash.as_slice()).unwrap();
    let eth_spv_proof = types::ETHSpvProof::new_builder()
        .amount(amount)
        .to_lockscript_hash(to_lockscript_hash)
        .record(record.clone())
        .proof(proof)
        .build();

    // outputs
    let mut outputs_data = vec![];
    // the 1st output is eth_bridge_cell
    let eth_bridge_cell_output = CellOutput::new_builder()
        .capacity(1000u64.pack())
        .lock(lock_script.clone())
        .type_(
            ScriptOpt::new_builder()
                .set(Some(type_script.clone()))
                .build(),
        )
        .build();
    let eth_bridge_cell_output_data = types::CellData::new_builder()
        .records(
            types::Records::new_builder()
                .set(vec![record.clone()])
                .build(),
        )
        .build();
    outputs_data.push(eth_bridge_cell_output_data.as_bytes());
    // the 2nd output is cETH cell
    let ceth_token_cell_output = CellOutput::new_builder()
        .capacity(1000u64.pack())
        .lock(always_success_script.clone())
        .type_(
            ScriptOpt::new_builder()
                .set(Some(sudt_script.clone()))
                .build(),
        )
        .build();
    outputs_data.push(Bytes::from(raw_amount.to_le_bytes().to_vec()));

    // witness
    let witness = WitnessArgs::new_builder()
        .input_type(Some(eth_spv_proof.as_bytes()).pack())
        .build();

    // build transaction
    let tx = TransactionBuilder::default()
        .input(eth_bridge_cell_input)
        .input(provide_capacity_cell_input)
        .output(eth_bridge_cell_output)
        .output(ceth_token_cell_output)
        .outputs_data(outputs_data.pack())
        .cell_dep(lock_script_dep)
        .cell_dep(type_script_dep)
        .cell_dep(always_success_script_dep)
        .cell_dep(sudt_script_dep)
        .witness(witness.as_bytes().pack())
        .build();
    // let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}
