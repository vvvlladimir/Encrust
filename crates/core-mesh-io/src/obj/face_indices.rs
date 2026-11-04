use std::io::{self, BufRead, Read};

const KINDS: [&str; 3] = ["vertex", "texture coordinate", "normal"];

/// Passes an OBJ to `tobj` line by line, refusing a relative face index reaching before the first
/// element: `tobj` 4.0.5 wraps it to a huge index and overflows scaling it (`add_vertex_multi_index`).
pub(super) struct FaceIndexGuard<R> {
    inner: R,
    line: Vec<u8>,
    at: usize,
    /// `v`, `vt` and `vn` lines read so far, which is what a relative index counts back from.
    counts: [u64; 3],
    rejected: Option<String>,
}

impl<R: BufRead> FaceIndexGuard<R> {
    pub(super) fn new(inner: R) -> Self {
        Self {
            inner,
            line: Vec::new(),
            at: 0,
            counts: [0; 3],
            rejected: None,
        }
    }

    /// Why the guard stopped the read, if it did.
    pub(super) fn rejected(&mut self) -> Option<String> {
        self.rejected.take()
    }

    fn check_line(&mut self) -> Result<(), String> {
        let Ok(text) = std::str::from_utf8(&self.line) else {
            return Ok(());
        };
        let mut words = text.split_whitespace();
        match words.next() {
            Some("v") => self.counts[0] += 1,
            Some("vt") => self.counts[1] += 1,
            Some("vn") => self.counts[2] += 1,
            Some("f" | "l") => return words.try_for_each(|vertex| self.check_vertex(vertex)),
            _ => {}
        }
        Ok(())
    }

    fn check_vertex(&self, vertex: &str) -> Result<(), String> {
        for ((kind, count), part) in KINDS.iter().zip(self.counts).zip(vertex.split('/')) {
            let Ok(index) = part.parse::<i64>() else {
                continue;
            };
            if index < 0 && index.unsigned_abs() > count {
                return Err(format!(
                    "face index {index} reaches before the first {kind}, {count} read so far"
                ));
            }
        }
        Ok(())
    }
}

impl<R: BufRead> Read for FaceIndexGuard<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let available = self.fill_buf()?;
        let amount = available.len().min(out.len());
        out[..amount].copy_from_slice(&available[..amount]);
        self.consume(amount);
        Ok(amount)
    }
}

impl<R: BufRead> BufRead for FaceIndexGuard<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.at == self.line.len() {
            self.line.clear();
            self.at = 0;
            self.inner.read_until(b'\n', &mut self.line)?;
            if let Err(reason) = self.check_line() {
                self.line.clear();
                self.rejected = Some(reason.clone());
                return Err(io::Error::new(io::ErrorKind::InvalidData, reason));
            }
        }
        Ok(&self.line[self.at..])
    }

    fn consume(&mut self, amount: usize) {
        self.at = (self.at + amount).min(self.line.len());
    }
}
