pub mod http;
pub mod proxy;
pub mod snapshot;
pub mod viewport;

pub use proxy::{
    apply_loopback_no_proxy_to_command, apply_loopback_no_proxy_to_process, no_proxy_with_loopback,
};
pub use snapshot::DEFAULT_SNAPSHOT_PORT;

pub const PLUGIN_PERMISSIONS: &[&str] = &["ReadApplicationState", "ReadPaneContents", "WebAccess"];
