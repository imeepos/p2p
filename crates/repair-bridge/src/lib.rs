//! Repair bridge byte-pump and protocol contract.

use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL_ID: &str = "/repair/mcp/1";
pub const IO_CHUNK_SIZE: usize = p2p_protocol::MAX_FRAME_SIZE as usize;

/// Copies raw stdin bytes as bounded protocol frames.
pub async fn stdin_to_stream<R, W>(mut input: R, mut stream: W) -> io::Result<()>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    let mut buf = vec![0u8; IO_CHUNK_SIZE];
    loop {
        let n = input.read(&mut buf).await?;
        if n == 0 {
            stream.shutdown().await?;
            return Ok(());
        }
        p2p_protocol::write_frame(&mut stream, &buf[..n]).await?;
        stream.flush().await?;
    }
}

/// Copies bounded stream frames unchanged to stdout.
pub async fn stream_to_stdout<R, W>(mut stream: R, mut output: W) -> io::Result<()>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    loop {
        let frame = p2p_protocol::read_frame(&mut stream).await?;
        output.write_all(&frame).await?;
        output.flush().await?;
    }
}

/// Runs both independent directions and reports the first completed direction.
pub async fn pump<R1, W1, R2, W2>(
    stdin: R1,
    stream_write: W1,
    stream_read: R2,
    stdout: W2,
) -> io::Result<()>
where
    R1: AsyncRead + Unpin + Send + 'static,
    W1: AsyncWrite + Unpin + Send + 'static,
    R2: AsyncRead + Unpin + Send + 'static,
    W2: AsyncWrite + Unpin + Send + 'static,
{
    let mut left = tokio::spawn(stdin_to_stream(stdin, stream_write));
    let mut right = tokio::spawn(stream_to_stdout(stream_read, stdout));
    let result = tokio::select! {
        value = &mut left => join_result(value, "stdin direction task failed"),
        value = &mut right => join_result(value, "stdout direction task failed"),
    };
    if !left.is_finished() {
        left.abort();
    }
    if !right.is_finished() {
        right.abort();
    }
    result.and_then(|_| {
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "bridge direction closed",
        ))
    })
}

fn join_result(
    result: Result<io::Result<()>, tokio::task::JoinError>,
    context: &str,
) -> io::Result<()> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{context}: {error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn large_payload_is_split_and_preserved() {
        let payload = vec![7u8; IO_CHUNK_SIZE + 17];
        let (mut source_w, source_r) = duplex(payload.len() + 1);
        let (stream_w, mut stream_r) = duplex(IO_CHUNK_SIZE * 2 + 64);
        let expected = payload.clone();
        let writer = tokio::spawn(async move { source_w.write_all(&expected).await });
        stdin_to_stream(source_r, stream_w).await.unwrap();
        writer.await.unwrap().unwrap();
        let first = p2p_protocol::read_frame(&mut stream_r).await.unwrap();
        let second = p2p_protocol::read_frame(&mut stream_r).await.unwrap();
        assert_eq!([first, second].concat(), payload);
    }

    #[tokio::test]
    async fn stream_frames_are_copied_without_changes() {
        let (mut input_w, input_r) = duplex(128);
        let (output_w, mut output_r) = duplex(128);
        let data = vec![1, 2, 3, 4];
        p2p_protocol::write_frame(&mut input_w, &data)
            .await
            .unwrap();
        drop(input_w);
        let task = tokio::spawn(stream_to_stdout(input_r, output_w));
        let mut copied = Vec::new();
        output_r.read_to_end(&mut copied).await.unwrap();
        assert_eq!(copied, data);
        assert!(task.await.unwrap().is_err());
    }
}
