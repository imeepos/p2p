//! 策略表加载：缺失文件 = 首启正常（空表，默认拒绝，留告警）；
//! 损坏/版本不符 = 显式报错拒启（acp-common 语义，禁止静默回退空表）。

use std::path::Path;

use acp_common::{PolicyStoreError, PolicyTable};

pub fn load(path: &Path) -> Result<PolicyTable, PolicyStoreError> {
    match PolicyTable::load(path) {
        Ok(table) => Ok(table),
        Err(PolicyStoreError::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(
                path = %path.display(),
                "policy file absent; starting with empty table (default deny)",
            );
            Ok(PolicyTable::new())
        }
        Err(other) => Err(other),
    }
}
