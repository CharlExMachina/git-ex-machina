//! Sealing a secret for GitHub: libsodium's sealed box, with the repository's public key, done
//! here, so a secret's value never leaves this computer readable and nobody can read it back.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use crypto_box::PublicKey;
use crypto_box::aead::OsRng;

/// `value` sealed with `key` (the provider's public key, base64), as base64.
pub fn seal(key: &str, value: &str) -> Result<String, String> {
    let bytes = STANDARD.decode(key.trim()).map_err(|e| format!("the repository's key isn't base64: {e}"))?;
    let key = PublicKey::from_slice(&bytes).map_err(|_| "the repository's key isn't 32 bytes".to_owned())?;
    let sealed = key.seal(&mut OsRng, value.as_bytes()).map_err(|_| "sealing the secret failed".to_owned())?;
    Ok(STANDARD.encode(sealed))
}

#[cfg(test)]
mod tests {
    use crypto_box::SecretKey;

    use super::*;

    #[test]
    fn only_the_secret_key_of_the_repository_opens_a_sealed_value() {
        let secret = SecretKey::generate(&mut OsRng);
        let public = STANDARD.encode(secret.public_key().as_bytes());
        let sealed = seal(&public, "codecov-token-123").unwrap();
        let opened = secret.unseal(&STANDARD.decode(&sealed).unwrap()).unwrap();
        assert_eq!(String::from_utf8(opened).unwrap(), "codecov-token-123");
        // Sealing is randomised: the same value seals differently each time.
        assert_ne!(seal(&public, "codecov-token-123").unwrap(), sealed);
        assert!(seal("not base64!", "x").is_err());
    }
}
