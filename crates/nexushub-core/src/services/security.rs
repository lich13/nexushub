use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    config::{SecurityConfig, DEFAULT_TURNSTILE_SITE_KEY},
    db::SecuritySettings,
};

pub const MIN_SESSION_TTL_SECONDS: u64 = 300;
pub const MIN_ADMIN_PASSWORD_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityPatch {
    pub turnstile_enabled: Option<bool>,
    pub turnstile_required: Option<bool>,
    pub turnstile_site_key: Option<String>,
    pub turnstile_secret_key: Option<String>,
    pub session_ttl_seconds: Option<u64>,
    pub turnstile_expected_hostname: Option<String>,
    pub turnstile_expected_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityPatchPlan {
    pub settings: Vec<SecuritySettingWrite>,
    pub turnstile_secret_key: Option<String>,
    pub secret_key_changed: bool,
    pub audit_detail: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecuritySettingWrite {
    pub key: &'static str,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityView {
    pub turnstile_enabled: bool,
    pub turnstile_required: bool,
    pub turnstile_site_key: String,
    pub turnstile_secret_configured: bool,
    pub session_ttl_seconds: u64,
    pub turnstile_expected_hostname: Option<String>,
    pub turnstile_expected_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublicSecurityView {
    pub site_name: String,
    pub turnstile_enabled: bool,
    pub turnstile_required: bool,
    pub turnstile_site_key: String,
    pub turnstile_action: String,
    pub admin_configured: bool,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordChangeRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordChangePlan {
    pub new_password: String,
}

pub fn security_view(
    settings: SecuritySettings,
    config: &SecurityConfig,
    stored_expected_hostname: Option<String>,
    stored_expected_action: Option<String>,
) -> SecurityView {
    SecurityView {
        turnstile_enabled: settings.turnstile_enabled,
        turnstile_required: settings.turnstile_required,
        turnstile_site_key: settings
            .turnstile_site_key
            .unwrap_or_else(|| DEFAULT_TURNSTILE_SITE_KEY.to_string()),
        turnstile_secret_configured: settings.turnstile_secret_configured,
        session_ttl_seconds: settings.session_ttl_seconds,
        turnstile_expected_hostname: stored_expected_hostname
            .or_else(|| config.turnstile_expected_hostname.clone()),
        turnstile_expected_action: stored_expected_action
            .or_else(|| config.turnstile_expected_action.clone()),
    }
}

pub fn public_security_view(
    settings: SecuritySettings,
    config: &SecurityConfig,
    stored_turnstile_action: Option<String>,
    admin_configured: bool,
    base_url: Option<String>,
) -> PublicSecurityView {
    PublicSecurityView {
        site_name: "NexusHub".to_string(),
        turnstile_enabled: settings.turnstile_enabled,
        turnstile_required: settings.turnstile_required,
        turnstile_site_key: settings
            .turnstile_site_key
            .unwrap_or_else(|| DEFAULT_TURNSTILE_SITE_KEY.to_string()),
        turnstile_action: stored_turnstile_action
            .or_else(|| config.turnstile_expected_action.clone())
            .unwrap_or_else(|| "login".to_string()),
        admin_configured,
        base_url,
    }
}

pub fn plan_security_patch(patch: SecurityPatch) -> anyhow::Result<SecurityPatchPlan> {
    let mut settings = Vec::new();
    if let Some(value) = patch.turnstile_enabled {
        settings.push(bool_setting("turnstile_enabled", value));
    }
    if let Some(value) = patch.turnstile_required {
        settings.push(bool_setting("turnstile_required", value));
    }
    if let Some(value) = patch.turnstile_site_key {
        settings.push(SecuritySettingWrite {
            key: "turnstile_site_key",
            value,
        });
    }
    if let Some(ttl) = patch.session_ttl_seconds {
        if ttl < MIN_SESSION_TTL_SECONDS {
            anyhow::bail!("session ttl must be at least 300 seconds");
        }
        settings.push(SecuritySettingWrite {
            key: "session_ttl_seconds",
            value: ttl.to_string(),
        });
    }
    if let Some(value) = patch.turnstile_expected_hostname {
        settings.push(SecuritySettingWrite {
            key: "turnstile_expected_hostname",
            value: value.trim().to_string(),
        });
    }
    if let Some(value) = patch.turnstile_expected_action {
        settings.push(SecuritySettingWrite {
            key: "turnstile_expected_action",
            value: value.trim().to_string(),
        });
    }

    let turnstile_secret_key = patch
        .turnstile_secret_key
        .and_then(|value| non_empty_owned(&value));
    let secret_key_changed = turnstile_secret_key.is_some();
    Ok(SecurityPatchPlan {
        settings,
        turnstile_secret_key,
        secret_key_changed,
        audit_detail: json!({
            "turnstile_secret_key": if secret_key_changed { Some("[configured]") } else { None::<&str> }
        }),
    })
}

pub fn plan_password_change(
    request: PasswordChangeRequest,
    current_password_matches: bool,
) -> anyhow::Result<PasswordChangePlan> {
    if !current_password_matches {
        anyhow::bail!("invalid current password");
    }
    if request.new_password.len() < MIN_ADMIN_PASSWORD_LEN {
        anyhow::bail!("new password must be at least 12 characters");
    }
    Ok(PasswordChangePlan {
        new_password: request.new_password,
    })
}

fn bool_setting(key: &'static str, value: bool) -> SecuritySettingWrite {
    SecuritySettingWrite {
        key,
        value: if value { "true" } else { "false" }.to_string(),
    }
}

fn non_empty_owned(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
