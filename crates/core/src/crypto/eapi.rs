use aes::Aes128;
use ecb::{Decryptor, Encryptor};
use aes::cipher::{BlockEncryptMut, BlockDecryptMut, KeyInit, block_padding::Pkcs7};
use md5::{Md5, Digest};

type Aes128EcbEnc = Encryptor<Aes128>;
type Aes128EcbDec = Decryptor<Aes128>;

const EAPI_KEY: &str = "e82ckenh8dichen8";

fn aes_ecb_encrypt(plaintext: &str, key: &str) -> String {
    let key_bytes = key.as_bytes();
    let enc = Aes128EcbEnc::new_from_slice(key_bytes).unwrap();
    let ciphertext = enc.encrypt_padded_vec_mut::<Pkcs7>(plaintext.as_bytes());
    hex::encode(&ciphertext).to_uppercase()
}

#[allow(dead_code)]
fn aes_ecb_decrypt(hex_str: &str, key: &str) -> String {
    let ciphertext = hex::decode(hex_str).unwrap();
    let key_bytes = key.as_bytes();
    let dec = Aes128EcbDec::new_from_slice(key_bytes).unwrap();
    let plaintext = dec.decrypt_padded_vec_mut::<Pkcs7>(&ciphertext).unwrap();
    String::from_utf8(plaintext).unwrap()
}

/// eapi encrypt: url + json payload -> params hex
pub fn eapi_encrypt(url: &str, payload: &serde_json::Value) -> String {
    let text = serde_json::to_string(payload).unwrap();
    let message = format!("nobody{}use{}md5forencrypt", url, text);
    let mut hasher = Md5::new();
    hasher.update(message.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    let data = format!("{}-36cd479b6b5-{}-36cd479b6b5-{}", url, text, digest);
    aes_ecb_encrypt(&data, EAPI_KEY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn eapi_roundtrip() {
        let payload = json!({"keywords":"hello","limit":30});
        let enc = eapi_encrypt("/api/cloudsearch/pc", &payload);
        assert!(enc.len() % 32 == 0);
        // decrypt should recover original data
        let dec = aes_ecb_decrypt(&enc, EAPI_KEY);
        assert!(dec.contains("/api/cloudsearch/pc"));
        assert!(dec.contains("hello"));
    }
}
