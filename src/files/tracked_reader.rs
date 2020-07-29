pub use std::sync::mpsc;
use std::io::Read;

/// A `Read` that sends back its progress through a channel
pub struct TrackedReader<R: Read> {
    inner: R,
    channel: mpsc::Sender<usize>,
}

impl<R: Read> Read for TrackedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let read = self.inner.read(buf)?;
        self.channel.send(read).unwrap();
        Ok(read)
    }
}

impl<R: Read> TrackedReader<R> {
    pub fn wrap(reader: R, channel: mpsc::Sender<usize>) -> Self {
        TrackedReader {
            inner: reader,
            channel,
        }
    }
}