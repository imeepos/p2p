//! 收据离线验签（§5.1，MVP A3）：指定收据文件 + 出借方公钥（base58），
//! 先验公钥-lender 绑定（PeerId = sha256(pubkey)），再 Ed25519 验签。
//! PASS/FAIL 双路径均结构化输出；文件缺失/损坏/公钥格式非法为显式错误。

use std::path::Path;

use llm_share_ledger::Receipt;
use p2p_identity::PeerId;
use serde::Serialize;

use super::read_json_or_none;

/// 收据验签报告：verdict 恒为 PASS 或 FAIL，FAIL 时 reason 给出可述原因。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptVerifyReport {
    pub verdict: String,
    pub reason: String,
    pub req_id: String,
    pub period: String,
    pub lender: String,
    pub borrower: String,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub estimated: bool,
    pub ts: u64,
}

/// 验签主流程：读文件 → 解公钥 → 绑定校验 → Ed25519 验签。
/// 返回 Err 仅限前置失败（文件/公钥不可用）；签名层面的失败走 FAIL 报告。
pub fn verify_file(path: &Path, pubkey: &str) -> Result<ReceiptVerifyReport, String> {
    let receipt: Receipt = match read_json_or_none(path, "收据")? {
        Some(receipt) => receipt,
        None => return Err(format!("收据文件不存在（{}）", path.display())),
    };
    let pubkey = decode_pubkey(pubkey)?;
    let binding = PeerId::from_public_key(&pubkey).to_string();
    if binding != receipt.lender {
        return Ok(report(
            &receipt,
            "FAIL",
            format!(
                "公钥与 lender 不绑定：该公钥对应 PeerId {binding}，收据 lender 为 {}",
                receipt.lender
            ),
        ));
    }
    match receipt.verify(&pubkey) {
        Ok(()) => Ok(report(&receipt, "PASS", "验签通过".to_owned())),
        Err(e) => Ok(report(&receipt, "FAIL", format!("验签失败: {e}"))),
    }
}

/// 出借方公钥（base58，解码后恰 32 字节）。
fn decode_pubkey(pubkey: &str) -> Result<[u8; 32], String> {
    let raw = bs58::decode(pubkey)
        .into_vec()
        .map_err(|_| format!("公钥非法（不是合法 base58）：{pubkey}"))?;
    let len = raw.len();
    let bytes: [u8; 32] = raw
        .try_into()
        .map_err(|_| format!("公钥非法（解码后应恰 32 字节，实得 {len}）"))?;
    Ok(bytes)
}

fn report(receipt: &Receipt, verdict: &str, reason: String) -> ReceiptVerifyReport {
    ReceiptVerifyReport {
        verdict: verdict.to_owned(),
        reason,
        req_id: receipt.req_id.clone(),
        period: receipt.period.clone(),
        lender: receipt.lender.clone(),
        borrower: receipt.borrower.clone(),
        model: receipt.model.clone(),
        input: receipt.usage.input,
        output: receipt.usage.output,
        estimated: receipt.estimated,
        ts: receipt.ts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_share_ledger::{Receipt, Usage};

    fn signed_receipt() -> (p2p_identity::Keypair, Receipt) {
        let keypair = p2p_identity::Keypair::generate();
        let mut receipt = Receipt {
            v: 1,
            req_id: "req-1".to_owned(),
            period: "2026-09".to_owned(),
            lender: keypair.peer_id().to_string(),
            borrower: bs58::encode([9u8; 32]).into_string(),
            model: "gpt-4o".to_owned(),
            usage: Usage {
                input: 1234,
                output: 5678,
            },
            estimated: false,
            upstream_hint: "openai".to_owned(),
            ts: 1_725_400_000,
            sig: String::new(),
        };
        receipt.sign(&keypair).unwrap();
        (keypair, receipt)
    }

    fn write_receipt(dir: &Path, receipt: &Receipt) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let file = dir.join("receipt.json");
        std::fs::write(&file, serde_json::to_string_pretty(receipt).unwrap()).unwrap();
        file
    }

    #[test]
    fn valid_receipt_verifies_pass() {
        let dir = std::env::temp_dir().join(format!("p2pcli-receipt-pass-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (keypair, receipt) = signed_receipt();
        let file = write_receipt(&dir, &receipt);
        let report = verify_file(&file, &bs58::encode(keypair.public()).into_string()).unwrap();
        assert_eq!(report.verdict, "PASS");
        assert_eq!(report.input, 1234);
        assert_eq!(report.output, 5678);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tampered_receipt_fails_with_reason() {
        let dir =
            std::env::temp_dir().join(format!("p2pcli-receipt-tamper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (keypair, mut receipt) = signed_receipt();
        receipt.usage.output += 1;
        let file = write_receipt(&dir, &receipt);
        let report = verify_file(&file, &bs58::encode(keypair.public()).into_string()).unwrap();
        assert_eq!(report.verdict, "FAIL");
        assert!(report.reason.contains("验签失败"), "{}", report.reason);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mismatched_pubkey_fails_binding_check() {
        let dir = std::env::temp_dir().join(format!("p2pcli-receipt-bind-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (_, receipt) = signed_receipt();
        let file = write_receipt(&dir, &receipt);
        let stranger = p2p_identity::Keypair::generate();
        let report = verify_file(&file, &bs58::encode(stranger.public()).into_string()).unwrap();
        assert_eq!(report.verdict, "FAIL");
        assert!(report.reason.contains("不绑定"), "{}", report.reason);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_inputs_are_explicit_errors() {
        let dir = std::env::temp_dir().join(format!("p2pcli-receipt-err-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let missing = verify_file(
            &dir.join("absent.json"),
            &bs58::encode([1u8; 32]).into_string(),
        );
        assert!(missing.unwrap_err().contains("不存在"));
        let (_, receipt) = signed_receipt();
        let file = write_receipt(&dir, &receipt);
        assert!(verify_file(&file, "not-base58!")
            .unwrap_err()
            .contains("base58"));
        assert!(verify_file(&file, &bs58::encode([1u8; 8]).into_string())
            .unwrap_err()
            .contains("32 字节"));
        std::fs::write(&file, "{ broken").unwrap();
        assert!(verify_file(&file, &bs58::encode([1u8; 32]).into_string())
            .unwrap_err()
            .contains("收据"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
