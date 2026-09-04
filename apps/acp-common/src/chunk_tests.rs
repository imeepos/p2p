use super::*;
use crate::consts::{FRAME_CHUNK_LIMIT, LINE_GUARD_LIMIT};

fn feed(re: &mut LineReassembler, line: &[u8]) -> Result<Option<Vec<u8>>, ErrorCode> {
    for frame in frames(line) {
        re.push_frame(frame)?;
    }
    Ok(re.take_line())
}

#[test]
fn small_line_roundtrip() {
    let mut re = LineReassembler::new();
    assert_eq!(
        feed(&mut re, b"hello").unwrap().as_deref(),
        Some(&b"hello\n"[..])
    );
}

#[test]
fn empty_line_passthrough() {
    let mut re = LineReassembler::new();
    assert_eq!(feed(&mut re, b"").unwrap().as_deref(), Some(&b"\n"[..]));
}

#[test]
fn frames_exact_one_mib_boundary() {
    let line = vec![b'x'; FRAME_CHUNK_LIMIT];
    let fs: Vec<&[u8]> = frames(&line).collect();
    assert_eq!(fs.len(), 2);
    assert_eq!(fs[0].len(), FRAME_CHUNK_LIMIT);
    assert_eq!(fs[1], b"\n");
}

#[test]
fn frames_one_mib_plus_one_boundary() {
    let line = vec![b'x'; FRAME_CHUNK_LIMIT + 1];
    let fs: Vec<&[u8]> = frames(&line).collect();
    assert_eq!(fs.len(), 3);
    assert_eq!(fs[0].len(), FRAME_CHUNK_LIMIT);
    assert_eq!(fs[1].len(), 1);
    assert_eq!(fs[2], b"\n");
}

#[test]
fn reassemble_exact_one_mib_line() {
    let mut re = LineReassembler::new();
    let line = vec![b'a'; FRAME_CHUNK_LIMIT];
    let out = feed(&mut re, &line).unwrap().unwrap();
    assert_eq!(out.len(), FRAME_CHUNK_LIMIT + 1);
    assert_eq!(&out[..4], b"aaaa");
    assert_eq!(out[out.len() - 1], b'\n');
}

#[test]
fn reassemble_multibyte_utf8_split_across_chunks() {
    // "日" = E6 97 A5；总长使 1MiB 帧界落在一个字符中间
    let mut line = Vec::new();
    while line.len() < FRAME_CHUNK_LIMIT + 1 {
        line.extend_from_slice("日".as_bytes());
    }
    assert_eq!(line[FRAME_CHUNK_LIMIT], 0x97, "boundary must cut mid-char");
    let mut re = LineReassembler::new();
    let mut out = feed(&mut re, &line).unwrap().unwrap();
    out.truncate(out.len() - 1);
    assert_eq!(out, line);
}

#[test]
fn crlf_normalized() {
    let mut re = LineReassembler::new();
    assert_eq!(
        feed(&mut re, b"abc\r").unwrap().as_deref(),
        Some(&b"abc\n"[..])
    );
}

#[test]
fn crlf_across_frames_normalized() {
    let mut re = LineReassembler::new();
    re.push_frame(b"abc\r").unwrap();
    re.push_frame(b"\n").unwrap();
    assert_eq!(re.take_line().as_deref(), Some(&b"abc\n"[..]));
}

#[test]
fn interior_cr_not_stripped() {
    let mut re = LineReassembler::new();
    assert_eq!(
        feed(&mut re, b"a\rb").unwrap().as_deref(),
        Some(&b"a\rb\n"[..])
    );
}

#[test]
fn incomplete_line_yields_none_then_completes() {
    let mut re = LineReassembler::new();
    re.push_frame(b"ab").unwrap();
    assert_eq!(re.take_line(), None);
    re.push_frame(b"c\n").unwrap();
    assert_eq!(re.take_line().as_deref(), Some(&b"abc\n"[..]));
}

#[test]
fn multiple_lines_in_one_frame_drain() {
    let mut re = LineReassembler::new();
    re.push_frame(b"l1\nl2\nl3\n").unwrap();
    assert_eq!(re.take_line().as_deref(), Some(&b"l1\n"[..]));
    assert_eq!(re.take_line().as_deref(), Some(&b"l2\n"[..]));
    assert_eq!(re.take_line().as_deref(), Some(&b"l3\n"[..]));
    assert_eq!(re.take_line(), None);
}

#[test]
fn line_at_guard_limit_passes() {
    let mut re = LineReassembler::new();
    // content = guard-1，frames() 自动补行尾 '\n' → 行总长恰 = guard
    let content = vec![b'x'; LINE_GUARD_LIMIT - 1];
    for frame in frames(&content) {
        re.push_frame(frame).unwrap();
    }
    assert_eq!(re.take_line().map(|l| l.len()), Some(LINE_GUARD_LIMIT));
    assert_eq!(re.finish(), Ok(()));
}

#[test]
fn over_guard_line_breaks_stream_with_limit() {
    let mut re = LineReassembler::new();
    // 末帧（frames() 自动补的行尾 '\n'）压线超 guard → 断流
    let content = vec![b'x'; LINE_GUARD_LIMIT];
    let all: Vec<&[u8]> = frames(&content).collect();
    let (head, last) = all.split_at(all.len() - 1);
    for frame in head {
        re.push_frame(frame).unwrap();
    }
    let err = re.push_frame(last[0]).unwrap_err();
    assert_eq!(
        err,
        ErrorCode::LineTooLong {
            limit_bytes: LINE_GUARD_LIMIT
        }
    );
    assert_eq!(err.code(), "line-too-long");
    // 断流毒化：继续喂快速失败，缓冲不复活
    assert_eq!(
        re.push_frame(b"x"),
        Err(ErrorCode::LineTooLong {
            limit_bytes: LINE_GUARD_LIMIT
        })
    );
    assert_eq!(re.take_line(), None);
    assert_eq!(
        re.poisoned(),
        Some(ErrorCode::LineTooLong {
            limit_bytes: LINE_GUARD_LIMIT
        })
    );
}

#[test]
fn oversize_frame_rejected_defensively() {
    let mut re = LineReassembler::new();
    let big = vec![0u8; FRAME_CHUNK_LIMIT + 1];
    let err = re.push_frame(&big).unwrap_err();
    assert_eq!(
        err,
        ErrorCode::FrameTooLarge {
            size_bytes: FRAME_CHUNK_LIMIT + 1,
            limit_bytes: FRAME_CHUNK_LIMIT
        }
    );
    assert!(re.poisoned().is_some());
}

#[test]
fn finish_flags_unterminated_tail() {
    let mut re = LineReassembler::new();
    re.push_frame(b"partial").unwrap();
    assert_eq!(re.take_line(), None);
    assert_eq!(re.finish(), Err(ErrorCode::NdjsonTruncated));

    let mut re = LineReassembler::new();
    re.push_frame(b"ok\n").unwrap();
    assert_eq!(re.take_line().as_deref(), Some(&b"ok\n"[..]));
    assert_eq!(re.finish(), Ok(()));
}
