//! 统一形状门禁：8 个向量集都必须符合章程 §8 顶层形状。

mod common;
use common::{case_name, load, VECTOR_FILES};

#[test]
fn all_vector_sets_share_uniform_shape() {
    for file in VECTOR_FILES {
        let doc = load(file);
        assert_eq!(
            doc["vector_set"].as_str().unwrap_or_default(),
            file.trim_end_matches(".json"),
            "{file} vector_set 须与文件名一致"
        );
        let spec = doc["spec"].as_str().unwrap_or_default();
        assert!(!spec.is_empty(), "{file} 缺 spec 依据引用");
        let arr = doc["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{file} 缺 cases 数组"));
        assert!(!arr.is_empty(), "{file} cases 为空");
        for case in arr {
            let name = case_name(case);
            assert!(!name.is_empty(), "{file} 有 case 缺 name");
            let note = case["note"].as_str().unwrap_or_default();
            assert!(!note.is_empty(), "{file} case {name} 缺 note");
        }
    }
}

#[test]
fn vector_file_count_matches_charter_manifest() {
    assert_eq!(VECTOR_FILES.len(), 8, "章程 §8 首波清单固定 8 组");
    for file in VECTOR_FILES {
        common::load(file);
    }
}
