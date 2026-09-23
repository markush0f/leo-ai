use crate::LlmError;

#[derive(Default)]
pub(crate) struct Lines {
    buffer: Vec<u8>,
}

impl Lines {
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = self.buffer.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            lines.push(line);
        }
        lines
    }

    pub(crate) fn finish(&mut self) -> Option<Vec<u8>> {
        if self.buffer.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buffer))
        }
    }
}

#[derive(Default)]
pub(crate) struct Sse {
    lines: Lines,
    data: Vec<u8>,
}

impl Sse {
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        let lines = self.lines.push(chunk);
        let mut events = Vec::new();
        for line in lines {
            self.line(line, &mut events);
        }
        events
    }

    pub(crate) fn finish(&mut self) -> Vec<Vec<u8>> {
        let mut events = Vec::new();
        if let Some(line) = self.lines.finish() {
            self.line(line, &mut events);
        }
        if !self.data.is_empty() {
            events.push(std::mem::take(&mut self.data));
        }
        events
    }

    fn line(&mut self, line: Vec<u8>, events: &mut Vec<Vec<u8>>) {
        if line.is_empty() {
            if !self.data.is_empty() {
                events.push(std::mem::take(&mut self.data));
            }
            return;
        }
        let Some(mut data) = line.strip_prefix(b"data:").map(<[u8]>::to_vec) else {
            return;
        };
        if data.first() == Some(&b' ') {
            data.remove(0);
        }
        if !self.data.is_empty() {
            self.data.push(b'\n');
        }
        self.data.extend(data);
    }
}

pub(crate) fn json(data: &[u8]) -> Result<serde_json::Value, LlmError> {
    Ok(serde_json::from_slice(data)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_sse_across_arbitrary_chunks_and_crlf() {
        let mut parser = Sse::default();
        assert!(parser.push(b"data: {\"x\":\"").is_empty());
        assert!(parser.push("á".as_bytes()).is_empty());
        let events = parser.push(b"\"}\r\n\r\ndata: [DONE]\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(json(&events[0]).unwrap()["x"], "á");
        assert_eq!(events[1], b"[DONE]");
    }

    #[test]
    fn frames_ndjson_across_chunks() {
        let mut lines = Lines::default();
        assert!(lines.push(b"{\"a\":").is_empty());
        let framed = lines.push(b"1}\n{\"b\":2}\n");
        assert_eq!(framed.len(), 2);
        assert_eq!(json(&framed[0]).unwrap()["a"], 1);
    }
}
