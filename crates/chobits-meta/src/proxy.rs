//! Helpers for keeping localhost traffic out of HTTP proxies.

const LOOPBACK_NO_PROXY: &[&str] = &["127.0.0.1", "localhost", "::1"];

/// Return a comma-separated `NO_PROXY` / `no_proxy` list that always bypasses loopback.
pub fn no_proxy_with_loopback(existing: &str) -> String {
    let mut entries: Vec<String> = existing
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect();

    for host in LOOPBACK_NO_PROXY {
        if !entries.iter().any(|entry| entry.eq_ignore_ascii_case(host)) {
            entries.push((*host).to_string());
        }
    }

    entries.join(",")
}

/// Update the current process so loopback hosts bypass any HTTP proxy.
pub fn apply_loopback_no_proxy_to_process() {
    for key in ["NO_PROXY", "no_proxy"] {
        let merged = no_proxy_with_loopback(&std::env::var(key).unwrap_or_default());
        std::env::set_var(key, merged);
    }
}

/// Set `NO_PROXY` / `no_proxy` on a child command (e.g. Zellij) before spawn.
pub fn apply_loopback_no_proxy_to_command(cmd: &mut std::process::Command) {
    for key in ["NO_PROXY", "no_proxy"] {
        let merged = no_proxy_with_loopback(&std::env::var(key).unwrap_or_default());
        cmd.env(key, merged);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_becomes_loopback_defaults() {
        assert_eq!(
            no_proxy_with_loopback(""),
            "127.0.0.1,localhost,::1"
        );
    }

    #[test]
    fn appends_missing_loopback_hosts() {
        assert_eq!(
            no_proxy_with_loopback("example.com"),
            "example.com,127.0.0.1,localhost,::1"
        );
    }

    #[test]
    fn does_not_duplicate_existing_entries() {
        assert_eq!(
            no_proxy_with_loopback("LOCALHOST,127.0.0.1,::1,corp.internal"),
            "LOCALHOST,127.0.0.1,::1,corp.internal"
        );
    }
}
