use std::hash::Hash;
use std::ops::Deref;
use std::{path::PathBuf, str::FromStr};

use namada_sdk::borsh::{BorshDeserialize, BorshSerialize, BorshSerializeExt};
use namada_sdk::collections::HashMap;
use namada_sdk::ibc::core::host::types::identifiers::{ChannelId, PortId};
use namada_sdk::ibc::IbcShieldingData;
use namada_sdk::masp::partial_deauthorize;
use namada_sdk::masp_primitives::constants::SPENDING_KEY_GENERATOR;
use namada_sdk::masp_primitives::sapling::redjubjub::PrivateKey;
use namada_sdk::masp_primitives::sapling::spend_sig;
use namada_sdk::masp_primitives::transaction::components::sapling;
use namada_sdk::masp_primitives::transaction::components::sapling::builder::{
    BuildParams, RngBuildParams,
};
use namada_sdk::masp_primitives::transaction::sighash::{signature_hash, SignableInput};
use namada_sdk::masp_primitives::transaction::txid::TxIdDigester;
use namada_sdk::masp_primitives::zip32::PseudoExtendedKey;
use namada_sdk::masp_proofs::group::GroupEncoding;
use namada_sdk::masp_proofs::jubjub;
use namada_sdk::signing::SigningTxData;
use namada_sdk::tx::data::GasLimit;
use namada_sdk::tx::{Section, Tx};
use namada_sdk::{
    address::Address,
    args::{self, InputAmount, TxExpiration},
    chain::ChainId,
    ethereum_events::EthAddress,
    key::common::PublicKey,
    token::{Amount, DenominatedAmount, NATIVE_MAX_DECIMAL_PLACES},
    TransferSource,
};
use namada_sdk::{error, masp_primitives, tendermint_rpc};
use namada_sdk::{ExtendedViewingKey, PaymentAddress};
use rand::rngs::OsRng;
use wasm_bindgen::JsError;

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct RevealPkMsg {
    public_key: String,
}

impl RevealPkMsg {
    pub fn new(public_key: String) -> RevealPkMsg {
        RevealPkMsg { public_key }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct WrapperTxMsg {
    token: String,
    fee_amount: String,
    gas_limit: String,
    chain_id: String,
    public_key: Option<String>,
    memo: Option<String>,
}

impl WrapperTxMsg {
    pub fn new(
        token: String,
        fee_amount: String,
        gas_limit: String,
        chain_id: String,
        public_key: Option<String>,
        memo: Option<String>,
    ) -> WrapperTxMsg {
        WrapperTxMsg {
            token,
            fee_amount,
            gas_limit,
            chain_id,
            public_key,
            memo,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct BondMsg {
    source: String,
    validator: String,
    amount: String,
}

impl BondMsg {
    pub fn new(source: String, validator: String, amount: String) -> BondMsg {
        BondMsg {
            source,
            validator,
            amount,
        }
    }
}

/// Maps serialized tx_msg into BondTx args.
///
/// # Arguments
///
/// * `bond_msg` - Borsh serialized bond_msg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn bond_tx_args(bond_msg: &[u8], tx_msg: &[u8]) -> Result<args::Bond, JsError> {
    let bond_msg = BondMsg::try_from_slice(bond_msg)?;

    let BondMsg {
        source,
        validator,
        amount,
    } = bond_msg;

    let source = Address::from_str(&source)?;
    let validator = Address::from_str(&validator)?;
    let amount = Amount::from_str(&amount, NATIVE_MAX_DECIMAL_PLACES)?;
    let tx = tx_msg_into_args(tx_msg)?;

    let args = args::Bond {
        tx,
        validator,
        amount,
        source: Some(source),
        tx_code_path: PathBuf::from("tx_bond.wasm"),
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct UnbondMsg {
    source: String,
    validator: String,
    amount: String,
}

impl UnbondMsg {
    pub fn new(source: String, validator: String, amount: String) -> UnbondMsg {
        UnbondMsg {
            source,
            validator,
            amount,
        }
    }
}

/// Maps serialized tx_msg into UnbondTx args.
///
/// # Arguments
///
/// * `unbond_msg` - Borsh serialized unbond_msg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn unbond_tx_args(unbond_msg: &[u8], tx_msg: &[u8]) -> Result<args::Unbond, JsError> {
    let unbond_msg = UnbondMsg::try_from_slice(unbond_msg)?;

    let UnbondMsg {
        source,
        validator,
        amount,
    } = unbond_msg;

    let source = Address::from_str(&source)?;
    let validator = Address::from_str(&validator)?;

    let amount = Amount::from_str(&amount, NATIVE_MAX_DECIMAL_PLACES)?;
    let tx = tx_msg_into_args(tx_msg)?;

    let args = args::Unbond {
        tx,
        validator,
        amount,
        source: Some(source),
        tx_code_path: PathBuf::from("tx_unbond.wasm"),
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct WithdrawMsg {
    source: String,
    validator: String,
}

impl WithdrawMsg {
    pub fn new(source: String, validator: String) -> WithdrawMsg {
        WithdrawMsg { source, validator }
    }
}

/// Maps serialized tx_msg into WithdrawTx args.
///
/// # Arguments
///
/// * `withdraw_msg` - Borsh serialized withdraw_msg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn withdraw_tx_args(withdraw_msg: &[u8], tx_msg: &[u8]) -> Result<args::Withdraw, JsError> {
    let withdraw_msg = WithdrawMsg::try_from_slice(withdraw_msg)?;

    let WithdrawMsg { source, validator } = withdraw_msg;

    let source = Address::from_str(&source)?;
    let validator = Address::from_str(&validator)?;
    let tx = tx_msg_into_args(tx_msg)?;

    let args = args::Withdraw {
        tx,
        validator,
        source: Some(source),
        tx_code_path: PathBuf::from("tx_withdraw.wasm"),
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct RedelegateMsg {
    owner: String,
    source_validator: String,
    destination_validator: String,
    amount: String,
}

impl RedelegateMsg {
    pub fn new(
        owner: String,
        source_validator: String,
        destination_validator: String,
        amount: String,
    ) -> RedelegateMsg {
        RedelegateMsg {
            owner,
            source_validator,
            destination_validator,
            amount,
        }
    }
}

/// Maps serialized tx_msg into RedelgationTx args.
///
/// # Arguments
///
/// * `redelegate_msg` - Borsh serialized redelegation_msg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn redelegate_tx_args(
    redelegate_msg: &[u8],
    tx_msg: &[u8],
) -> Result<args::Redelegate, JsError> {
    let redelegate_msg = RedelegateMsg::try_from_slice(redelegate_msg)?;

    let RedelegateMsg {
        owner,
        source_validator,
        destination_validator,
        amount,
    } = redelegate_msg;

    let owner = Address::from_str(&owner)?;
    let src_validator = Address::from_str(&source_validator)?;
    let dest_validator = Address::from_str(&destination_validator)?;
    let amount = Amount::from_str(&amount, NATIVE_MAX_DECIMAL_PLACES)?;
    let tx = tx_msg_into_args(tx_msg)?;

    let args = args::Redelegate {
        tx,
        src_validator,
        dest_validator,
        amount,
        owner,
        tx_code_path: PathBuf::from("tx_redelegate.wasm"),
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct VoteProposalMsg {
    signer: String,
    proposal_id: u64,
    vote: String,
}

impl VoteProposalMsg {
    pub fn new(signer: String, proposal_id: u64, vote: String) -> VoteProposalMsg {
        VoteProposalMsg {
            signer,
            proposal_id,
            vote,
        }
    }
}

/// Maps serialized tx_msg into VoteProposalTx args.
///
/// # Arguments
///
/// * `vote_proposal_msg` - Borsh serialized vote_proposal_msg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn vote_proposal_tx_args(
    vote_proposal_msg: &[u8],
    tx_msg: &[u8],
) -> Result<args::VoteProposal, JsError> {
    let vote_proposal_msg = VoteProposalMsg::try_from_slice(vote_proposal_msg)?;

    let VoteProposalMsg {
        signer,
        proposal_id,
        vote,
    } = vote_proposal_msg;
    let tx = tx_msg_into_args(tx_msg)?;
    let voter_address = Address::from_str(&signer)?;

    let args = args::VoteProposal {
        tx,
        proposal_id,
        vote,
        voter_address,
        tx_code_path: PathBuf::from("tx_vote_proposal.wasm"),
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct ClaimRewardsMsg {
    validator: String,
    source: Option<String>,
}

impl ClaimRewardsMsg {
    pub fn new(validator: String, source: Option<String>) -> ClaimRewardsMsg {
        ClaimRewardsMsg { validator, source }
    }
}

/// Maps serialized tx_msg into ClaimRewardsTx args.
///
/// # Arguments
///
/// * `claim_rewards_msg` - Borsh serialized claim_rewards_msg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn claim_rewards_tx_args(
    claim_rewards_msg: &[u8],
    tx_msg: &[u8],
) -> Result<args::ClaimRewards, JsError> {
    let claim_rewards_msg = ClaimRewardsMsg::try_from_slice(claim_rewards_msg)?;

    let ClaimRewardsMsg { validator, source } = claim_rewards_msg;
    let tx = tx_msg_into_args(tx_msg)?;

    let validator_address = Address::from_str(&validator)?;
    let source_address = source.map(|str| Address::from_str(&str).expect("valid address"));

    let args = args::ClaimRewards {
        tx,
        validator: validator_address,
        source: source_address,
        tx_code_path: PathBuf::from("tx_claim_rewards.wasm"),
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct TransferDataMsg {
    owner: String,
    token: String,
    amount: String,
}

impl TransferDataMsg {
    pub fn new(owner: String, token: String, amount: String) -> TransferDataMsg {
        TransferDataMsg {
            owner,
            token,
            amount,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct TransferMsg {
    sources: Vec<TransferDataMsg>,
    targets: Vec<TransferDataMsg>,
    shielded_section_hash: Option<Vec<u8>>,
}

impl TransferMsg {
    pub fn new(
        sources: Vec<TransferDataMsg>,
        targets: Vec<TransferDataMsg>,
        shielded_section_hash: Option<Vec<u8>>,
    ) -> TransferMsg {
        TransferMsg {
            sources,
            targets,
            shielded_section_hash,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct TransparentTransferDataMsg {
    source: String,
    target: String,
    token: String,
    amount: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct TransparentTransferMsg {
    data: Vec<TransparentTransferDataMsg>,
}

/// Maps serialized tx_msg into TransferTx args.
///
/// # Arguments
///
/// * `transfer_msg` - Borsh serialized TransparentTransferMsg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn transparent_transfer_tx_args(
    transfer_msg: &[u8],
    tx_msg: &[u8],
) -> Result<args::TxTransparentTransfer, JsError> {
    let transfer_msg = TransparentTransferMsg::try_from_slice(transfer_msg)?;
    let TransparentTransferMsg { data } = transfer_msg;

    let mut transfer_data: Vec<args::TxTransparentTransferData> = vec![];

    for transfer in data {
        let source = Address::from_str(&transfer.source)?;
        let target = Address::from_str(&transfer.target)?;
        let token = Address::from_str(&transfer.token)?;
        let denom_amount =
            DenominatedAmount::from_str(&transfer.amount).expect("Amount to be valid.");
        let amount = InputAmount::Unvalidated(denom_amount);

        transfer_data.push(args::TxTransparentTransferData {
            source,
            target,
            token,
            amount,
        });
    }

    let tx = tx_msg_into_args(tx_msg)?;

    let args = args::TxTransparentTransfer {
        tx,
        data: transfer_data,
        tx_code_path: PathBuf::from("tx_transfer.wasm"),
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct ShieldedTransferDataMsg {
    source: Vec<u8>,
    target: String,
    token: String,
    amount: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct ShieldedTransferMsg {
    data: Vec<ShieldedTransferDataMsg>,
    gas_spending_keys: Vec<Vec<u8>>,
}

/// Maps serialized tx_msg into TxShieldedTransfer args.
///
/// # Arguments
///
/// * `shielding_transfer_msg` - Borsh serialized ShieldingTransferMsg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn shielded_transfer_tx_args(
    shielded_transfer_msg: &[u8],
    tx_msg: &[u8],
) -> Result<args::TxShieldedTransfer, JsError> {
    let shielded_transfer_msg = ShieldedTransferMsg::try_from_slice(shielded_transfer_msg)?;
    let ShieldedTransferMsg {
        data,
        gas_spending_keys,
    } = shielded_transfer_msg;

    let mut shielded_transfer_data: Vec<args::TxShieldedTransferData> = vec![];

    for shielded_transfer in data {
        let mut source = PseudoExtendedKey::try_from_slice(&shielded_transfer.source)?;
        source.augment_spend_authorizing_key_unchecked(PrivateKey(jubjub::Fr::default()));
        web_sys::console::log_1(&format!("source: {:?}", source).into());

        let target = PaymentAddress::from_str(&shielded_transfer.target)?;
        let token = Address::from_str(&shielded_transfer.token)?;
        let denom_amount =
            DenominatedAmount::from_str(&shielded_transfer.amount).expect("Amount to be valid.");
        let amount = InputAmount::Unvalidated(denom_amount);

        shielded_transfer_data.push(args::TxShieldedTransferData {
            source,
            target,
            token,
            amount,
        });
    }

    let tx = tx_msg_into_args(tx_msg)?;
    let mut gsk: Vec<PseudoExtendedKey> = vec![];

    for sk in gas_spending_keys {
        let gas_spending_key = PseudoExtendedKey::try_from_slice(&sk)?;
        gsk.push(gas_spending_key);
    }

    let args = args::TxShieldedTransfer {
        data: shielded_transfer_data,
        tx,
        tx_code_path: PathBuf::from("tx_transfer.wasm"),
        // TODO: false for now
        disposable_signing_key: false,
        gas_spending_keys: gsk,
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct ShieldingTransferDataMsg {
    source: String,
    token: String,
    amount: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct ShieldingTransferMsg {
    target: String,
    data: Vec<ShieldingTransferDataMsg>,
}

/// Maps serialized tx_msg into TxShieldingTransfer args.
///
/// # Arguments
///
/// * `shielding_transfer_msg` - Borsh serialized ShieldingTransferMsg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn shielding_transfer_tx_args(
    shielding_transfer_msg: &[u8],
    tx_msg: &[u8],
) -> Result<args::TxShieldingTransfer, JsError> {
    let shielding_transfer_msg = ShieldingTransferMsg::try_from_slice(shielding_transfer_msg)?;
    let ShieldingTransferMsg { target, data } = shielding_transfer_msg;
    let target = PaymentAddress::from_str(&target)?;

    let mut shielding_transfer_data: Vec<args::TxShieldingTransferData> = vec![];

    for shielding_transfer in data {
        let source = Address::from_str(&shielding_transfer.source)?;
        let token = Address::from_str(&shielding_transfer.token)?;
        let denom_amount =
            DenominatedAmount::from_str(&shielding_transfer.amount).expect("Amount to be valid.");
        let amount = InputAmount::Unvalidated(denom_amount);

        shielding_transfer_data.push(args::TxShieldingTransferData {
            source,
            token,
            amount,
        });
    }

    let tx = tx_msg_into_args(tx_msg)?;

    let args = args::TxShieldingTransfer {
        data: shielding_transfer_data,
        target,
        tx,
        tx_code_path: PathBuf::from("tx_transfer.wasm"),
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct UnshieldingTransferDataMsg {
    target: String,
    token: String,
    amount: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct UnshieldingTransferMsg {
    source: Vec<u8>,
    data: Vec<UnshieldingTransferDataMsg>,
    gas_spending_keys: Vec<Vec<u8>>,
}

/// Maps serialized tx_msg into TxUnshieldingTransfer args.
///
/// # Arguments
///
/// * `shielding_transfer_msg` - Borsh serialized UnshieldingTransferMsg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn unshielding_transfer_tx_args(
    unshielding_transfer_msg: &[u8],
    tx_msg: &[u8],
) -> Result<args::TxUnshieldingTransfer, JsError> {
    let unshielding_transfer_msg =
        UnshieldingTransferMsg::try_from_slice(unshielding_transfer_msg)?;
    let UnshieldingTransferMsg {
        source,
        data,
        gas_spending_keys,
    } = unshielding_transfer_msg;
    let source = PseudoExtendedKey::try_from_slice(&source)?;

    let mut unshielding_transfer_data: Vec<args::TxUnshieldingTransferData> = vec![];

    for unshielding_transfer in data {
        let target = Address::from_str(&unshielding_transfer.target)?;
        let token = Address::from_str(&unshielding_transfer.token)?;
        let denom_amount =
            DenominatedAmount::from_str(&unshielding_transfer.amount).expect("Amount to be valid.");
        let amount = InputAmount::Unvalidated(denom_amount);

        unshielding_transfer_data.push(args::TxUnshieldingTransferData {
            target,
            token,
            amount,
        });
    }

    let tx = tx_msg_into_args(tx_msg)?;

    let args = args::TxUnshieldingTransfer {
        data: unshielding_transfer_data,
        source,
        tx,
        gas_spending_keys: vec![],
        // TODO: false for now
        disposable_signing_key: false,
        tx_code_path: PathBuf::from("tx_transfer.wasm"),
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct IbcTransferMsg {
    source: String,
    receiver: String,
    token: String,
    amount: String,
    port_id: String,
    channel_id: String,
    timeout_height: Option<u64>,
    timeout_sec_offset: Option<u64>,
    memo: Option<String>,
    shielding_data: Option<Vec<u8>>,
}

impl IbcTransferMsg {
    pub fn new(
        source: String,
        receiver: String,
        token: String,
        amount: String,
        port_id: String,
        channel_id: String,
        timeout_height: Option<u64>,
        timeout_sec_offset: Option<u64>,
        memo: Option<String>,
        shielding_data: Option<Vec<u8>>,
    ) -> IbcTransferMsg {
        IbcTransferMsg {
            source,
            receiver,
            token,
            amount,
            port_id,
            channel_id,
            timeout_height,
            timeout_sec_offset,
            memo,
            shielding_data,
        }
    }
}

/// Maps serialized tx_msg into IbcTransferTx args.
///
/// # Arguments
///
/// * `ibc_transfer_msg` - Borsh serialized ibc_transfer_msg.
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if the tx_msg can't be deserialized or
/// Rust structs can't be created.
pub fn ibc_transfer_tx_args(
    ibc_transfer_msg: &[u8],
    tx_msg: &[u8],
) -> Result<args::TxIbcTransfer, JsError> {
    let ibc_transfer_msg = IbcTransferMsg::try_from_slice(ibc_transfer_msg)?;
    let IbcTransferMsg {
        source,
        receiver,
        token,
        amount,
        port_id,
        channel_id,
        timeout_height,
        timeout_sec_offset,
        memo,
        shielding_data,
    } = ibc_transfer_msg;

    let source_address = Address::from_str(&source)?;
    let source = TransferSource::Address(source_address);
    let token = Address::from_str(&token)?;
    let denom_amount = DenominatedAmount::from_str(&amount).expect("Amount to be valid.");
    let amount = InputAmount::Unvalidated(denom_amount);
    let port_id = PortId::from_str(&port_id).expect("Port id to be valid");
    let channel_id = ChannelId::from_str(&channel_id).expect("Channel id to be valid");
    let ibc_shielding_data = match shielding_data {
        Some(v) => Some(IbcShieldingData::try_from_slice(&v)?),
        None => None,
    };

    let tx = tx_msg_into_args(tx_msg)?;

    let args = args::TxIbcTransfer {
        tx,
        ibc_memo: memo,
        ibc_shielding_data,
        source,
        receiver,
        token,
        amount,
        port_id,
        channel_id,
        timeout_height,
        timeout_sec_offset,
        // TODO: false for now
        disposable_signing_key: false,
        tx_code_path: PathBuf::from("tx_ibc.wasm"),
        refund_target: None,
        // TODO: Implement?
        gas_spending_keys: vec![],
    };

    Ok(args)
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
#[borsh(crate = "namada_sdk::borsh")]
pub struct EthBridgeTransferMsg {
    nut: bool,
    asset: String,
    recipient: String,
    sender: String,
    amount: String,
    fee_amount: String,
    fee_payer: Option<String>,
    fee_token: String,
}

pub fn eth_bridge_transfer_tx_args(
    eth_bridge_transfer_msg: &[u8],
    tx_msg: &[u8],
) -> Result<args::EthereumBridgePool, JsError> {
    let eth_bridge_transfer_msg = EthBridgeTransferMsg::try_from_slice(eth_bridge_transfer_msg)?;
    let EthBridgeTransferMsg {
        nut,
        asset,
        recipient,
        sender,
        amount,
        fee_amount,
        fee_payer,
        fee_token,
    } = eth_bridge_transfer_msg;

    let tx = tx_msg_into_args(tx_msg)?;
    let asset = EthAddress::from_str(&asset).map_err(|e| JsError::new(&format!("{}", e)))?;
    let recipient =
        EthAddress::from_str(&recipient).map_err(|e| JsError::new(&format!("{}", e)))?;
    let sender = Address::from_str(&sender)?;
    let denom_amount = DenominatedAmount::from_str(&amount).expect("Amount to be valid.");
    let amount = InputAmount::Unvalidated(denom_amount);
    let denom_amount = DenominatedAmount::from_str(&fee_amount).expect("Amount to be valid.");
    let fee_amount = InputAmount::Unvalidated(denom_amount);
    let fee_payer = fee_payer.map(|v| Address::from_str(&v)).transpose()?;
    let fee_token = Address::from_str(&fee_token)?;
    let code_path = PathBuf::from("tx_bridge_pool.wasm");

    let args = args::EthereumBridgePool {
        nut,
        tx,
        asset,
        recipient,
        sender,
        amount,
        fee_amount,
        fee_payer,
        fee_token,
        code_path,
    };

    Ok(args)
}

pub fn tx_args_from_slice(tx_msg_bytes: &[u8]) -> Result<args::Tx, JsError> {
    let args = tx_msg_into_args(tx_msg_bytes)?;

    Ok(args)
}

/// Maps serialized tx_msg into Tx args.
/// This is common for all tx types.
///
/// # Arguments
///
/// * `tx_msg` - Borsh serialized tx_msg.
///
/// # Errors
///
/// Returns JsError if token address is invalid.
fn tx_msg_into_args(tx_msg: &[u8]) -> Result<args::Tx, JsError> {
    let tx_msg = WrapperTxMsg::try_from_slice(tx_msg)?;
    let WrapperTxMsg {
        token,
        fee_amount,
        gas_limit,
        chain_id,
        public_key,
        memo,
    } = tx_msg;

    let token = Address::from_str(&token)?;

    let fee_amount = DenominatedAmount::from_str(&fee_amount)
        .expect(format!("Fee amount has to be valid. Received {}", fee_amount).as_str());
    let fee_input_amount = InputAmount::Unvalidated(fee_amount);

    let public_key = match public_key {
        Some(v) => {
            let pk = PublicKey::from_str(&v)?;
            Some(pk)
        }
        _ => None,
    };

    let signing_keys: Vec<PublicKey> = match public_key {
        Some(v) => vec![v.clone()],
        _ => vec![],
    };

    // Ledger address is not used in the SDK.
    // We can leave it as whatever as long as it's valid url.
    let ledger_address = tendermint_rpc::Url::from_str("http://notinuse:13337").unwrap();

    let memo = memo.map(|v| v.as_bytes().to_vec());

    let args = args::Tx {
        dry_run: false,
        dry_run_wrapper: false,
        dump_tx: false,
        force: false,
        broadcast_only: false,
        ledger_address,
        wallet_alias_force: false,
        initialized_account_alias: None,
        fee_amount: Some(fee_input_amount),
        fee_token: token.clone(),
        gas_limit: GasLimit::from_str(&gas_limit).expect("Gas limit to be valid"),
        wrapper_fee_payer: None,
        output_folder: None,
        expiration: TxExpiration::Default,
        chain_id: Some(ChainId(String::from(chain_id))),
        signatures: vec![],
        signing_keys,
        tx_reveal_code_path: PathBuf::from("tx_reveal_pk.wasm"),
        use_device: false,
        password: None,
        memo,
        device_transport: Default::default(),
    };

    Ok(args)
}

pub async fn generate_masp_build_params(
    // TODO: those will be needed for HD Wallet support
    _spend_len: usize,
    _convert_len: usize,
    _output_len: usize,
    args: &args::Tx,
) -> Result<Box<dyn BuildParams>, error::Error> {
    // Construct the build parameters that parameterized the Transaction
    // authorizations
    if args.use_device {
        // HD Wallet support
        Err(error::Error::Other("Device not supported".into()))
    } else {
        Ok(Box::new(RngBuildParams::new(OsRng)))
    }
}

// Sign the given transaction's MASP component using signatures produced by the
// hardware wallet. This function takes the list of spending keys that are
// hosted on the hardware wallet.
pub async fn masp_sign(
    tx: &mut Tx,
    signing_data: &SigningTxData,
    bparams: &mut dyn BuildParams,
) -> Result<(), error::Error> {
    // Get the MASP section that is the target of our signing
    if let Some(shielded_hash) = signing_data.shielded_hash {
        let mut masp_tx = tx
            .get_masp_section(&shielded_hash)
            .expect("Expected to find the indicated MASP Transaction")
            .clone();
        let masp_builder = tx
            .get_masp_builder(&shielded_hash)
            .expect("Expected to find the indicated MASP Builder");

        // Reverse the spend metadata to enable looking up construction
        // material
        let sapling_inputs = masp_builder.builder.sapling_inputs();
        let mut descriptor_map = vec![0; sapling_inputs.len()];
        for i in 0.. {
            if let Some(pos) = masp_builder.metadata.spend_index(i) {
                descriptor_map[pos] = i;
            } else {
                break;
            };
        }

        let tx_data = masp_tx.deref();

        let unauth_tx_data = partial_deauthorize(tx_data).unwrap();

        let txid_parts = unauth_tx_data.digest(TxIdDigester);
        let sighash = signature_hash(&unauth_tx_data, &SignableInput::Shielded, &txid_parts);

        // This we just get frpm extension
        let xsk = "zsknam1q00j7ewuqqqqpq8gz8yvtpx226gg7nhw9vrmyvp3ay2gnjtp3xg86lsvtc7ng9nsk9lrjlutm77ghgsewqhrxu32ns054sthl4qeprppxahze0pmthmqzjqa2pmzp0xy9hnqmnkwswygf875ra4ksllyp63r6rjze2n8cwsy355fhc2lq0hyfsa2ehsflrumwkx5tqkq992g8p0af4zw7cx94mdntgvkacrs9r3j45fdsjc209f7p79lzz6mr5vdk3fqt4jkkjlckmc3ckwpk";
        let xsk = namada_sdk::ExtendedSpendingKey::from_str(xsk).unwrap();
        web_sys::console::log_1(&format!("xsk zzzzzawdasd: {:?}", xsk).into());

        let mut authorizations = HashMap::new();
        for (tx_pos, _) in descriptor_map.iter().enumerate() {
            let pk = PrivateKey(
                namada_sdk::masp_primitives::zip32::ExtendedSpendingKey::from(xsk)
                    .expsk
                    .ask,
            );
            let mut rng = OsRng;

            let sig = spend_sig(pk, bparams.spend_alpha(tx_pos), sighash.as_ref(), &mut rng);

            authorizations.insert(tx_pos, sig);
        }

        tx.sections.iter().for_each(|section| match section {
            Section::MaspTx(d) => {
                web_sys::console::log_1(&format!("masp_tx oldddd: {:?}", d).into());
            }
            _ => {}
        });

        masp_tx = (*masp_tx)
            .clone()
            .map_authorization::<masp_primitives::transaction::Authorized>(
                (),
                MapSaplingSigAuth(authorizations),
            )
            .freeze()
            .unwrap();

        tx.remove_masp_section(&shielded_hash);
        tx.add_section(Section::MaspTx(masp_tx));
    }
    Ok(())
}

struct MapSaplingSigAuth(HashMap<usize, <sapling::Authorized as sapling::Authorization>::AuthSig>);

impl sapling::MapAuth<sapling::Authorized, sapling::Authorized> for MapSaplingSigAuth {
    fn map_proof(
        &self,
        p: <sapling::Authorized as sapling::Authorization>::Proof,
        _pos: usize,
    ) -> <sapling::Authorized as sapling::Authorization>::Proof {
        p
    }

    fn map_auth_sig(
        &self,
        s: <sapling::Authorized as sapling::Authorization>::AuthSig,
        pos: usize,
    ) -> <sapling::Authorized as sapling::Authorization>::AuthSig {
        self.0.get(&pos).cloned().unwrap_or(s)
    }

    fn map_authorization(&self, a: sapling::Authorized) -> sapling::Authorized {
        a
    }
}
