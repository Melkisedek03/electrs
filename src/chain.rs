#[cfg(not(feature = "liquid"))] // use regular Bitcoin data structures
pub use bitcoin::{
    address, blockdata::block::Header as BlockHeader, blockdata::script, consensus::deserialize,
    hash_types::TxMerkleNode, Address, Block, BlockHash, OutPoint, ScriptBuf as Script, Sequence,
    Transaction, TxIn, TxOut, Txid, Wtxid,
};

#[cfg(feature = "liquid")]
pub use {
    crate::elements::asset,
    elements::{
        address, confidential, encode::deserialize, script, Address, AssetId, Block, BlockHash,
        BlockHeader, OutPoint, Script, Sequence, Transaction, TxIn, TxMerkleNode, TxOut, Txid,
        Wtxid,
    },
};

use bitcoin::blockdata::constants::genesis_block;
#[cfg(not(feature = "liquid"))]
use bitcoin::hashes::Hash;
pub use bitcoin::network::Network as BNetwork;

#[cfg(not(feature = "liquid"))]
pub type Value = u64;
#[cfg(feature = "liquid")]
pub use confidential::Value;

#[derive(Debug, Copy, Clone, PartialEq, Hash, Serialize, Ord, PartialOrd, Eq)]
pub enum Network {
    #[cfg(not(feature = "liquid"))]
    Bitcoin,
    #[cfg(not(feature = "liquid"))]
    Testnet,
    #[cfg(not(feature = "liquid"))]
    Testnet4,
    #[cfg(not(feature = "liquid"))]
    Regtest,
    #[cfg(not(feature = "liquid"))]
    Signet,
    /// Nusacoin (NUX) mainnet: a Bitcoin-protocol fork (X11 PoW, SegWit).
    /// Unlike the variants above, its chain parameters are NOT hardcoded in
    /// rust-bitcoin: the genesis hash is resolved from the connected daemon
    /// (see `Daemon::get_genesis_hash`) and address prefixes follow the
    /// Nusacoin chainparams (P2PKH version 0x35, P2SH version 0x4b,
    /// bech32 HRP "nu").
    #[cfg(not(feature = "liquid"))]
    Nusacoin,
    /// Nusacoin (NUX) testnet: same fork, testnet chain parameters
    /// (P2P magic 0x0b110907, P2PKH version 111, P2SH version 196,
    /// bech32 HRP "tn"). The genesis hash is likewise resolved from the
    /// connected daemon, so any testnet chain works.
    #[cfg(not(feature = "liquid"))]
    NusacoinTestnet,

    #[cfg(feature = "liquid")]
    Liquid,
    #[cfg(feature = "liquid")]
    LiquidTestnet,
    #[cfg(feature = "liquid")]
    LiquidRegtest,
}

impl Network {
    #[cfg(not(feature = "liquid"))]
    pub fn magic(self) -> u32 {
        match self {
            // Nusacoin mainnet pchMessageStart = 0x4E555341 ("NUSA"), see chainparams.cpp
            Network::Nusacoin => u32::from_le_bytes([0x4E, 0x55, 0x53, 0x41]),
            // Nusacoin testnet pchMessageStart, see chainparams.cpp (CTestNetParams)
            Network::NusacoinTestnet => u32::from_le_bytes([0x0b, 0x11, 0x09, 0x07]),
            _ => u32::from_le_bytes(BNetwork::from(self).magic().to_bytes()),
        }
    }

    #[cfg(feature = "liquid")]
    pub fn magic(self) -> u32 {
        match self {
            Network::Liquid | Network::LiquidRegtest => 0xDAB5_BFFA,
            Network::LiquidTestnet => 0x62DD_0E41,
        }
    }

    pub fn is_regtest(self) -> bool {
        match self {
            #[cfg(not(feature = "liquid"))]
            Network::Regtest => true,
            #[cfg(feature = "liquid")]
            Network::LiquidRegtest => true,
            _ => false,
        }
    }

    #[cfg(feature = "liquid")]
    pub fn address_params(self) -> &'static address::AddressParams {
        // Liquid regtest uses elements's address params
        match self {
            Network::Liquid => &address::AddressParams::LIQUID,
            Network::LiquidRegtest => &address::AddressParams::ELEMENTS,
            Network::LiquidTestnet => &address::AddressParams::LIQUID_TESTNET,
        }
    }

    #[cfg(feature = "liquid")]
    pub fn native_asset(self) -> &'static AssetId {
        match self {
            Network::Liquid => &*asset::NATIVE_ASSET_ID,
            Network::LiquidTestnet => &*asset::NATIVE_ASSET_ID_TESTNET,
            Network::LiquidRegtest => &*asset::NATIVE_ASSET_ID_REGTEST,
        }
    }

    #[cfg(feature = "liquid")]
    pub fn pegged_asset(self) -> Option<&'static AssetId> {
        match self {
            Network::Liquid => Some(&*asset::NATIVE_ASSET_ID),
            Network::LiquidTestnet | Network::LiquidRegtest => None,
        }
    }

    pub fn names() -> Vec<String> {
        #[cfg(not(feature = "liquid"))]
        return vec![
            "mainnet".to_string(),
            "testnet".to_string(),
            "testnet4".to_string(),
            "regtest".to_string(),
            "signet".to_string(),
            "nusacoin".to_string(),
            "nusacoin-testnet".to_string(),
        ];

        #[cfg(feature = "liquid")]
        return vec![
            "liquid".to_string(),
            "liquidtestnet".to_string(),
            "liquidregtest".to_string(),
        ];
    }
}

pub fn genesis_hash(network: Network) -> BlockHash {
    #[cfg(not(feature = "liquid"))]
    return match network {
        // The Nusacoin genesis hash is intentionally not hardcoded: it is
        // resolved from the connected daemon via `Daemon::get_genesis_hash()`,
        // so this fork works with any Nusacoin chain (mainnet/testnet/regtest).
        Network::Nusacoin | Network::NusacoinTestnet => panic!(
            "Nusacoin genesis hash must be resolved from the daemon, see Daemon::get_genesis_hash()"
        ),
        _ => bitcoin_genesis_hash(network.into()),
    };
    #[cfg(feature = "liquid")]
    return liquid_genesis_hash(network);
}

/// Compute the block hash of a header for the given network.
///
/// Nusacoin identifies blocks by their X11 hash (its `GetHash()` is wired to
/// `HashX11`), not by Bitcoin's double-SHA256. Every other network keeps
/// rust-bitcoin's default SHA256d block hash.
#[cfg_attr(feature = "liquid", allow(unused_variables))]
pub fn block_hash(network: Network, header: &BlockHeader) -> BlockHash {
    #[cfg(not(feature = "liquid"))]
    return match network {
        Network::Nusacoin | Network::NusacoinTestnet => x11_block_hash(header),
        _ => header.block_hash(),
    };
    #[cfg(feature = "liquid")]
    return header.block_hash();
}

/// X11 block hash, matching the daemon's `GetHash()` on Nusacoin networks.
/// `rs_x11_hash` returns the digest in internal (little-endian) byte order,
/// which is exactly what `BlockHash::from_byte_array` expects, so no
/// byte-swapping is needed for the display format to match `getblockhash`.
#[cfg(not(feature = "liquid"))]
fn x11_block_hash(header: &BlockHeader) -> BlockHash {
    use bitcoin::consensus::encode::serialize;
    let digest = rs_x11_hash::get_x11_hash(serialize(header));
    BlockHash::from_byte_array(digest)
}

pub fn bitcoin_genesis_hash(network: BNetwork) -> bitcoin::BlockHash {
    lazy_static! {
        static ref BITCOIN_GENESIS: bitcoin::BlockHash =
            genesis_block(BNetwork::Bitcoin).block_hash();
        static ref TESTNET_GENESIS: bitcoin::BlockHash =
            genesis_block(BNetwork::Testnet).block_hash();
        static ref TESTNET4_GENESIS: bitcoin::BlockHash =
            genesis_block(BNetwork::Testnet4).block_hash();
        static ref REGTEST_GENESIS: bitcoin::BlockHash =
            genesis_block(BNetwork::Regtest).block_hash();
        static ref SIGNET_GENESIS: bitcoin::BlockHash =
            genesis_block(BNetwork::Signet).block_hash();
    }
    match network {
        BNetwork::Bitcoin => *BITCOIN_GENESIS,
        BNetwork::Testnet => *TESTNET_GENESIS,
        BNetwork::Testnet4 => *TESTNET4_GENESIS,
        BNetwork::Regtest => *REGTEST_GENESIS,
        BNetwork::Signet => *SIGNET_GENESIS,
    }
}

#[cfg(feature = "liquid")]
pub fn liquid_genesis_hash(network: Network) -> elements::BlockHash {
    use crate::util::DEFAULT_BLOCKHASH;

    lazy_static! {
        static ref LIQUID_GENESIS: BlockHash =
            "1466275836220db2944ca059a3a10ef6fd2ea684b0688d2c379296888a206003"
                .parse()
                .unwrap();
    }

    match network {
        Network::Liquid => *LIQUID_GENESIS,
        // The genesis block for liquid regtest chains varies based on the chain configuration.
        // This instead uses an all zeroed-out hash, which doesn't matter in practice because its
        // only used for Electrum server discovery, which isn't active on regtest.
        _ => *DEFAULT_BLOCKHASH,
    }
}

impl From<&str> for Network {
    fn from(network_name: &str) -> Self {
        match network_name {
            #[cfg(not(feature = "liquid"))]
            "mainnet" => Network::Bitcoin,
            #[cfg(not(feature = "liquid"))]
            "testnet" => Network::Testnet,
            #[cfg(not(feature = "liquid"))]
            "testnet4" => Network::Testnet4,
            #[cfg(not(feature = "liquid"))]
            "regtest" => Network::Regtest,
            #[cfg(not(feature = "liquid"))]
            "signet" => Network::Signet,
            #[cfg(not(feature = "liquid"))]
            "nusacoin" => Network::Nusacoin,
            #[cfg(not(feature = "liquid"))]
            "nusacoin-testnet" => Network::NusacoinTestnet,

            #[cfg(feature = "liquid")]
            "liquid" => Network::Liquid,
            #[cfg(feature = "liquid")]
            "liquidtestnet" => Network::LiquidTestnet,
            #[cfg(feature = "liquid")]
            "liquidregtest" => Network::LiquidRegtest,

            _ => panic!("unsupported Bitcoin network: {:?}", network_name),
        }
    }
}

#[cfg(not(feature = "liquid"))]
impl From<Network> for BNetwork {
    fn from(network: Network) -> Self {
        match network {
            Network::Bitcoin => BNetwork::Bitcoin,
            Network::Testnet => BNetwork::Testnet,
            Network::Testnet4 => BNetwork::Testnet4,
            Network::Regtest => BNetwork::Regtest,
            Network::Signet => BNetwork::Signet,
            // rust-bitcoin has no Nusacoin network parameters. Every conversion
            // site special-cases the Nusacoin networks before reaching here
            // (see Network::magic and genesis_hash), so this arm is unreachable.
            Network::Nusacoin | Network::NusacoinTestnet => {
                panic!("rust-bitcoin has no Nusacoin network parameters")
            }
        }
    }
}

#[cfg(not(feature = "liquid"))]
impl From<BNetwork> for Network {
    fn from(network: BNetwork) -> Self {
        match network {
            BNetwork::Bitcoin => Network::Bitcoin,
            BNetwork::Testnet => Network::Testnet,
            BNetwork::Testnet4 => Network::Testnet4,
            BNetwork::Regtest => Network::Regtest,
            BNetwork::Signet => Network::Signet,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// The Nusacoin daemon identifies blocks by their X11 hash, not SHA256d.
    /// This pins `block_hash()` to a real mainnet vector: the genesis header
    /// (from `getblockheader <genesis> false` on the live daemon) must hash
    /// to the daemon's `getblockhash 0`.
    ///
    /// Regression test for the "bitcoind returned non-contiguous headers"
    /// failure: with SHA256d the same header hashes to
    /// `fdad7256daf7e620a5adcf97ea6d154be219123c9723dd818b6610432e501892`,
    /// which matches nothing the daemon knows.
    #[test]
    #[cfg(not(feature = "liquid"))]
    fn nusacoin_block_hash_is_x11() {
        let raw = hex_to_bytes(
            "0100000000000000000000000000000000000000000000000000000000000000000000009bfd61aa387643ae280ad4d29bceb017541657fd213d0ef2df59d2a7ddda0e666fdb2c69f0ff0f1e4ea7be01",
        );
        let header: BlockHeader = deserialize(&raw).unwrap();
        assert_eq!(
            block_hash(Network::Nusacoin, &header).to_string(),
            "0000017123d6f996589bc2e58bb5502218012ac7f527ab599a3be84c1951c669"
        );
        assert_eq!(
            block_hash(Network::NusacoinTestnet, &header).to_string(),
            "0000017123d6f996589bc2e58bb5502218012ac7f527ab599a3be84c1951c669"
        );
    }

    /// Other networks keep rust-bitcoin's SHA256d block hash.
    #[test]
    #[cfg(not(feature = "liquid"))]
    fn bitcoin_block_hash_unchanged() {
        let raw = hex_to_bytes(
            "0100000000000000000000000000000000000000000000000000000000000000000000009bfd61aa387643ae280ad4d29bceb017541657fd213d0ef2df59d2a7ddda0e666fdb2c69f0ff0f1e4ea7be01",
        );
        let header: BlockHeader = deserialize(&raw).unwrap();
        assert_eq!(block_hash(Network::Bitcoin, &header), header.block_hash());
        // ... which is NOT the daemon's X11 hash of the same header.
        assert_ne!(
            block_hash(Network::Bitcoin, &header).to_string(),
            "0000017123d6f996589bc2e58bb5502218012ac7f527ab599a3be84c1951c669"
        );
    }
}
