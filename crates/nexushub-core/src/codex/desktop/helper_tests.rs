use super::read_bounded_line;
use crate::{codex::desktop::DesktopError, resource_limits::CODEX_DESKTOP_FRAME_BYTES};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, BufReader, ReadBuf};

const GENERATED_CHUNK_BYTES: usize = 8 * 1024;

struct GeneratedReader {
    prefix: &'static [u8],
    prefix_offset: usize,
    fill_byte: u8,
    fill_remaining: usize,
    suffix: &'static [u8],
    suffix_offset: usize,
}

impl GeneratedReader {
    fn new(prefix: &'static [u8], fill_byte: u8, fill_len: usize, suffix: &'static [u8]) -> Self {
        Self {
            prefix,
            prefix_offset: 0,
            fill_byte,
            fill_remaining: fill_len,
            suffix,
            suffix_offset: 0,
        }
    }
}

impl AsyncRead for GeneratedReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if output.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }

        if this.prefix_offset < this.prefix.len() {
            let end = (this.prefix_offset + output.remaining()).min(this.prefix.len());
            output.put_slice(&this.prefix[this.prefix_offset..end]);
            this.prefix_offset = end;
        } else if this.fill_remaining > 0 {
            let length = output
                .remaining()
                .min(this.fill_remaining)
                .min(GENERATED_CHUNK_BYTES);
            let chunk = [this.fill_byte; GENERATED_CHUNK_BYTES];
            output.put_slice(&chunk[..length]);
            this.fill_remaining -= length;
        } else if this.suffix_offset < this.suffix.len() {
            let end = (this.suffix_offset + output.remaining()).min(this.suffix.len());
            output.put_slice(&this.suffix[this.suffix_offset..end]);
            this.suffix_offset = end;
        }

        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn read_bounded_line_rejects_invalid_json() {
    let mut reader = BufReader::new(std::io::Cursor::new(&b"{not json}\n"[..]));
    assert!(matches!(
        read_bounded_line(&mut reader).await,
        Err(DesktopError::InvalidFrame)
    ));
}

#[tokio::test]
async fn read_bounded_line_requires_a_complete_newline_terminated_frame() {
    let mut reader = BufReader::new(std::io::Cursor::new(&b"{\"id\":1}"[..]));
    assert!(matches!(
        read_bounded_line(&mut reader).await,
        Err(DesktopError::Disconnected)
    ));
}

#[tokio::test]
async fn read_bounded_line_accepts_exactly_the_64_mib_limit() {
    let fill_len = CODEX_DESKTOP_FRAME_BYTES - 3;
    let generated = GeneratedReader::new(b"{", b' ', fill_len, b"}\n");
    let mut reader = BufReader::new(generated);

    assert_eq!(
        read_bounded_line(&mut reader)
            .await
            .expect("exactly bounded JSON line is accepted"),
        serde_json::json!({})
    );
}

#[tokio::test]
async fn read_bounded_line_rejects_one_byte_over_the_64_mib_limit() {
    let generated = GeneratedReader::new(b"{", b' ', CODEX_DESKTOP_FRAME_BYTES, b"}\n");
    let mut reader = BufReader::new(generated);

    assert!(matches!(
        read_bounded_line(&mut reader).await,
        Err(DesktopError::FrameTooLarge)
    ));
}
