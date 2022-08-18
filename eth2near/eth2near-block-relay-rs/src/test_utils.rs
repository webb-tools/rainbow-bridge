use crate::config::Config;
use crate::eth2near_relay::Eth2NearRelay;
use crate::init_contract::init_contract;
use crate::test_utils;
use contract_wrapper::eth_client_contract::EthClientContract;
use contract_wrapper::eth_client_contract_trait::EthClientContractTrait;
use contract_wrapper::sandbox_contract_wrapper::SandboxContractWrapper;
use eth_types::eth2::{ExtendedBeaconBlockHeader, LightClientUpdate, SyncCommittee};
use eth_types::BlockHeader;
use near_units::*;
use std::{thread, time};
use tokio::runtime::Runtime;
use workspaces::prelude::*;
use workspaces::{network::Sandbox, Account, Contract, Worker};

pub fn read_json_file_from_data_dir(file_name: &str) -> std::string::String {
    let mut json_file_path = std::env::current_exe().unwrap();
    json_file_path.pop();
    json_file_path.push("../../../data");
    json_file_path.push(file_name);

    std::fs::read_to_string(json_file_path).expect("Unable to read file")
}

pub fn init_contract_from_files(eth_client_contract: &mut EthClientContract) {
    const PATH_TO_CURRENT_SYNC_COMMITTEE: &str =
        "../contract_wrapper/data/next_sync_committee_133.json";
    const PATH_TO_NEXT_SYNC_COMMITTEE: &str =
        "../contract_wrapper/data/next_sync_committee_134.json";
    const NETWORK: &str = "kiln";
    const PATH_TO_EXECUTION_BLOCKS: &str =
        "../contract_wrapper/data/execution_block_headers_kiln_1099394-1099937.json";
    const PATH_TO_LIGHT_CLIENT_UPDATES: &str =
        "../contract_wrapper/data/light_client_updates_kiln_1099394-1099937.json";

    let execution_blocks: Vec<BlockHeader> = serde_json::from_str(
        &std::fs::read_to_string(PATH_TO_EXECUTION_BLOCKS).expect("Unable to read file"),
    )
    .unwrap();

    let light_client_updates: Vec<LightClientUpdate> = serde_json::from_str(
        &std::fs::read_to_string(PATH_TO_LIGHT_CLIENT_UPDATES).expect("Unable to read file"),
    )
    .unwrap();

    let current_sync_committee: SyncCommittee = serde_json::from_str(
        &std::fs::read_to_string(PATH_TO_CURRENT_SYNC_COMMITTEE).expect("Unable to read file"),
    )
    .unwrap();
    let next_sync_committee: SyncCommittee = serde_json::from_str(
        &std::fs::read_to_string(PATH_TO_NEXT_SYNC_COMMITTEE).expect("Unable to read file"),
    )
    .unwrap();

    let finalized_beacon_header = ExtendedBeaconBlockHeader::from(
        light_client_updates[0]
            .clone()
            .finality_update
            .header_update,
    );

    let finalized_hash = light_client_updates[0]
        .clone()
        .finality_update
        .header_update
        .execution_block_hash;
    let mut finalized_execution_header = None::<BlockHeader>;
    for header in &execution_blocks {
        if header.hash.unwrap() == finalized_hash {
            finalized_execution_header = Some(header.clone());
            break;
        }
    }

    eth_client_contract.init_contract(
        NETWORK.to_string(),
        finalized_execution_header.unwrap(),
        finalized_beacon_header,
        current_sync_committee,
        next_sync_committee,
    );
    thread::sleep(time::Duration::from_secs(30));
}

const WASM_FILEPATH: &str = "../../contracts/near/res/eth2_client.wasm";

fn create_contract() -> (Account, Contract, Worker<Sandbox>) {
    let rt = Runtime::new().unwrap();

    let worker = rt.block_on(workspaces::sandbox()).unwrap();
    let wasm = std::fs::read(WASM_FILEPATH).unwrap();
    let contract = rt.block_on(worker.dev_deploy(&wasm)).unwrap();

    // create accounts
    let owner = worker.root_account().unwrap();
    let relay_account = rt
        .block_on(
            owner
                .create_subaccount(&worker, "relay_account")
                .initial_balance(parse_near!("30 N"))
                .transact(),
        )
        .unwrap()
        .into_result()
        .unwrap();

    (relay_account, contract, worker)
}

fn get_config() -> Config {
    Config {
        beacon_endpoint: "https://lodestar-kiln.chainsafe.io".to_string(),
        eth1_endpoint: "https://rpc.kiln.themerge.dev".to_string(),
        total_submit_headers: 8,
        near_endpoint: "NaN".to_string(),
        signer_account_id: "NaN".to_string(),
        path_to_signer_secret_key: "NaN".to_string(),
        contract_account_id: "NaN".to_string(),
        network: "kiln".to_string(),
        contract_type: "near".to_string(),
        light_client_updates_submission_frequency_in_epochs: 1,
        max_blocks_for_finalization: 5000,
        near_network_id: "testnet".to_string(),
        dao_contract_account_id: None,
        output_dir: None,
    }
}

pub fn get_client_contract(from_file: bool) -> Box<dyn EthClientContractTrait> {
    let (relay_account, contract, worker) = create_contract();
    let contract_wrapper = Box::new(SandboxContractWrapper::new(relay_account, contract, worker));
    let mut eth_client_contract = EthClientContract::new(contract_wrapper);

    let config = get_config();
    match from_file {
        true => test_utils::init_contract_from_files(&mut eth_client_contract),
        false => init_contract(&config, &mut eth_client_contract).unwrap(),
    };

    Box::new(eth_client_contract)
}

pub fn get_relay(enable_binsearch: bool, from_file: bool) -> Eth2NearRelay {
    let config = get_config();
    Eth2NearRelay::init(
        &config,
        get_client_contract(from_file),
        enable_binsearch,
        true,
    )
}
