use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sha2::{Digest, Sha256};

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| anyhow!("hash password: {err}"))?
        .to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn is_sensitive_output_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("auth.json")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("authorization")
        || lower.contains("cookie")
        || lower.contains("device_key")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("private_key")
        || lower.contains("access_key")
        || lower.contains("bearer ")
        || lower.contains("sk-")
        || lower.contains("ghp_")
        || lower.contains("github_pat_")
        || lower.contains("xoxb-")
        || lower.contains("xoxp-")
}

pub fn redact_output(input: &str) -> String {
    let mut out = Vec::new();
    for line in input.lines() {
        if is_sensitive_output_line(line) {
            out.push("[redacted sensitive line]".to_string());
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{hash_password, redact_output, verify_password};

    #[test]
    fn password_hash_round_trips_with_argon2() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
        assert!(!hash.contains("correct horse"));
    }

    #[test]
    fn redacts_token_like_lines() {
        assert_eq!(
            redact_output("ok\nTOKEN=abc"),
            "ok\n[redacted sensitive line]"
        );
    }

    #[test]
    fn redacts_cookie_device_key_and_authorization_lines() {
        assert_eq!(
            redact_output(
                "ok\nCookie: nexushub_session=abc\ndevice_key=secret\nAuthorization Bearer abc\nend"
            ),
            "ok\n[redacted sensitive line]\n[redacted sensitive line]\n[redacted sensitive line]\nend"
        );
    }

    #[test]
    fn redacts_common_api_key_lines() {
        assert_eq!(
            redact_output("ok\nOPENAI_API_KEY=sk-secret\nPRIVATE_KEY=abc\naccess_key=abc\nend"),
            "ok\n[redacted sensitive line]\n[redacted sensitive line]\n[redacted sensitive line]\nend"
        );
    }
}
