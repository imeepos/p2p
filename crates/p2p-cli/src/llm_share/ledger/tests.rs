use super::*;
use llm_share_ledger::{Receipt, Usage};

fn receipt(req_id: &str, lender: &str, borrower: &str, period: &str, tokens: u64) -> Receipt {
    Receipt {
        v: 1,
        req_id: req_id.to_owned(),
        period: period.to_owned(),
        lender: lender.to_owned(),
        borrower: borrower.to_owned(),
        model: "gpt-4o".to_owned(),
        usage: Usage {
            input: tokens / 2,
            output: tokens - tokens / 2,
        },
        estimated: false,
        upstream_hint: "openai".to_owned(),
        ts: 1_000,
        sig: "x".to_owned(),
    }
}

fn peer(tag: u8) -> String {
    bs58::encode([tag; 32]).into_string()
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2pcli-ledger-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn roundtrip_and_list_filters() {
    let dir = temp_dir("list");
    let (me, other) = (peer(1), peer(2));
    {
        let mut file = LedgerFile::new();
        file.append(receipt("r1", &other, &me, "2026-09", 100));
        file.append(receipt("r2", &me, &other, "2026-09", 40));
        file.append(receipt("r3", &me, &other, "2026-08", 10));
        save(&path(dir.to_str().unwrap()), &file).unwrap();
    }
    let all = list(dir.to_str().unwrap(), LedgerFilters::default()).unwrap();
    assert_eq!(all.count, 3);
    assert_eq!(all.entries[0].req_id, "r1");
    assert_eq!(all.entries[0].tokens, 100);
    let mine = list(
        dir.to_str().unwrap(),
        LedgerFilters {
            lender: Some(&other),
            borrower: None,
            period: Some("2026-09"),
        },
    )
    .unwrap();
    assert_eq!(mine.count, 1);
    assert_eq!(mine.entries[0].req_id, "r1");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn balance_splits_by_lender_and_period() {
    let dir = temp_dir("balance");
    let (me, a, b) = (peer(1), peer(2), peer(3));
    let mut file = LedgerFile::new();
    file.append(receipt("r1", &a, &me, "2026-09", 100));
    file.append(receipt("r2", &me, &b, "2026-09", 40));
    file.append(receipt("r3", &a, &me, "2026-08", 10));
    file.append(receipt("r4", &b, &a, "2026-09", 500));
    save(&path(dir.to_str().unwrap()), &file).unwrap();
    let report = balance(dir.to_str().unwrap(), &me, None).unwrap();
    assert_eq!(report.rows.len(), 3, "无关条目 r4 不进本机视角");
    let sep = |i: usize| {
        (
            report.rows[i].lender.as_str(),
            report.rows[i].period.as_str(),
        )
    };
    assert_eq!(
        sep(0),
        (me.as_str(), "2026-09"),
        "BTreeMap 按 (lender, period) 序"
    );
    assert_eq!(report.rows[0].lent_out, 40);
    assert_eq!(report.rows[0].net, 40);
    assert_eq!(sep(1), (a.as_str(), "2026-08"));
    assert_eq!(report.rows[1].borrowed, 10);
    assert_eq!(report.rows[1].net, -10);
    assert_eq!(sep(2), (a.as_str(), "2026-09"));
    assert_eq!(report.rows[2].borrowed, 100);
    assert_eq!(report.rows[2].net, -100);
    let only_sep = balance(dir.to_str().unwrap(), &me, Some("2026-09")).unwrap();
    assert_eq!(only_sep.rows.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_ledger_is_empty_and_corrupt_errors() {
    let dir = temp_dir("empty");
    let me = peer(9);
    assert_eq!(
        balance(dir.to_str().unwrap(), &me, None)
            .unwrap()
            .rows
            .len(),
        0
    );
    std::fs::create_dir_all(path(dir.to_str().unwrap()).parent().unwrap()).unwrap();
    std::fs::write(path(dir.to_str().unwrap()), "broken").unwrap();
    assert!(list(dir.to_str().unwrap(), LedgerFilters::default()).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
