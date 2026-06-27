use nexushub_core::services::commands as rpc_commands;

pub(crate) const RPC_THREAD_EVENTS_ROUTE: &str = "/api/rpc/threadEvents/:id";
pub(crate) const RPC_UPLOAD_FILES_ROUTE: &str = "/api/rpc/uploadFiles";
pub(crate) const RPC_COMMAND_ROUTE: &str = "/api/rpc/:command";
pub(crate) const LEGACY_API_FALLBACK_ROUTE: &str = "/api/*path";

pub(crate) fn is_business_rpc_command(command: &str) -> bool {
    rpc_commands::is_allowed_rpc_command(command)
}

pub(crate) fn is_transport_rpc_command(command: &str) -> bool {
    rpc_commands::ALLOWED_TRANSPORT_COMMANDS.contains(&command)
}

pub(crate) fn is_retired_rpc_command(command: &str) -> bool {
    rpc_commands::is_retired_command(command)
}

#[cfg(test)]
mod tests {
    use super::{
        is_business_rpc_command, is_retired_rpc_command, is_transport_rpc_command,
        LEGACY_API_FALLBACK_ROUTE, RPC_COMMAND_ROUTE, RPC_THREAD_EVENTS_ROUTE,
        RPC_UPLOAD_FILES_ROUTE,
    };
    use nexushub_core::services::commands as rpc_commands;

    #[test]
    fn routes_keep_transport_surface_explicit() {
        assert_eq!(RPC_THREAD_EVENTS_ROUTE, "/api/rpc/threadEvents/:id");
        assert_eq!(RPC_UPLOAD_FILES_ROUTE, "/api/rpc/uploadFiles");
        assert_eq!(RPC_COMMAND_ROUTE, "/api/rpc/:command");
        assert_eq!(LEGACY_API_FALLBACK_ROUTE, "/api/*path");
    }

    #[test]
    fn business_transport_and_retired_command_sets_stay_disjoint() {
        for command in rpc_commands::ALLOWED_RPC_COMMANDS {
            assert!(is_business_rpc_command(command));
            assert!(!is_transport_rpc_command(command));
            assert!(!is_retired_rpc_command(command));
        }

        for command in rpc_commands::ALLOWED_TRANSPORT_COMMANDS {
            assert!(is_transport_rpc_command(command));
            assert!(!is_business_rpc_command(command));
        }

        for command in rpc_commands::RETIRED_COMMANDS {
            assert!(is_retired_rpc_command(command));
            assert!(!is_business_rpc_command(command));
        }
    }

    #[test]
    fn required_transport_and_retired_exceptions_stay_out_of_business_allowlist() {
        for command in ["uploadFiles", "threadEvents"] {
            assert!(
                is_transport_rpc_command(command),
                "transport exception must remain explicit: {command}"
            );
            assert!(
                !is_business_rpc_command(command),
                "transport exception must stay out of business allowlist: {command}"
            );
        }

        for command in [
            "startProbeJob",
            "runUpdateAction",
            "getDesktopOverview",
            "getDesktopHome",
        ] {
            assert!(
                is_retired_rpc_command(command),
                "retired compatibility command must stay marked retired: {command}"
            );
            assert!(
                !is_business_rpc_command(command),
                "retired compatibility command must not re-enter business allowlist: {command}"
            );
            assert!(
                !is_transport_rpc_command(command),
                "retired compatibility command must not become a transport exception: {command}"
            );
        }
    }
}
