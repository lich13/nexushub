use sha2::{Digest, Sha256};

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
    use super::redact_output;

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
