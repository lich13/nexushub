use crate::state::AppState;
use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TurnstileResponse {
    success: bool,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

pub async fn verify_turnstile(
    state: &AppState,
    token: &str,
    remote_ip: Option<&str>,
) -> Result<()> {
    if token.trim().is_empty() {
        anyhow::bail!("turnstile token is empty");
    }
    if state.db.turnstile_token_seen(token)? {
        anyhow::bail!("turnstile token has already been used");
    }
    let secret = state
        .db
        .turnstile_secret()?
        .filter(|v| !v.trim().is_empty())
        .context("turnstile secret is not configured")?;
    let mut params = vec![("secret", secret), ("response", token.to_string())];
    if let Some(ip) = remote_ip {
        params.push(("remoteip", ip.to_string()));
    }
    let response = state
        .http
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&params)
        .send()
        .await
        .context("verify turnstile")?;
    let parsed = response
        .json::<TurnstileResponse>()
        .await
        .context("decode turnstile")?;
    state.db.record_turnstile_attempt(
        token,
        parsed.action.as_deref().unwrap_or(""),
        parsed.hostname.as_deref(),
        remote_ip,
        parsed.success,
        &parsed.error_codes,
    )?;
    if !parsed.success {
        anyhow::bail!("{}", turnstile_failure_message(&parsed.error_codes));
    }
    let expected_hostname = state
        .db
        .get_setting("turnstile_expected_hostname")?
        .or_else(|| state.config().security.turnstile_expected_hostname.clone());
    if let Some(expected) = expected_hostname
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if parsed.hostname.as_deref() != Some(expected) {
            anyhow::bail!("turnstile hostname mismatch");
        }
    }
    let expected_action = state
        .db
        .get_setting("turnstile_expected_action")?
        .or_else(|| state.config().security.turnstile_expected_action.clone());
    if let Some(expected) = expected_action
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if parsed.action.as_deref() != Some(expected) {
            anyhow::bail!("turnstile action mismatch");
        }
    }
    Ok(())
}

pub(crate) fn turnstile_failure_message(error_codes: &[String]) -> String {
    if error_codes
        .iter()
        .any(|code| code == "invalid-input-secret")
    {
        return "turnstile verification failed: Turnstile Secret 配置无效，可能仍保存为加密密文或密钥不匹配".to_string();
    }
    if error_codes
        .iter()
        .any(|code| code == "invalid-input-response" || code == "missing-input-response")
    {
        return "turnstile verification failed: Turnstile token 无效或缺失，请刷新页面后重试"
            .to_string();
    }
    if error_codes
        .iter()
        .any(|code| code == "timeout-or-duplicate")
    {
        return "turnstile verification failed: Turnstile token 已过期或重复使用，请重新验证"
            .to_string();
    }
    if error_codes.is_empty() {
        "turnstile verification failed: Cloudflare 未返回具体错误".to_string()
    } else {
        format!(
            "turnstile verification failed: Cloudflare 返回 {}",
            error_codes.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::turnstile_failure_message;

    #[test]
    fn explains_invalid_secret_without_exposing_secret() {
        let message = turnstile_failure_message(&["invalid-input-secret".to_string()]);

        assert!(message.contains("Secret 配置无效"));
        assert!(!message.contains("secret="));
    }
}
