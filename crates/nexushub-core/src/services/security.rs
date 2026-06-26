use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    config::{SecurityConfig, DEFAULT_TURNSTILE_SITE_KEY},
    db::SecuritySettings,
    platform::PlatformPaths,
    services::system::{require_capability_for_surface, Capability, HostSurface},
};

pub const MIN_SESSION_TTL_SECONDS: u64 = 300;
pub const MIN_ADMIN_PASSWORD_LEN: usize = 12;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
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
pub struct SecurityPatchFacadePlan {
    pub required_capability: Capability,
    pub patch: SecurityPatchPlan,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublicSecurityViewFacadePlan {
    pub required_capability: Capability,
    pub public: PublicSecurityView,
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordChangeFacadePlan {
    pub required_capability: Capability,
    pub change: PasswordChangePlan,
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

pub fn security_view_with_capability(
    platform: &PlatformPaths,
    settings: SecuritySettings,
    config: &SecurityConfig,
    stored_expected_hostname: Option<String>,
    stored_expected_action: Option<String>,
) -> anyhow::Result<SecurityView> {
    security_view_with_surface(
        platform,
        HostSurface::default_for_platform(platform),
        settings,
        config,
        stored_expected_hostname,
        stored_expected_action,
    )
}

pub fn security_view_with_surface(
    platform: &PlatformPaths,
    host_surface: HostSurface,
    settings: SecuritySettings,
    config: &SecurityConfig,
    stored_expected_hostname: Option<String>,
    stored_expected_action: Option<String>,
) -> anyhow::Result<SecurityView> {
    require_capability_for_surface(platform, host_surface, Capability::SecuritySettings)?;
    Ok(security_view(
        settings,
        config,
        stored_expected_hostname,
        stored_expected_action,
    ))
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

pub fn public_security_view_with_capability(
    platform: &PlatformPaths,
    settings: SecuritySettings,
    config: &SecurityConfig,
    stored_turnstile_action: Option<String>,
    admin_configured: bool,
    base_url: Option<String>,
) -> anyhow::Result<PublicSecurityViewFacadePlan> {
    public_security_view_with_surface(
        platform,
        HostSurface::default_for_platform(platform),
        settings,
        config,
        stored_turnstile_action,
        admin_configured,
        base_url,
    )
}

pub fn public_security_view_with_surface(
    platform: &PlatformPaths,
    host_surface: HostSurface,
    settings: SecuritySettings,
    config: &SecurityConfig,
    stored_turnstile_action: Option<String>,
    admin_configured: bool,
    base_url: Option<String>,
) -> anyhow::Result<PublicSecurityViewFacadePlan> {
    require_capability_for_surface(platform, host_surface, Capability::WebAuth)?;
    Ok(PublicSecurityViewFacadePlan {
        required_capability: Capability::WebAuth,
        public: public_security_view(
            settings,
            config,
            stored_turnstile_action,
            admin_configured,
            base_url,
        ),
    })
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

pub fn plan_security_patch_with_capability(
    platform: &PlatformPaths,
    patch: SecurityPatch,
) -> anyhow::Result<SecurityPatchFacadePlan> {
    plan_security_patch_with_surface(platform, HostSurface::default_for_platform(platform), patch)
}

pub fn plan_security_patch_with_surface(
    platform: &PlatformPaths,
    host_surface: HostSurface,
    patch: SecurityPatch,
) -> anyhow::Result<SecurityPatchFacadePlan> {
    require_capability_for_surface(platform, host_surface, Capability::SecuritySettings)?;
    Ok(SecurityPatchFacadePlan {
        required_capability: Capability::SecuritySettings,
        patch: plan_security_patch(patch)?,
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

pub fn plan_password_change_with_capability(
    platform: &PlatformPaths,
    request: PasswordChangeRequest,
    current_password_matches: bool,
) -> anyhow::Result<PasswordChangeFacadePlan> {
    plan_password_change_with_surface(
        platform,
        HostSurface::default_for_platform(platform),
        request,
        current_password_matches,
    )
}

pub fn plan_password_change_with_surface(
    platform: &PlatformPaths,
    host_surface: HostSurface,
    request: PasswordChangeRequest,
    current_password_matches: bool,
) -> anyhow::Result<PasswordChangeFacadePlan> {
    require_capability_for_surface(platform, host_surface, Capability::AdminPassword)?;
    Ok(PasswordChangeFacadePlan {
        required_capability: Capability::AdminPassword,
        change: plan_password_change(request, current_password_matches)?,
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

#[cfg(test)]
mod tests {
    use super::{
        plan_security_patch_with_capability, public_security_view_with_surface,
        security_view_with_capability, security_view_with_surface, SecurityPatch,
    };
    use crate::{
        config::{Config, DEFAULT_TURNSTILE_SITE_KEY},
        db::SecuritySettings,
        platform::{PlatformKind, PlatformPaths},
        services::system::{Capability, HostSurface},
    };

    #[test]
    fn security_view_plan_requires_linux_security_settings_capability() {
        let config = Config::for_platform_kind(PlatformKind::Macos);
        let platform = PlatformPaths::for_kind(PlatformKind::Macos);
        let settings = SecuritySettings {
            turnstile_enabled: true,
            turnstile_required: true,
            turnstile_site_key: None,
            turnstile_secret_configured: false,
            session_ttl_seconds: 900,
        };

        let err = security_view_with_capability(&platform, settings, &config.security, None, None)
            .expect_err("macOS should not expose server security settings");

        assert!(err
            .to_string()
            .contains("security_settings is unavailable on macos"));
    }

    #[test]
    fn security_patch_plan_requires_linux_and_records_capability() {
        let platform = PlatformPaths::for_kind(PlatformKind::Linux);
        let plan = plan_security_patch_with_capability(
            &platform,
            SecurityPatch {
                turnstile_enabled: Some(true),
                turnstile_required: Some(false),
                turnstile_site_key: Some(DEFAULT_TURNSTILE_SITE_KEY.to_string()),
                turnstile_secret_key: Some(" secret ".to_string()),
                session_ttl_seconds: Some(300),
                turnstile_expected_hostname: Some(" example.com ".to_string()),
                turnstile_expected_action: Some(" login ".to_string()),
            },
        )
        .expect("Linux should allow security settings");

        assert_eq!(plan.required_capability, Capability::SecuritySettings);
        assert_eq!(plan.patch.turnstile_secret_key.as_deref(), Some("secret"));
        assert!(plan.patch.secret_key_changed);

        let macos = PlatformPaths::for_kind(PlatformKind::Macos);
        let err = plan_security_patch_with_capability(&macos, SecurityPatch::default())
            .expect_err("macOS should not allow security settings");
        assert!(err
            .to_string()
            .contains("security_settings is unavailable on macos"));
    }

    #[test]
    fn desktop_lan_webui_allows_public_login_view_but_not_security_admin_view() {
        let config = Config::for_platform_kind(PlatformKind::Macos);
        let platform = PlatformPaths::for_kind(PlatformKind::Macos);
        let settings = SecuritySettings {
            turnstile_enabled: false,
            turnstile_required: false,
            turnstile_site_key: None,
            turnstile_secret_configured: false,
            session_ttl_seconds: 86400,
        };

        let public = public_security_view_with_surface(
            &platform,
            HostSurface::DesktopLanWebui,
            settings.clone(),
            &config.security,
            None,
            true,
            None,
        )
        .expect("desktop LAN WebUI should expose login metadata");
        assert_eq!(public.required_capability, Capability::WebAuth);
        assert!(!public.public.turnstile_enabled);

        let err = security_view_with_surface(
            &platform,
            HostSurface::DesktopLanWebui,
            settings,
            &config.security,
            None,
            None,
        )
        .expect_err("desktop LAN WebUI must not expose security admin settings");
        assert!(err
            .to_string()
            .contains("security_settings is unavailable on macos desktop_lan_webui"));
    }
}
