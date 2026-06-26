pub mod http;
pub mod snapshot;
pub mod viewport;

pub use snapshot::DEFAULT_SNAPSHOT_PORT;

pub const PLUGIN_PERMISSIONS: &[&str] = &["ReadApplicationState", "ReadPaneContents", "WebAccess"];
