use anyhow::bail;
use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::sign::Signer;
use openssl::symm::{self, Cipher};

use crate::jose::JoseError;
use crate::jwe::JweContentEncryption;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum AesCbcHmacJweEncryption {
    /// AES_128_CBC_HMAC_SHA_256 authenticated encryption algorithm
    A128CbcHS256,
    /// AES_192_CBC_HMAC_SHA_384 authenticated encryption algorithm
    A192CbcHS384,
    /// AES_256_CBC_HMAC_SHA_512 authenticated encryption algorithm
    A256CbcHS512,
}

impl AesCbcHmacJweEncryption {
    fn cipher(&self) -> Cipher {
        match self {
            Self::A128CbcHS256 => Cipher::aes_128_cbc(),
            Self::A192CbcHS384 => Cipher::aes_192_cbc(),
            Self::A256CbcHS512 => Cipher::aes_256_cbc(),
        }
    }

    fn calcurate_tag(&self, aad: &[u8], iv: &[u8], ciphertext: &[u8], mac_key: &[u8]) -> Result<Vec<u8>, JoseError> {
        let (message_digest, tlen) = match self {
            Self::A128CbcHS256 => (MessageDigest::sha256(), 16),
            Self::A192CbcHS384 => (MessageDigest::sha384(), 24),
            Self::A256CbcHS512 => (MessageDigest::sha512(), 32),
        };

        let pkey = (|| -> anyhow::Result<PKey<Private>> {
            let pkey = PKey::hmac(mac_key)?;
            Ok(pkey)
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))?;

        let signature = (|| -> anyhow::Result<Vec<u8>> {
            let mut signer = Signer::new(message_digest, &pkey)?;
            signer.update(aad)?;
            signer.update(iv)?;
            signer.update(ciphertext)?;
            signer.update(&aad.len().to_be_bytes())?;
            let mut signature = signer.sign_to_vec()?;
            signature.truncate(tlen);
            Ok(signature)
        })()
        .map_err(|err| JoseError::InvalidSignature(err))?;

        Ok(signature)
    }
}

impl JweContentEncryption for AesCbcHmacJweEncryption {
    fn name(&self) -> &str {
        match self {
            Self::A128CbcHS256 => "A128CBC-HS256",
            Self::A192CbcHS384 => "A192CBC-HS384",
            Self::A256CbcHS512 => "A256CBC-HS512",
        }
    }

    fn content_encryption_key_len(&self) -> usize {
        match self {
            Self::A128CbcHS256 => 16 + 16,
            Self::A192CbcHS384 => 24 + 16,
            Self::A256CbcHS512 => 32 + 16,
        }
    }

    fn iv_len(&self) -> usize {
        16
    }

    fn encrypt(&self, key: &[u8], iv: &[u8], message: &[u8], aad: &[u8]) -> Result<(Vec<u8>, Vec<u8>), JoseError> {
        let split_pos = self.content_encryption_key_len() - 16;
        let mac_key = &key[0..split_pos];
        let enc_key = &key[split_pos..];

        let encrypted_message = (|| -> anyhow::Result<Vec<u8>> {
            let cipher = self.cipher();
            let encrypted_message = symm::encrypt(cipher, enc_key, Some(iv), message)?;
            Ok(encrypted_message)
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))?;

        let tag = self.calcurate_tag(aad, iv, message, mac_key)?;

        Ok((encrypted_message, tag))
    }

    fn decrypt(&self,  key: &[u8], iv: &[u8], encrypted_message: &[u8], aad: &[u8], tag: &[u8]) -> Result<Vec<u8>, JoseError> {
        let split_pos = self.content_encryption_key_len() - 16;
        let mac_key = &key[0..split_pos];
        let enc_key = &key[split_pos..];

        let message = (|| -> anyhow::Result<Vec<u8>> {
            let cipher = self.cipher();
            let message = symm::decrypt(cipher, enc_key, Some(iv), encrypted_message)?;
            Ok(message)
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))?;

        (|| -> anyhow::Result<()> {
            let calc_tag = self.calcurate_tag(aad, iv, &message, mac_key)?;
            if calc_tag.as_slice() != tag {
                bail!("The tag doesn't match.");
            }
            Ok(())
        })()
        .map_err(|err| JoseError::InvalidSignature(err))?;

        Ok(message)
    }

    fn box_clone(&self) -> Box<dyn JweContentEncryption> {
        Box::new(self.clone())
    }
}
