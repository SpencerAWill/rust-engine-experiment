//! `prng-stats`: stream `Prng` output to stdout, for piping into a statistical test suite.
//!
//! Usage (`PractRand`'s `RNG_test` is not bundled; install it separately):
//!
//! ```text
//! cargo xtask prng-stats | RNG_test stdin64
//! cargo xtask prng-stats --seed 42 --bytes 1073741824 > sample.bin
//! ```

use std::io::{self, Write};

use engine_random::Prng;

/// Bytes generated per `fill_bytes` call. A multiple of 8, so chunking never changes the
/// stream: only the final chunk can end mid-word, exactly like one big `fill_bytes`.
pub(crate) const CHUNK: usize = 64 * 1024;

/// Writes `Prng::from_seed(seed)` output to `out` in 64 KiB chunks until `bytes` have been
/// written (or forever when `None`). A closed pipe is a normal way to stop, so it is `Ok`.
pub(crate) fn write_stream<W: Write>(out: &mut W, seed: u64, bytes: Option<u64>) -> io::Result<()> {
    let mut rng = Prng::from_seed(seed);
    let mut buf = vec![0_u8; CHUNK];
    let mut remaining = bytes;
    loop {
        let want = match remaining {
            Some(0) => return Ok(()),
            Some(left) => usize::try_from(left).map_or(CHUNK, |left| left.min(CHUNK)),
            None => CHUNK,
        };
        rng.fill_bytes(&mut buf[..want]);
        match out.write_all(&buf[..want]) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
            Err(e) => return Err(e),
        }
        if let Some(left) = &mut remaining {
            *left -= want as u64;
        }
    }
}

/// Runs the command against locked stdout.
pub(crate) fn run(seed: u64, bytes: Option<u64>) -> Result<(), String> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    write_stream(&mut lock, seed, bytes).map_err(|e| format!("prng-stats: {e}"))?;
    lock.flush().or_else(|e| {
        if e.kind() == io::ErrorKind::BrokenPipe {
            Ok(())
        } else {
            Err(format!("prng-stats: {e}"))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_random::Prng;

    #[test]
    fn writes_exactly_the_requested_bytes_matching_fill_bytes() {
        let mut out = Vec::new();
        write_stream(&mut out, 7, Some(16)).unwrap();
        assert_eq!(out.len(), 16);

        let mut expected = [0_u8; 16];
        Prng::from_seed(7).fill_bytes(&mut expected);
        assert_eq!(out, expected);
    }

    #[test]
    fn chunking_does_not_change_the_stream() {
        // Larger than one 64 KiB chunk, and not a multiple of 8.
        let len = CHUNK as u64 + 13;
        let mut out = Vec::new();
        write_stream(&mut out, 3, Some(len)).unwrap();

        let mut expected = vec![0_u8; usize::try_from(len).unwrap()];
        Prng::from_seed(3).fill_bytes(&mut expected);
        assert_eq!(out, expected);
    }

    #[test]
    fn a_closed_pipe_is_success() {
        struct Closed;
        impl std::io::Write for Closed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(write_stream(&mut Closed, 1, None).is_ok());
    }
}
