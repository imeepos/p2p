//! 录屏最小闭环：定时采样帧 → GIF 编码原子落盘（格式不限，选 GIF 免外部依赖）。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use super::capture::{write_atomic, Frame, FrameSource};

/// 帧数上限：默认间隔约 2.5 分钟，防内存失控（超出 truncated=true 可观测）。
const MAX_FRAMES: usize = 300;
/// 录屏帧最长边（像素）：GIF 体积控制。
const MAX_SIDE: u32 = 640;

pub struct RecordStats {
    pub path: String,
    pub frames: usize,
    pub bytes: u64,
    pub truncated: bool,
}

pub struct RecordSession {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<RecordStats>>,
    path: PathBuf,
}

impl RecordSession {
    pub fn start(frame: Arc<dyn FrameSource>, path: PathBuf, interval_ms: u64) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建录屏输出目录失败: {e}"))?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let sample_path = path.clone();
        let worker = std::thread::Builder::new()
            .name("gc1-record".to_string())
            .spawn({
                let stop = stop.clone();
                move || sample_loop(stop, frame, sample_path, interval_ms)
            })
            .map_err(|e| format!("录屏线程启动失败: {e}"))?;
        Ok(Self { stop, worker: Some(worker), path })
    }

    /// 收尾：置停止位等采样线程编码落盘；join 失败记零产出统计（可观测）。
    pub fn finalize(mut self) -> RecordStats {
        self.stop.store(true, Ordering::SeqCst);
        let path = self.path.display().to_string();
        match self.worker.take() {
            Some(handle) => handle.join().unwrap_or_else(|_| {
                tracing::error!("control: 录屏线程 panic，按零产出统计");
                RecordStats { path, frames: 0, bytes: 0, truncated: true }
            }),
            None => RecordStats { path, frames: 0, bytes: 0, truncated: true },
        }
    }
}

fn sample_loop(
    stop: Arc<AtomicBool>,
    frame: Arc<dyn FrameSource>,
    path: PathBuf,
    interval_ms: u64,
) -> RecordStats {
    let mut frames: Vec<Frame> = Vec::new();
    let mut truncated = false;
    while !stop.load(Ordering::SeqCst) {
        if frames.len() >= MAX_FRAMES {
            truncated = true;
            tracing::warn!("control: 录屏达到 {MAX_FRAMES} 帧上限，提前收尾（仍产出文件）");
            break;
        }
        match frame.capture() {
            Ok(f) => frames.push(f),
            Err(e) => {
                tracing::warn!("control: 录屏采样失败 {} {}（已采 {} 帧）", e.code, e.message, frames.len());
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(interval_ms));
    }
    if frames.is_empty() {
        tracing::error!("control: 录屏零帧（采样即失败），不落空文件");
        return RecordStats { path: path.display().to_string(), frames: 0, bytes: 0, truncated: true };
    }
    let downscaled: Vec<Frame> = frames.iter().map(|f| downscale(f, MAX_SIDE)).collect();
    let written = encode_gif(&downscaled, interval_ms).and_then(|bytes| write_atomic(&path, &bytes));
    match written {
        Ok(bytes) => RecordStats {
            path: path.display().to_string(),
            frames: downscaled.len(),
            bytes,
            truncated,
        },
        Err(e) => {
            tracing::error!("control: 录屏落盘失败: {e}");
            RecordStats {
                path: path.display().to_string(),
                frames: downscaled.len(),
                bytes: 0,
                truncated: true,
            }
        }
    }
}

fn encode_gif(frames: &[Frame], interval_ms: u64) -> Result<Vec<u8>, String> {
    let first = frames.first().ok_or_else(|| "无帧可编码".to_string())?;
    let mut out = Vec::new();
    let mut encoder = gif::Encoder::new(&mut out, first.width as u16, first.height as u16, &[])
        .map_err(|e| format!("GIF 初始化失败: {e}"))?;
    let _ = encoder.set_repeat(gif::Repeat::Infinite);
    let delay = (interval_ms / 10).max(1) as u16;
    for frame in frames {
        let mut rgba = frame.rgba.clone();
        let mut gframe = gif::Frame::from_rgba_speed(
            frame.width as u16,
            frame.height as u16,
            &mut rgba,
            30,
        );
        gframe.delay = delay;
        encoder.write_frame(&gframe).map_err(|e| format!("GIF 写帧失败: {e}"))?;
    }
    drop(encoder);
    Ok(out)
}

/// 最近邻降采样，控 GIF 体积；已小于上限则原样克隆。
fn downscale(frame: &Frame, max_side: u32) -> Frame {
    let w = frame.width.max(1);
    let h = frame.height.max(1);
    if w <= max_side && h <= max_side {
        return frame.clone();
    }
    let scale = f64::from(max_side) / f64::from(w.max(h));
    let nw = ((f64::from(w) * scale) as u32).max(1);
    let nh = ((f64::from(h) * scale) as u32).max(1);
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        let sy = (y * h / nh) as usize;
        for x in 0..nw {
            let sx = (x * w / nw) as usize;
            let src = (sy * w as usize + sx) * 4;
            let dst = ((y * nw + x) * 4) as usize;
            out[dst..dst + 4].copy_from_slice(&frame.rgba[src..src + 4]);
        }
    }
    Frame { width: nw, height: nh, rgba: out }
}
