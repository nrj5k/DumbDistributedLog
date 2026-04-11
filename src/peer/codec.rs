//! Binary codec for peer request/response serialization.

use super::{error::PeerError, request::PeerRequest, response::PeerResponse};

const VERSION: u8 = 0x01;

/// Namespace for peer message encoding/decoding.
pub struct PeerCodec;

impl PeerCodec {
    /// Generic encode with version prefix.
    fn encode_with_version<T: serde::Serialize>(
        version: u8,
        value: &T,
    ) -> Result<Vec<u8>, PeerError> {
        let mut bytes = bincode::serialize(value).map_err(|e| PeerError::CodecError {
            message: e.to_string(),
        })?;
        let mut framed = Vec::with_capacity(bytes.len() + 1);
        framed.push(version);
        framed.append(&mut bytes);
        Ok(framed)
    }

    /// Generic decode with version validation.
    fn decode_with_version<T: serde::de::DeserializeOwned>(
        version: u8,
        bytes: &[u8],
    ) -> Result<T, PeerError> {
        if bytes.is_empty() {
            return Err(PeerError::CodecError {
                message: "empty input".into(),
            });
        }
        if bytes[0] != version {
            return Err(PeerError::CodecError {
                message: format!("invalid version: expected {}, got {}", version, bytes[0]),
            });
        }
        bincode::deserialize(&bytes[1..]).map_err(|e| PeerError::CodecError {
            message: e.to_string(),
        })
    }

    /// Encode a PeerRequest to bytes with version prefix.
    pub fn encode_request(request: &PeerRequest) -> Result<Vec<u8>, PeerError> {
        Self::encode_with_version(VERSION, request)
    }

    /// Decode a PeerRequest from bytes, validating version.
    pub fn decode_request(bytes: &[u8]) -> Result<PeerRequest, PeerError> {
        Self::decode_with_version(VERSION, bytes)
    }

    /// Encode a PeerResponse to bytes with version prefix.
    pub fn encode_response(response: &PeerResponse) -> Result<Vec<u8>, PeerError> {
        Self::encode_with_version(VERSION, response)
    }

    /// Decode a PeerResponse from bytes, validating version.
    pub fn decode_response(bytes: &[u8]) -> Result<PeerResponse, PeerError> {
        Self::decode_with_version(VERSION, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::types::MetricKey;

    #[test]
    fn test_request_round_trip() {
        let request = PeerRequest::new(42, 99, MetricKey::from("test"), vec![1, 2, 3]);
        let encoded = PeerCodec::encode_request(&request).unwrap();
        let decoded = PeerCodec::decode_request(&encoded).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn test_response_round_trip() {
        use crate::peer::types::RequestId;
        let request_id = RequestId::new(42);
        let response = PeerResponse::success(request_id, 100, vec![4, 5, 6]);
        let encoded = PeerCodec::encode_response(&response).unwrap();
        let decoded = PeerCodec::decode_response(&encoded).unwrap();
        assert_eq!(response, decoded);
    }

    #[test]
    fn test_invalid_version() {
        let request = PeerRequest::new(1, 2, MetricKey::from("x"), vec![]);
        let mut encoded = PeerCodec::encode_request(&request).unwrap();
        encoded[0] = 0x99;
        assert!(PeerCodec::decode_request(&encoded).is_err());
    }

    #[test]
    fn test_empty_input() {
        assert!(PeerCodec::decode_request(&[]).is_err());
        assert!(PeerCodec::decode_response(&[]).is_err());
    }
}
