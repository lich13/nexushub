//! Cancellation and memory accounting for read-only cleanup previews.
use anyhow::{ensure, Result};
use std::{
    io::{self, Read},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub(crate) const SCAN_MEMORY_LIMIT: usize = 64 * 1024 * 1024;
pub(crate) const SCAN_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub(crate) const READ_CHUNK: usize = 64 * 1024;

#[derive(Default)]
pub(crate) struct MemoryBudget(AtomicUsize);

#[derive(Debug)]
pub(crate) struct MemoryLimit;
impl std::fmt::Display for MemoryLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("隐藏扫描超过内存预算，请减少会话规模后重试")
    }
}
impl std::error::Error for MemoryLimit {}

pub(crate) struct MemoryLease {
    budget: Arc<MemoryBudget>,
    bytes: usize,
}

impl MemoryLease {
    pub(crate) fn resize(&mut self, bytes: usize) -> Result<()> {
        if bytes > self.bytes {
            let extra = bytes - self.bytes;
            let mut used = self.budget.0.load(Ordering::Acquire);
            loop {
                let next = used
                    .checked_add(extra)
                    .filter(|next| *next <= SCAN_MEMORY_LIMIT)
                    .ok_or(MemoryLimit)?;
                match self.budget.0.compare_exchange_weak(
                    used,
                    next,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(current) => used = current,
                }
            }
        } else {
            self.budget
                .0
                .fetch_sub(self.bytes - bytes, Ordering::AcqRel);
        }
        self.bytes = bytes;
        Ok(())
    }

    pub(crate) fn grow(&mut self, bytes: usize) -> Result<()> {
        self.resize(self.bytes.saturating_add(bytes))
    }
}

impl Drop for MemoryLease {
    fn drop(&mut self) {
        self.budget.0.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

#[derive(Clone)]
pub(crate) struct ScanWork {
    cancelled: Arc<AtomicBool>,
    touched: Arc<Mutex<Instant>>,
    expiring: bool,
    budget: Arc<MemoryBudget>,
}

impl Default for ScanWork {
    fn default() -> Self {
        Self::new(Arc::new(MemoryBudget::default()), false)
    }
}

impl ScanWork {
    pub(crate) fn new(budget: Arc<MemoryBudget>, expiring: bool) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            touched: Arc::new(Mutex::new(Instant::now())),
            expiring,
            budget,
        }
    }

    pub(crate) fn check(&self) -> Result<()> {
        ensure!(!self.cancelled.load(Ordering::Acquire), "扫描已停止");
        ensure!(!self.expired(), "扫描已过期，请重新扫描");
        Ok(())
    }

    pub(crate) fn touch(&self) {
        if let Ok(mut touched) = self.touched.lock() {
            *touched = Instant::now();
        }
    }

    pub(crate) fn expired(&self) -> bool {
        self.expiring
            && self
                .touched
                .lock()
                .map_or(true, |time| time.elapsed() >= SCAN_IDLE_TIMEOUT)
    }

    pub(crate) fn idle_remaining(&self) -> Duration {
        self.touched.lock().map_or(Duration::ZERO, |time| {
            SCAN_IDLE_TIMEOUT.saturating_sub(time.elapsed())
        })
    }

    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub(crate) fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn lease(&self) -> MemoryLease {
        MemoryLease {
            budget: self.budget.clone(),
            bytes: 0,
        }
    }

    pub(crate) fn reader<R: Read>(&self, inner: R) -> CheckedReader<R> {
        CheckedReader {
            inner,
            work: self.clone(),
        }
    }
}

pub(crate) struct CheckedReader<R> {
    inner: R,
    work: ScanWork,
}

impl<R: Read> Read for CheckedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.work.check().map_err(io::Error::other)?;
        let length = buffer.len().min(READ_CHUNK);
        self.inner.read(&mut buffer[..length])
    }
}

pub(crate) fn read_record<R: io::BufRead>(
    reader: &mut R,
    work: &ScanWork,
) -> Result<Option<(Vec<u8>, MemoryLease)>> {
    let mut bytes = Vec::new();
    let mut lease = work.lease();
    loop {
        work.check()?;
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok((!bytes.is_empty()).then_some((bytes, lease)));
        }
        let count = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |n| n + 1)
            .min(READ_CHUNK);
        let end = available[count - 1] == b'\n';
        // Covers the record buffer and serde's temporary value tree.
        lease.resize(
            bytes
                .len()
                .saturating_add(count)
                .saturating_mul(8)
                .saturating_add(4096),
        )?;
        bytes.extend_from_slice(&available[..count]);
        reader.consume(count);
        if end {
            return Ok(Some((bytes, lease)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    #[test]
    fn shared_budget_rejects_overflow_and_returns_all_reservations() {
        let budget = Arc::new(MemoryBudget::default());
        let first = ScanWork::new(budget.clone(), false);
        let second = ScanWork::new(budget.clone(), false);
        let mut first_lease = first.lease();
        let mut second_lease = second.lease();
        first_lease.resize(SCAN_MEMORY_LIMIT - 1).unwrap();
        second_lease.resize(1).unwrap();
        assert!(second_lease.grow(1).unwrap_err().is::<MemoryLimit>());
        assert_eq!(budget.0.load(Ordering::Acquire), SCAN_MEMORY_LIMIT);
        first_lease.resize(0).unwrap();
        second_lease.resize(SCAN_MEMORY_LIMIT).unwrap();
        drop(second_lease);
        assert_eq!(budget.0.load(Ordering::Acquire), 0);
    }

    #[test]
    fn inactivity_expires_and_polling_refreshes_the_deadline() {
        let work = ScanWork::new(Arc::new(MemoryBudget::default()), true);
        *work.touched.lock().unwrap() = Instant::now() - SCAN_IDLE_TIMEOUT;
        assert!(work.expired());
        assert_eq!(work.idle_remaining(), Duration::ZERO);
        assert!(work.check().is_err());
        work.touch();
        assert!(!work.expired());
        assert!(work.idle_remaining() > SCAN_IDLE_TIMEOUT - Duration::from_secs(1));
        work.cancel();
        work.touch();
        assert!(work.check().is_err());
    }

    #[test]
    fn cancellation_is_checked_between_bounded_reads() {
        let work = ScanWork::default();
        let mut reader = work.reader(io::repeat(b'x'));
        let mut buffer = vec![0; 2 * 1024 * 1024];
        let bytes = reader.read(&mut buffer).unwrap();
        assert_eq!(bytes, READ_CHUNK);
        assert!(bytes <= 1024 * 1024);
        work.cancel();
        assert!(reader.read(&mut buffer).is_err());
    }

    #[test]
    fn oversized_record_fails_without_retaining_its_buffer_budget() {
        let work = ScanWork::default();
        let mut reader = BufReader::new(work.reader(io::repeat(b'x')));
        assert!(matches!(read_record(&mut reader, &work), Err(error) if error.is::<MemoryLimit>()));
        assert_eq!(work.budget.0.load(Ordering::Acquire), 0);
        let mut normal = Cursor::new(b"first\nsecond\n");
        let first = read_record(&mut normal, &work).unwrap().unwrap();
        assert_eq!(first.0, b"first\n");
        drop(first);
        assert_eq!(work.budget.0.load(Ordering::Acquire), 0);
    }
}
