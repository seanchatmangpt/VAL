use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Receipt {
    pub algorithm: String,
    pub subject: String,
    pub digest: String,
    pub bytes: usize,
}

pub fn receipt(subject: impl Into<String>, bytes: &[u8]) -> Receipt {
    Receipt {
        algorithm: "BLAKE3".to_string(),
        subject: subject.into(),
        digest: blake3::hash(bytes).to_hex().to_string(),
        bytes: bytes.len(),
    }
}
