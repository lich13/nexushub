//! Read-only source comparison: native desktop build 20586. This is a fixed
//! allowlist, not negotiation. Unknown versions fail closed; never probe a new
//! mutation or fall back to an independent turn executor.
pub(super) const STREAM_VERSION: u64 = 11;
pub(super) const QUEUE_VERSION: u64 = 2;

#[derive(Clone, Copy)]
pub(super) enum Method {
    Initialize,
    DiscoverOwner,
    Follow,
    Start,
    Steer,
    Interrupt,
    Settings,
    CommandDecision,
    FileDecision,
    Permissions,
    Answers,
    Elicitation,
}

impl Method {
    pub const ALL: [Self; 12] = [
        Self::Initialize,
        Self::DiscoverOwner,
        Self::Follow,
        Self::Start,
        Self::Steer,
        Self::Interrupt,
        Self::Settings,
        Self::CommandDecision,
        Self::FileDecision,
        Self::Permissions,
        Self::Answers,
        Self::Elicitation,
    ];
    pub const fn wire(self) -> (&'static str, u64) {
        match self {
            Self::Initialize => ("initialize", 0),
            Self::DiscoverOwner => ("thread-owner-discovery", 1),
            Self::Follow => ("thread-stream-following-changed", 1),
            Self::Start => ("thread-follower-start-turn", 2),
            Self::Steer => ("thread-follower-steer-turn", 1),
            Self::Interrupt => ("thread-follower-interrupt-turn", 4),
            Self::Settings => ("thread-follower-update-thread-settings", 2),
            Self::CommandDecision => ("thread-follower-command-approval-decision", 1),
            Self::FileDecision => ("thread-follower-file-approval-decision", 1),
            Self::Permissions => ("thread-follower-permissions-request-approval-response", 1),
            Self::Answers => ("thread-follower-submit-user-input", 1),
            Self::Elicitation => ("thread-follower-submit-mcp-server-elicitation-response", 1),
        }
    }
}
