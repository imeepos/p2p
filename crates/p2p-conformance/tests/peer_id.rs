//! peer-id.json 消费：固定种子到 PeerId 全链推导。

mod common;
use common::{case_name, cases, load, unhex};
use p2p_identity::Keypair;
use sha2::{Digest, Sha256};

const FILE: &str = "peer-id.json";

#[test]
fn seed_to_peer_id_chain_matches_vectors() {
    let doc = load(FILE);
    for case in cases(&doc) {
        let name = case_name(case);
        let seed: [u8; 32] = unhex(case["seed_hex"].as_str().unwrap())
            .try_into()
            .expect("seed 须为 32 字节");
        let kp = Keypair::from_seed(&seed);
        let pubkey = kp.public();
        assert_eq!(
            hex_of(&pubkey),
            case["public_key_hex"].as_str().unwrap(),
            "case {name}: ed25519 公钥不符"
        );
        let digest: [u8; 32] = Sha256::digest(pubkey).into();
        assert_eq!(
            hex_of(&digest),
            case["sha256_hex"].as_str().unwrap(),
            "case {name}: SHA-256 中间值不符"
        );
        assert_eq!(
            kp.peer_id().to_string(),
            case["peer_id_base58"].as_str().unwrap(),
            "case {name}: base58 PeerId 不符"
        );
    }
}

fn hex_of(bytes: &[u8]) -> String {
    common::hex(bytes)
}
