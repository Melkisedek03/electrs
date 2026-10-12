#[cfg(feature = "liquid")]
use crate::elements::ebcompact::*;
#[cfg(feature = "liquid")]
use elements::address as elements_address;

use crate::chain::{script, Network, Script, TxIn, TxOut};
use script::Instruction::PushBytes;

pub struct InnerScripts {
    pub redeem_script: Option<Script>,
    pub witness_script: Option<Script>,
}

pub trait ScriptToAsm: std::fmt::Debug {
    fn to_asm(&self) -> String {
        let asm = format!("{:?}", self);
        (&asm[7..asm.len() - 1]).to_string()
    }
}
impl ScriptToAsm for bitcoin::ScriptBuf {}
#[cfg(feature = "liquid")]
impl ScriptToAsm for elements::Script {}

pub trait ScriptToAddr {
    fn to_address_str(&self, network: Network) -> Option<String>;
}
#[cfg(not(feature = "liquid"))]
impl ScriptToAddr for bitcoin::Script {
    fn to_address_str(&self, network: Network) -> Option<String> {
        match network {
            // rust-bitcoin only knows Bitcoin's address prefixes.
            Network::Nusacoin | Network::NusacoinTestnet => {
                nusacoin_script_to_address(self, network)
            }
            _ => bitcoin::Address::from_script(self, bitcoin::Network::from(network))
                .map(|s| s.to_string())
                .ok(),
        }
    }
}

/// Nusacoin (NUX) address parameters, following the Nusacoin chainparams
/// (see `src/chainparams.cpp` in the Nusacoin Core repo).
#[cfg(not(feature = "liquid"))]
struct NusacoinAddressParams {
    p2pkh_version: u8,
    p2sh_version: u8,
    bech32_hrp: &'static str,
}

#[cfg(not(feature = "liquid"))]
fn nusacoin_address_params(network: Network) -> NusacoinAddressParams {
    match network {
        // mainnet: P2PKH version byte 0x35, P2SH version byte 0x4b, bech32 HRP "nu"
        Network::Nusacoin => NusacoinAddressParams {
            p2pkh_version: 0x35,
            p2sh_version: 0x4b,
            bech32_hrp: "nu",
        },
        // testnet: P2PKH version byte 111, P2SH version byte 196, bech32 HRP "tn"
        Network::NusacoinTestnet => NusacoinAddressParams {
            p2pkh_version: 111,
            p2sh_version: 196,
            bech32_hrp: "tn",
        },
        _ => panic!("not a Nusacoin network: {:?}", network),
    }
}

/// Nusacoin (NUX) address encoding, following the Nusacoin chainparams.
#[cfg(not(feature = "liquid"))]
fn nusacoin_script_to_address(script: &bitcoin::Script, network: Network) -> Option<String> {
    use bitcoin::bech32::{segwit, Fe32, Hrp};
    use std::convert::TryFrom;

    let params = nusacoin_address_params(network);
    let bytes = script.as_bytes();
    if script.is_p2pkh() {
        // OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
        let mut payload = Vec::with_capacity(21);
        payload.push(params.p2pkh_version);
        payload.extend_from_slice(&bytes[3..23]);
        Some(bitcoin::base58::encode_check(&payload))
    } else if script.is_p2sh() {
        // OP_HASH160 <20 bytes> OP_EQUAL
        let mut payload = Vec::with_capacity(21);
        payload.push(params.p2sh_version);
        payload.extend_from_slice(&bytes[2..22]);
        Some(bitcoin::base58::encode_check(&payload))
    } else if script.is_witness_program() {
        // <version opcode> <push opcode> <program>
        let version = match bytes[0] {
            0x00 => 0u8,
            n @ 0x51..=0x60 => n - 0x50, // OP_1..OP_16 -> 1..16
            _ => return None,
        };
        let program = &bytes[2..];
        let hrp = Hrp::parse(params.bech32_hrp).ok()?;
        let fe_version = Fe32::try_from(version).ok()?;
        // segwit::encode picks Bech32 (v0) vs Bech32m (v1+) automatically.
        segwit::encode(hrp, fe_version, program).ok()
    } else {
        None
    }
}
/// Parse a Nusacoin (NUX) address string into its scriptPubKey.
/// Inverse of `nusacoin_script_to_address`: accepts base58check with the
/// network's version bytes (P2PKH/P2SH) and bech32/bech32m with the
/// network's HRP.
#[cfg(not(feature = "liquid"))]
pub fn nusacoin_address_to_script(addr: &str, network: Network) -> Option<Script> {
    use bitcoin::bech32::{segwit, Hrp};
    use bitcoin::hashes::Hash;
    use std::convert::TryFrom;

    let params = nusacoin_address_params(network);
    // Try base58check first.
    if let Ok(data) = bitcoin::base58::decode_check(addr) {
        if data.len() == 21 {
            let hash = <[u8; 20]>::try_from(&data[1..21]).ok()?;
            if data[0] == params.p2pkh_version {
                let hash = bitcoin::PubkeyHash::from_byte_array(hash);
                return Some(Script::new_p2pkh(&hash));
            }
            if data[0] == params.p2sh_version {
                let hash = bitcoin::ScriptHash::from_byte_array(hash);
                return Some(Script::new_p2sh(&hash));
            }
        }
        return None;
    }
    // Try bech32/bech32m with the network's HRP.
    let (hrp, version, program) = segwit::decode(addr).ok()?;
    if hrp != Hrp::parse(params.bech32_hrp).ok()? {
        return None;
    }
    let version = bitcoin::WitnessVersion::try_from(version.to_u8()).ok()?;
    let program = bitcoin::WitnessProgram::new(version, &program).ok()?;
    Some(Script::new_witness_program(&program))
}

#[cfg(feature = "liquid")]
impl ScriptToAddr for elements::Script {
    fn to_address_str(&self, network: Network) -> Option<String> {
        elements_address::Address::from_script(self, None, network.address_params())
            .map(|a| a.to_string())
    }
}

// Returns the witnessScript in the case of p2wsh, or the redeemScript in the case of p2sh.
pub fn get_innerscripts(txin: &TxIn, prevout: &TxOut) -> InnerScripts {
    // Wrapped redeemScript for P2SH spends
    let redeem_script = if prevout.script_pubkey.is_p2sh() {
        if let Some(Ok(PushBytes(redeemscript))) = txin.script_sig.instructions().last() {
            #[cfg(not(feature = "liquid"))] // rust-bitcoin has a PushBytes wrapper type
            let redeemscript = redeemscript.as_bytes();
            Some(Script::from(redeemscript.to_vec()))
        } else {
            None
        }
    } else {
        None
    };

    // Wrapped witnessScript for P2WSH or P2SH-P2WSH spends
    let witness_script = if prevout.script_pubkey.is_p2wsh()
        || redeem_script.as_ref().map_or(false, |s| s.is_p2wsh())
    {
        let witness = &txin.witness;
        #[cfg(feature = "liquid")]
        let witness = &witness.script_witness;

        // rust-bitcoin returns witness items as a [u8] slice, while rust-elements returns a Vec<u8>
        #[cfg(not(feature = "liquid"))]
        let wit_to_vec = Vec::from;
        #[cfg(feature = "liquid")]
        let wit_to_vec = Clone::clone;

        witness.iter().last().map(wit_to_vec).map(Script::from)
    } else {
        None
    };

    InnerScripts {
        redeem_script,
        witness_script,
    }
}

#[cfg(all(test, not(feature = "liquid")))]
mod tests {
    use super::*;
    use crate::chain::Network;

    fn hex_script(hex: &str) -> Script {
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<u8>>();
        Script::from(bytes)
    }

    /// Test vectors cross-checked against an independent Python implementation
    /// (base58check + BIP-173/350) of the Nusacoin chainparams.
    #[test]
    fn nusacoin_address_encoding() {
        let h160 = "00112233445566778899aabbccddeeff00112233";
        let p32 = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

        // P2PKH (version byte 0x35)
        let script = hex_script(&format!("76a914{}88ac", h160));
        assert_eq!(
            script.to_address_str(Network::Nusacoin).as_deref(),
            Some("NKvKiDevJSHSuQRzNso6nsNyMHyHGE87Uf")
        );
        // P2SH (version byte 0x4b)
        let script = hex_script(&format!("a914{}87", h160));
        assert_eq!(
            script.to_address_str(Network::Nusacoin).as_deref(),
            Some("XBMbNcDFvQUitwVtv787TdNHCPe2qZjMJk")
        );
        // P2WPKH (bech32, HRP "nu")
        let script = hex_script(&format!("0014{}", h160));
        assert_eq!(
            script.to_address_str(Network::Nusacoin).as_deref(),
            Some("nu1qqqgjyv6y24n80zye42aueh0wluqpzg3n3f9x3k")
        );
        // P2WSH (bech32, HRP "nu")
        let script = hex_script(&format!("0020{}", p32));
        assert_eq!(
            script.to_address_str(Network::Nusacoin).as_deref(),
            Some("nu1qqqgjyv6y24n80zye42aueh0wluqpzg3ng32kvaugnx4thnxaamls2z0l9q")
        );
        // P2TR (bech32m, HRP "nu")
        let script = hex_script(&format!("5120{}", p32));
        assert_eq!(
            script.to_address_str(Network::Nusacoin).as_deref(),
            Some("nu1pqqgjyv6y24n80zye42aueh0wluqpzg3ng32kvaugnx4thnxaamlsq40kau")
        );
        // Non-standard scripts have no address encoding.
        let script = hex_script("6a046e757861"); // OP_RETURN "nuxa"
        assert_eq!(script.to_address_str(Network::Nusacoin), None);
    }

    /// Address strings must parse back to the script they were encoded from.
    #[test]
    fn nusacoin_address_roundtrip() {
        use crate::util::nusacoin_address_to_script;

        let cases = [
            (
                "76a91400112233445566778899aabbccddeeff0011223388ac",
                "NKvKiDevJSHSuQRzNso6nsNyMHyHGE87Uf",
            ),
            (
                "a91400112233445566778899aabbccddeeff0011223387",
                "XBMbNcDFvQUitwVtv787TdNHCPe2qZjMJk",
            ),
            (
                "001400112233445566778899aabbccddeeff00112233",
                "nu1qqqgjyv6y24n80zye42aueh0wluqpzg3n3f9x3k",
            ),
            (
                "002000112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
                "nu1qqqgjyv6y24n80zye42aueh0wluqpzg3ng32kvaugnx4thnxaamls2z0l9q",
            ),
            (
                "512000112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
                "nu1pqqgjyv6y24n80zye42aueh0wluqpzg3ng32kvaugnx4thnxaamlsq40kau",
            ),
        ];
        for (script_hex, addr) in cases {
            let script = hex_script(script_hex);
            // encode
            assert_eq!(script.to_address_str(Network::Nusacoin).as_deref(), Some(addr));
            // parse back
            let parsed =
                nusacoin_address_to_script(addr, Network::Nusacoin).expect("address must parse");
            assert_eq!(parsed, script, "roundtrip failed for {}", addr);
        }
        // Wrong-network addresses must be rejected.
        assert_eq!(
            nusacoin_address_to_script("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", Network::Nusacoin),
            None
        ); // Bitcoin P2PKH
        assert_eq!(
            nusacoin_address_to_script(
                "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
                Network::Nusacoin
            ),
            None
        ); // Bitcoin bech32
        // Mainnet addresses must be rejected on testnet and vice versa.
        assert_eq!(
            nusacoin_address_to_script(
                "NKvKiDevJSHSuQRzNso6nsNyMHyHGE87Uf",
                Network::NusacoinTestnet
            ),
            None
        );
        assert_eq!(
            nusacoin_address_to_script(
                "nu1qqqgjyv6y24n80zye42aueh0wluqpzg3n3f9x3k",
                Network::NusacoinTestnet
            ),
            None
        );
    }

    /// Testnet vectors cross-checked against an independent Python implementation
    /// (base58check + BIP-173/350) of the Nusacoin testnet chainparams
    /// (P2PKH version 111, P2SH version 196, bech32 HRP "tn").
    #[test]
    fn nusacoin_testnet_address_encoding() {
        let h160 = "00112233445566778899aabbccddeeff00112233";
        let p32 = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

        let cases = [
            // P2PKH (version byte 111)
            (
                format!("76a914{}88ac", h160),
                "mfXJpWwdUuAGLYX1oD7av9AbrYuzUZSGHb",
            ),
            // P2SH (version byte 196)
            (
                format!("a914{}87", h160),
                "2MsFaWkH7qEYjrPNNssRg8oJUMRpAs4D6Ps",
            ),
            // P2WPKH (bech32, HRP "tn")
            (
                format!("0014{}", h160),
                "tn1qqqgjyv6y24n80zye42aueh0wluqpzg3naadcum",
            ),
            // P2WSH (bech32, HRP "tn")
            (
                format!("0020{}", p32),
                "tn1qqqgjyv6y24n80zye42aueh0wluqpzg3ng32kvaugnx4thnxaamlstnghvq",
            ),
            // P2TR (bech32m, HRP "tn")
            (
                format!("5120{}", p32),
                "tn1pqqgjyv6y24n80zye42aueh0wluqpzg3ng32kvaugnx4thnxaamlspyg75u",
            ),
        ];
        for (script_hex, addr) in cases {
            let script = hex_script(&script_hex);
            // encode
            assert_eq!(
                script.to_address_str(Network::NusacoinTestnet).as_deref(),
                Some(addr)
            );
            // parse back
            let parsed = nusacoin_address_to_script(addr, Network::NusacoinTestnet)
                .expect("address must parse");
            assert_eq!(parsed, script, "roundtrip failed for {}", addr);
        }
    }
}
