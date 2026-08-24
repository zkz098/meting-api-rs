use aes::Aes128;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cbc::{Decryptor, Encryptor};
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use rand::{distributions::Alphanumeric, Rng};
use num_bigint::BigUint;

type Aes128CbcEnc = Encryptor<Aes128>;
type Aes128CbcDec = Decryptor<Aes128>;

const NONCE: &str = "0CoJUm6Qyw8W8jud";
const IV: &str = "0102030405060708";
const PUB_KEY: &str = "010001";
const MODULUS: &str = "00e0b509f6259df8642dbc35662901477df22677ec152b5ff68ace615bb7b725152b3ab17a876aea8a5aa76d2e417629ec4ee341f56135fccf695280104e0312ecbda92557c93870114af6c9d05c4f7f0c3685b7a46bee255932575cce10b424d813cfe4875d3e82047b97ddef52741d546b8e289dc6935b3ece0462db0a22b8e7";

/// Generate 16-char alphanum secret key (a-zA-Z0-9)
pub fn create_secret_key(size: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(size)
        .map(char::from)
        .collect()
}

fn aes_cbc_encrypt(plaintext: &str, key: &str) -> String {
    let key_bytes = key.as_bytes();
    let iv_bytes = IV.as_bytes();
    let encryptor = Aes128CbcEnc::new_from_slices(key_bytes, iv_bytes).expect("invalid key/iv");
    let buf = plaintext.as_bytes().to_vec();
    // cbc crate pads in encrypt_padded
    let ciphertext = encryptor.encrypt_padded_vec_mut::<Pkcs7>(&buf);
    BASE64.encode(&ciphertext)
}

#[allow(dead_code)]
fn aes_cbc_decrypt(b64: &str, key: &str) -> String {
    let ciphertext = BASE64.decode(b64).unwrap();
    let key_bytes = key.as_bytes();
    let iv_bytes = IV.as_bytes();
    let decryptor = Aes128CbcDec::new_from_slices(key_bytes, iv_bytes).unwrap();
    let buf = ciphertext.clone();
    let plaintext = decryptor.decrypt_padded_vec_mut::<Pkcs7>(&buf).unwrap();
    String::from_utf8(plaintext).unwrap()
}

/// RSA encrypt with no padding: reverse(text) -> hex -> powmod
fn rsa_encrypt(text: &str) -> String {
    let reversed: String = text.chars().rev().collect();
    let bi_text = BigUint::from_bytes_be(reversed.as_bytes());
    // But original does hex encode of utf8 bytes then parse as hex -> same as from_bytes_be for ascii
    // To match PHP/JS behavior precisely, use hex->bigint: we can use from_bytes_be of hex decoded? Actually hex encoding of bytes then parsing as hex == from_bytes_be
    // So keep from_bytes_be for correctness (ascii safe)
    let mut exp_bytes = hex::decode(PUB_KEY).unwrap();
    // PUB_KEY is "010001" -> hex decode gives [0x01,0x00,0x01]; but modulus has leading 00, need handle
    if exp_bytes[0]==0 { exp_bytes.remove(0); }
    let bi_exp = BigUint::from_bytes_be(&exp_bytes);
    let mod_bytes = hex::decode(MODULUS.trim_start_matches("00")).unwrap();
    // MODULUS has leading 00, strip one
    let bi_mod = BigUint::from_bytes_be(&hex::decode(MODULUS).unwrap());
    // Use modulus as full 00... version; BigUint ignores leading zeros
    let _ = mod_bytes;
    let bi_ret = bi_text.modpow(&bi_exp, &bi_mod);
    let mut hex = bi_ret.to_str_radix(16);
    while hex.len() < 256 {
        hex.insert(0, '0');
    }
    hex
}

/// Main weapi entry: double AES + RSA
/// Returns (params, encSecKey)
pub fn weapi_encrypt(json_text: &str) -> (String, String) {
    let secret = create_secret_key(16);
    weapi_encrypt_with_secret(json_text, &secret)
}

pub fn weapi_encrypt_with_secret(json_text: &str, secret: &str) -> (String, String) {
    let first = aes_cbc_encrypt(json_text, NONCE);
    let params = aes_cbc_encrypt(&first, secret);
    let enc_sec_key = rsa_encrypt(secret);
    (params, enc_sec_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_roundtrip() {
        let pt = r#"{"s":"hello","type":"1"}"#;
        let enc = aes_cbc_encrypt(pt, NONCE);
        let dec = aes_cbc_decrypt(&enc, NONCE);
        assert_eq!(pt, dec);
    }

    #[test]
    fn weapi_deterministic_len() {
        let (params, enc) = weapi_encrypt_with_secret(r#"{"s":"hello"}"#, "a1b2c3d4e5f6g7h8");
        assert!(!params.is_empty());
        assert_eq!(enc.len(), 256);
        // second call with same secret should be identical
        let (p2, e2) = weapi_encrypt_with_secret(r#"{"s":"hello"}"#, "a1b2c3d4e5f6g7h8");
        assert_eq!(params, p2);
        assert_eq!(enc, e2);
    }
}
