//! Minisign verification for the portable bundle.
//!
//! The installers are verified by `tauri-plugin-updater` itself. The portable
//! zip has no such plumbing, so it is verified here — against the **same**
//! public key, so both update paths share one trust root.
//!
//! ## The format gotcha
//!
//! `tauri signer generate` and `tauri signer sign` do not emit what
//! `minisign-verify` consumes. They emit the *whole minisign file*,
//! base64-encoded:
//!
//! ```text
//! tauri.conf.json pubkey  -> base64 of "untrusted comment: ...\nRWQ+iEBZ...\n"
//! latest.json signature   -> base64 of the 4-line minisign signature file
//! ```
//!
//! while `PublicKey::from_base64` wants only the key line and
//! `Signature::decode` wants the signature file text. Both formats above were
//! confirmed by running the real CLI, not inferred — see the fixtures in
//! `src-tauri/tests/fixtures/`, generated with `npx tauri signer`.

use minisign_verify::{PublicKey, Signature};

/// Decodes the base64 blob stored in `tauri.conf.json` -> `plugins.updater.pubkey`.
pub fn decode_pubkey(encoded: &str) -> Result<PublicKey, String> {
    let text = decode_base64_text(encoded, "public key")?;
    let key_line = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("untrusted comment:"))
        .next_back()
        .ok_or_else(|| "public key has no key line".to_string())?;

    PublicKey::from_base64(key_line).map_err(|e| format!("invalid public key: {e}"))
}

/// Decodes the base64 blob stored in `latest.json` -> `platforms.*.signature`
/// (identical to the contents of the `.sig` file the CLI writes).
pub fn decode_signature(encoded: &str) -> Result<Signature, String> {
    let text = decode_base64_text(encoded, "signature")?;
    Signature::decode(&text).map_err(|e| format!("invalid signature: {e}"))
}

/// Verifies `bytes` against a base64 signature and a base64 public key.
///
/// Any failure — malformed input or a genuine mismatch — is an `Err`, and the
/// caller must treat it as "do not install".
pub fn verify(bytes: &[u8], signature: &str, pubkey: &str) -> Result<(), String> {
    let key = decode_pubkey(pubkey)?;
    let sig = decode_signature(signature)?;
    key.verify(bytes, &sig, false)
        .map_err(|e| format!("signature does not match the downloaded file: {e}"))
}

fn decode_base64_text(encoded: &str, what: &str) -> Result<String, String> {
    let bytes = base64_decode(encoded.trim()).ok_or_else(|| format!("{what} is not valid base64"))?;
    String::from_utf8(bytes).map_err(|_| format!("{what} is not valid UTF-8 once decoded"))
}

/// Minimal standard-alphabet base64 decoder.
///
/// The project has no base64 crate and this is the only place that needs one;
/// pulling a dependency in for ~30 lines is not worth it. Whitespace is skipped
/// so multi-line blobs decode as-is.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    fn value(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some((byte - b'A') as u32),
            b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
            b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits = 0;
    let mut padding = 0;

    for byte in input.bytes() {
        match byte {
            b'\n' | b'\r' | b' ' | b'\t' => continue,
            b'=' => {
                padding += 1;
                continue;
            }
            _ => {}
        }
        // Data after padding means the blob is malformed.
        if padding > 0 {
            return None;
        }
        acc = (acc << 6) | value(byte)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }

    if padding > 2 || acc != 0 {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Throwaway key pair generated with `npx tauri signer generate` purely for
    // these tests — it signs nothing that ships, so it is safe in the repo.
    const PUBKEY: &str = include_str!("../../tests/fixtures/updater_test.pub");
    const SIGNATURE: &str = include_str!("../../tests/fixtures/sample.bin.sig");
    const SAMPLE: &[u8] = include_bytes!("../../tests/fixtures/sample.bin");

    /// Guards the key that actually ships.
    ///
    /// `tauri signer generate` writes two files whose base64 blobs look alike,
    /// and pasting `localmind.key` instead of `localmind.key.pub` puts the
    /// **private** key in a committed file — it happened once. The plugin only
    /// notices at `app.updater()` time, in front of a user. This notices at
    /// `cargo test` time.
    #[test]
    fn the_configured_public_key_is_a_public_key_and_parses() {
        const CONFIG: &str = include_str!("../../tauri.conf.json");

        let config: serde_json::Value = serde_json::from_str(CONFIG).unwrap();
        let configured = config["plugins"]["updater"]["pubkey"]
            .as_str()
            .expect("plugins.updater.pubkey must exist");

        assert!(
            !configured.is_empty(),
            "plugins.updater.pubkey is empty — nothing will be signed or verified"
        );

        let decoded = decode_base64_text(configured, "public key").unwrap();
        assert!(
            !decoded.contains("secret key"),
            "a PRIVATE key is configured as the public key — replace it with the .pub file"
        );
        assert!(
            decoded.contains("minisign public key"),
            "configured pubkey is not a minisign public key: {}",
            decoded.lines().next().unwrap_or("")
        );

        decode_pubkey(configured).expect("the shipped public key must parse");
    }

    #[test]
    fn base64_roundtrips_known_values() {
        assert_eq!(base64_decode("").unwrap(), b"");
        assert_eq!(base64_decode("Zg==").unwrap(), b"f");
        assert_eq!(base64_decode("Zm8=").unwrap(), b"fo");
        assert_eq!(base64_decode("Zm9v").unwrap(), b"foo");
        assert_eq!(base64_decode("Zm9vYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn base64_rejects_garbage() {
        assert!(base64_decode("not base64!").is_none());
        assert!(base64_decode("Zg==Zg==").is_none());
    }

    #[test]
    fn pubkey_from_the_tauri_cli_format_decodes() {
        // The whole point: the raw blob is NOT what PublicKey::from_base64 takes.
        assert!(PublicKey::from_base64(PUBKEY.trim()).is_err());
        assert!(decode_pubkey(PUBKEY).is_ok());
    }

    #[test]
    fn signature_from_the_tauri_cli_format_decodes() {
        assert!(Signature::decode(SIGNATURE.trim()).is_err());
        assert!(decode_signature(SIGNATURE).is_ok());
    }

    #[test]
    fn a_real_signed_file_verifies() {
        verify(SAMPLE, SIGNATURE, PUBKEY).expect("fixture must verify");
    }

    #[test]
    fn tampered_content_is_rejected() {
        let mut tampered = SAMPLE.to_vec();
        tampered[0] ^= 0xff;
        let err = verify(&tampered, SIGNATURE, PUBKEY).unwrap_err();
        assert!(err.contains("does not match"), "unexpected error: {err}");
    }

    #[test]
    fn malformed_inputs_are_distinguishable() {
        assert!(verify(SAMPLE, SIGNATURE, "not-base64!").unwrap_err().contains("public key"));
        assert!(verify(SAMPLE, "not-base64!", PUBKEY).unwrap_err().contains("signature"));
    }
}
