use anyhow::Context;
use web3::ethabi::{encode, Address, Token};
use zksync_types::api::TransactionRequest;
use zksync_types::fee::Fee;
use zksync_types::l2::{L2Tx, TransactionType};
use zksync_types::{
    ExecuteTransactionCommon, K256PrivateKey, Nonce, PackedEthSignature, Transaction, H256, U256,
};

pub fn gen_l2_tx(
    private_key: &zksync_types::K256PrivateKey,
    to: Option<Address>,
    data: Vec<u8>,
    value: U256,
    nonce: u32,
    fee: Fee,
    timestamp: u64,
    chain_id: u64,
) -> anyhow::Result<Transaction> {
    let initiator_address = private_key.address();

    // We do a whole dance to reconstruct missing data: RLP encoding, hash and signature.
    let req = TransactionRequest {
        nonce: nonce.into(),
        from: Some(initiator_address),
        to,
        value,
        gas_price: fee.max_fee_per_gas,
        gas: fee.gas_limit,
        max_priority_fee_per_gas: None,
        input: zksync_types::web3::Bytes(data),
        v: None,
        r: None,
        s: None,
        raw: None,
        transaction_type: None,
        access_list: None,
        eip712_meta: None,
        chain_id: Some(chain_id),
    };

    let data = req
        .get_default_signed_message()
        .context("get_default_signed_message()")?;

    let sig = PackedEthSignature::sign_raw(private_key, &data).context("sign_raw")?;

    let raw = req.get_signed_bytes(&sig).context("get_signed_bytes")?;

    let (req, hash) =
        TransactionRequest::from_bytes_unverified(&raw).context("from_bytes_unverified()")?;
    // Since we allow users to specify `None` recipient, EVM emulation is implicitly enabled.
    let mut tx = L2Tx::from_request(req, 60000, true).context("from_request()")?;
    tx.set_input(raw, hash);

    tx.received_timestamp_ms = timestamp * 1000; // seconds to ms
    Ok(tx.into())
}

// TODO import zkos dev branch

pub(crate) const MAX_GAS_PER_PUBDATA_BYTE: u64 = 50_000;

#[derive(Debug, Default, Clone)]
pub(crate) struct TransactionData {
    pub(crate) tx_type: u8,
    pub(crate) from: Address,
    pub(crate) to: Option<Address>,
    pub(crate) gas_limit: U256,
    pub(crate) pubdata_price_limit: U256,
    pub(crate) max_fee_per_gas: U256,
    pub(crate) max_priority_fee_per_gas: U256,
    pub(crate) paymaster: Address,
    pub(crate) nonce: U256,
    pub(crate) value: U256,
    // The reserved fields that are unique for different types of transactions.
    // E.g. nonce is currently used in all transaction, but it should not be mandatory
    // in the long run.
    pub(crate) reserved: [U256; 4],
    pub(crate) data: Vec<u8>,
    pub(crate) signature: Vec<u8>,
    // The factory deps provided with the transaction.
    // Note that *only hashes* of these bytecodes are signed by the user
    // and they are used in the ABI encoding of the struct.
    // TODO: include this into the tx signature as part of SMA-1010
    pub(crate) factory_deps: Vec<Vec<u8>>,
    pub(crate) paymaster_input: Vec<u8>,
    pub(crate) reserved_dynamic: Vec<u8>,
    pub(crate) raw_bytes: Option<Vec<u8>>,
}

impl TransactionData {
    pub fn abi_encode(self) -> Vec<u8> {
        let mut res = encode(&[Token::Tuple(vec![
            Token::Uint(U256::from_big_endian(&self.tx_type.to_be_bytes())),
            Token::Address(self.from),
            Token::Address(self.to.unwrap_or_default()),
            Token::Uint(self.gas_limit),
            Token::Uint(self.pubdata_price_limit),
            Token::Uint(self.max_fee_per_gas),
            Token::Uint(self.max_priority_fee_per_gas),
            Token::Address(self.paymaster),
            Token::Uint(self.nonce),
            Token::Uint(self.value),
            Token::FixedArray(self.reserved.iter().copied().map(Token::Uint).collect()),
            Token::Bytes(self.data),
            Token::Bytes(self.signature),
            // todo: factory deps must be empty
            Token::Array(Vec::new()),
            Token::Bytes(self.paymaster_input),
            Token::Bytes(self.reserved_dynamic),
        ])]);

        res.drain(0..32);
        res
    }
}

impl From<Transaction> for TransactionData {
    fn from(execute_tx: Transaction) -> Self {
        match execute_tx.common_data {
            ExecuteTransactionCommon::L2(common_data) => {
                let nonce = U256::from_big_endian(&common_data.nonce.to_be_bytes());

                let should_check_chain_id = if matches!(
                    common_data.transaction_type,
                    TransactionType::LegacyTransaction
                ) && common_data.extract_chain_id().is_some()
                {
                    U256([1, 0, 0, 0])
                } else {
                    U256::zero()
                };

                // todo: second `reserved` value should be non-zero for deployment tx

                // Ethereum transactions do not sign gas per pubdata limit, and so for them we need to use
                // some default value. We use the maximum possible value that is allowed by the bootloader
                // (i.e. we can not use u64::MAX, because the bootloader requires gas per pubdata for such
                // transactions to be higher than `MAX_GAS_PER_PUBDATA_BYTE`).
                let gas_per_pubdata_limit = if common_data.transaction_type.is_ethereum_type() {
                    MAX_GAS_PER_PUBDATA_BYTE.into()
                } else {
                    unreachable!()
                };

                let is_deployment_transaction = match execute_tx.execute.contract_address {
                    None =>
                    // that means it's a deploy transaction
                    {
                        U256([1, 0, 0, 0])
                    }
                    // all other transactions
                    Some(_) => U256::zero(),
                };

                TransactionData {
                    tx_type: (common_data.transaction_type as u32) as u8,
                    from: common_data.initiator_address,
                    to: execute_tx.execute.contract_address,
                    gas_limit: common_data.fee.gas_limit,
                    pubdata_price_limit: gas_per_pubdata_limit,
                    max_fee_per_gas: common_data.fee.max_fee_per_gas,
                    max_priority_fee_per_gas: common_data.fee.max_priority_fee_per_gas,
                    paymaster: common_data.paymaster_params.paymaster,
                    nonce,
                    value: execute_tx.execute.value,
                    reserved: [
                        should_check_chain_id,
                        is_deployment_transaction,
                        U256::zero(),
                        U256::zero(),
                    ],
                    data: execute_tx.execute.calldata,
                    signature: common_data.signature,
                    factory_deps: execute_tx.execute.factory_deps,
                    paymaster_input: common_data.paymaster_params.paymaster_input,
                    reserved_dynamic: vec![],
                    raw_bytes: execute_tx.raw_bytes.map(|a| a.0),
                }
            }
            ExecuteTransactionCommon::L1(_) => {
                unimplemented!("l1 transactions are not supported for zk os")
            }
            ExecuteTransactionCommon::ProtocolUpgrade(_) => {
                unreachable!()
            }
        }
    }
}
