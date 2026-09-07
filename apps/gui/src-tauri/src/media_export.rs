//! 聊天媒体导出（契约 §12 加法，2026-09-07）：附件经系统保存对话框选目标后
//! 后台拷贝落盘，进度经 Channel 流式回报。MediaContent 下载按钮改走本命令，
//! 规避 webview 把 asset 二进制当文档打开/产生 .crdownload 半成品。

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::state::AppState;

/// 进度事件载荷（Channel → 前端 onProgress）。
#[derive(Debug, Serialize)]
pub struct MediaExportProgress {
    pub received_bytes: u64,
    pub total_bytes: u64,
}

/// 导出完成回执：destPath 供前端提示保存位置。
#[derive(Debug, Serialize)]
pub struct MediaExportResult {
    pub dest_path: String,
    pub total_bytes: u64,
}

/// 进度回报节流：每 256KiB 一次（64MiB 上限 → 最多约 256 个事件，不刷爆 IPC）。
const PROGRESS_STEP: u64 = 256 * 1024;
/// 拷贝块大小。
const COPY_CHUNK: usize = 64 * 1024;

/// asset URL（asset:// 或 Windows http://asset.localhost）还原本端落盘绝对路径。
/// 与 util::to_asset_url 互为逆变换；本端落盘路径是唯一事实来源，前端只持 URL。
pub fn source_path_from_asset_url(url: &str) -> Result<PathBuf, String> {
    const ASSET: &str = "asset://localhost/";
    const WIN_ASSET: &str = "http://asset.localhost/";
    let encoded = url
        .strip_prefix(ASSET)
        .or_else(|| url.strip_prefix(WIN_ASSET))
        .ok_or_else(|| format!("非 asset 协议地址，无法导出: {url}"))?;
    if encoded.is_empty() {
        return Err("asset URL 缺少路径段".into());
    }
    let bytes = percent_decode(encoded)?;
    String::from_utf8(bytes)
        .map(PathBuf::from)
        .map_err(|e| format!("asset URL 路径非 UTF-8: {e}"))
}

/// encodeURIComponent 逆变换：%XX 十六进制 → 字节，其余字节透传。
fn percent_decode(s: &str) -> Result<Vec<u8>, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes
                .get(i + 1..i + 3)
                .ok_or_else(|| format!("截断的 % 转义: {s}"))?;
            let hex = std::str::from_utf8(hex).map_err(|e| format!("非法 % 转义: {e}"))?;
            let value =
                u8::from_str_radix(hex, 16).map_err(|_| format!("非法十六进制转义: %{hex}"))?;
            out.push(value);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(out)
}

/// 来源必须位于应用数据目录内（防前端传任意路径把盘上文件拷走）；
/// canonicalize 顺带解析符号链接，防越界路径借道绕过。
fn ensure_within_data_dir(data_dir: &Path, source: &Path) -> Result<PathBuf, String> {
    let base = data_dir
        .canonicalize()
        .map_err(|e| format!("定位应用数据目录失败: {e}"))?;
    let resolved = source
        .canonicalize()
        .map_err(|e| format!("附件文件不存在或不可访问: {e}"))?;
    if resolved.starts_with(&base) {
        Ok(resolved)
    } else {
        Err(format!(
            "导出来源越界（须位于应用数据目录内）: {}",
            resolved.display()
        ))
    }
}

/// 导出核心（与 Tauri 运行时解耦，便于直测）：来源校验 → 分块拷贝 → 进度回报。
/// 收尾必发一次终态进度（小文件可能全程不达节流阈值）。
pub async fn export_media(
    data_dir: PathBuf,
    source_url: String,
    dest_path: String,
    on_progress: impl Fn(MediaExportProgress),
) -> Result<MediaExportResult, String> {
    let source = ensure_within_data_dir(&data_dir, &source_path_from_asset_url(&source_url)?)?;
    let total = tokio::fs::metadata(&source)
        .await
        .map_err(|e| format!("读取附件元数据失败: {e}"))?
        .len();
    let mut reader = tokio::fs::File::open(&source)
        .await
        .map_err(|e| format!("打开附件失败: {e}"))?;
    let mut writer = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| format!("创建目标文件失败（{dest_path}）: {e}"))?;
    let mut chunk = vec![0u8; COPY_CHUNK];
    let mut received: u64 = 0;
    let mut reported: u64 = 0;
    loop {
        let n = reader
            .read(&mut chunk)
            .await
            .map_err(|e| format!("读取附件失败: {e}"))?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&chunk[..n])
            .await
            .map_err(|e| format!("写入目标文件失败: {e}"))?;
        received += u64::try_from(n).unwrap_or(u64::MAX);
        if received - reported >= PROGRESS_STEP {
            on_progress(MediaExportProgress {
                received_bytes: received,
                total_bytes: total,
            });
            reported = received;
        }
    }
    writer
        .flush()
        .await
        .map_err(|e| format!("落盘目标文件失败: {e}"))?;
    on_progress(MediaExportProgress {
        received_bytes: received,
        total_bytes: total,
    });
    Ok(MediaExportResult {
        dest_path,
        total_bytes: total,
    })
}

/// chat_media_export：导出聊天附件到用户经保存对话框选择的目标路径；
/// sourceUrl 为 chat_media_file/group_media_file 返回的 asset URL。
#[tauri::command]
pub async fn chat_media_export(
    state: State<'_, AppState>,
    source_url: String,
    dest_path: String,
    on_progress: Channel<MediaExportProgress>,
) -> Result<MediaExportResult, String> {
    let data_dir = state.data_dir().to_path_buf();
    export_media(data_dir, source_url, dest_path, move |p| {
        // 回报失败（前端已离开）仅告警不中断：拷贝任务本身照常完成
        if let Err(e) = on_progress.send(p) {
            tracing::warn!("媒体导出进度回报失败（前端可能已离开）: {e}");
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 临时目录守卫：退出清理，删不掉留告警不 panic（不掩盖真失败原因）。
    struct DirGuard(PathBuf);

    impl Drop for DirGuard {
        fn drop(&mut self) {
            if let Err(e) = std::fs::remove_dir_all(&self.0) {
                eprintln!("[media-export] 清理临时目录失败 {}: {e}", self.0.display());
            }
        }
    }

    fn temp_root(tag: &str) -> (PathBuf, DirGuard) {
        let dir = std::env::temp_dir().join(format!(
            "media-export-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let guard = DirGuard(dir.clone());
        (dir, guard)
    }

    #[test]
    fn asset_url_roundtrip_keeps_unicode_and_spaces() {
        let path = "/data/chat/media/PeerA/m1_照片 名.png";
        let url = crate::util::to_asset_url(path);
        let decoded = source_path_from_asset_url(&url).expect("解码成功");
        assert_eq!(decoded, PathBuf::from(path));
    }

    #[test]
    fn windows_asset_url_prefix_also_decodes() {
        let encoded = "data%2Fchat%2Fmedia%2Fa.png";
        let url = format!("http://asset.localhost/{encoded}");
        assert_eq!(
            source_path_from_asset_url(&url).expect("解码成功"),
            PathBuf::from("data/chat/media/a.png")
        );
    }

    #[test]
    fn rejects_non_asset_prefix_and_truncated_escape() {
        assert!(source_path_from_asset_url("https://example.com/a.png").is_err());
        assert!(source_path_from_asset_url("asset://localhost/").is_err());
        assert!(source_path_from_asset_url("asset://localhost/%E4%BC").is_err());
        assert!(source_path_from_asset_url("asset://localhost/%ZZ.png").is_err());
    }

    #[tokio::test]
    async fn export_copies_bytes_and_reports_progress() {
        let (root, _guard) = temp_root("copy");
        let media_dir = root.join("chat/media/PeerA");
        std::fs::create_dir_all(&media_dir).expect("建媒体目录");
        // 300KB 载荷：跨过 256KiB 节流阈值，保证节流进度与终态进度都有
        let payload: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let source = media_dir.join("m1.bin");
        std::fs::write(&source, &payload).expect("写附件");
        let dest = root.join("export/m1-save.bin");
        std::fs::create_dir_all(dest.parent().expect("有父目录")).expect("建导出目录");

        let events = std::sync::Mutex::new(Vec::new());
        let url = crate::util::to_asset_url(&source.to_string_lossy());
        let result = export_media(
            root.clone(),
            url,
            dest.to_string_lossy().into_owned(),
            |p| events.lock().expect("锁").push(p),
        )
        .await
        .expect("导出成功");

        assert_eq!(result.total_bytes, payload.len() as u64);
        assert_eq!(std::fs::read(&dest).expect("读导出产物"), payload);
        let events = events.into_inner().expect("锁");
        let last = events.last().expect("至少终态一次");
        assert_eq!(last.received_bytes, payload.len() as u64);
        assert!(events.len() >= 2, "跨阈值应有节流进度: {events:?}");
    }

    #[tokio::test]
    async fn export_rejects_outside_data_dir_and_bad_source() {
        let (root, _guard) = temp_root("reject");
        let outside = root.parent().expect("有父目录").join("outside.bin");
        std::fs::write(&outside, b"x").expect("写外部文件");
        let url = crate::util::to_asset_url(&outside.to_string_lossy());
        let err = export_media(
            root.clone(),
            url,
            root.join("o.bin").to_string_lossy().into_owned(),
            |_| {},
        )
        .await
        .expect_err("越界拒绝");
        assert!(err.contains("越界"), "实际: {err}");

        let err = export_media(root, "https://example.com/a.png".into(), "d".into(), |_| {})
            .await
            .expect_err("非 asset 前缀拒绝");
        assert!(err.contains("asset"), "实际: {err}");
    }
}
