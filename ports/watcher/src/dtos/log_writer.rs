use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub(crate) struct VectorWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl VectorWriter {
    pub(crate) fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn get_buffer(&self) -> &Arc<Mutex<Vec<u8>>> {
        &self.buffer
    }
}

impl std::io::Write for VectorWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let buf_len = buf.len();

        let mut self_buffer = match self.buffer.lock() {
            Ok(buffer) => buffer,
            Err(err) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to lock the buffer: {err}"),
                ));
            }
        };

        self_buffer.extend_from_slice(buf);

        Ok(buf_len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
