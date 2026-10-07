//! PPDRIVE command-line interface.
//!
//! A CLI tool for managing PPDRIVE resources: creating clients, provisioning buckets,
//! launching the storage server, and editing the configuration file.

use crate::command::Cli;
use anyhow::Context;
use clap::Parser;
use std::path::Path;

mod command;
mod plugin;
mod subs;
mod uninstall;
mod update;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    cli.execute().await?;

    Ok(())
}

/// Stream a response body into `dest`.
///
/// `ureq`'s `Body::read_to_vec()` is capped at its 10MB default limit, which
/// release assets such as the media runtime bundle (16MB+) exceed;
/// `into_reader()` streams without a size limit. On any failure the partial
/// file is removed so it can never replace a good download.
fn write_body_to_file(mut reader: impl std::io::Read, dest: &Path) -> Result<(), anyhow::Error> {
    let mut write = || -> Result<(), anyhow::Error> {
        let mut file = std::fs::File::create(dest)
            .with_context(|| format!("failed to create {}", dest.display()))?;
        std::io::copy(&mut reader, &mut file).context("failed to read download body")?;
        Ok(())
    };

    if let Err(err) = write() {
        let _ = std::fs::remove_file(dest);
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write_body_to_file;
    use std::io::{Cursor, Read};

    #[test]
    fn writes_full_body() -> anyhow::Result<()> {
        let dest = std::env::temp_dir().join(format!("ppdrive-body-ok-{}", std::process::id()));
        let _ = std::fs::remove_file(&dest);

        write_body_to_file(Cursor::new(b"release asset"), &dest)?;

        assert_eq!(std::fs::read(&dest)?, b"release asset");
        let _ = std::fs::remove_file(&dest);
        Ok(())
    }

    /// A reader that yields `yield` bytes and then fails mid-stream.
    struct PartialThenFail {
        sent: usize,
        yield_bytes: usize,
    }

    impl Read for PartialThenFail {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.sent >= self.yield_bytes {
                return Err(std::io::Error::other("connection reset"));
            }
            let n = buf.len().min(self.yield_bytes - self.sent);
            buf[..n].fill(b'a');
            self.sent += n;
            Ok(n)
        }
    }

    #[test]
    fn removes_partial_file_when_reader_fails() {
        let dest = std::env::temp_dir().join(format!("ppdrive-body-fail-{}", std::process::id()));
        let _ = std::fs::remove_file(&dest);

        let reader = PartialThenFail {
            sent: 0,
            yield_bytes: 4096,
        };
        assert!(write_body_to_file(reader, &dest).is_err());

        assert!(
            !dest.exists(),
            "partial download must be removed on failure"
        );
    }
}
