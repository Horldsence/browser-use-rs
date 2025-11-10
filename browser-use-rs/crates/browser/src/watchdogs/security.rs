//! Security Watchdog - Enforces domain access policies
//!
//! Responsibilities:
//! - Validate URLs against allowed/prohibited domain lists
//! - Block navigation to disallowed domains
//! - Handle redirects to blocked domains
//! - Support glob patterns for domain matching

use async_trait::async_trait;
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::cdp::CDPClient;
use crate::events::BrowserEvent;
use crate::watchdog::Watchdog;

/// Security policy configuration
#[derive(Clone, Debug)]
pub struct SecurityPolicy {
    /// Allowed domains (whitelist). If empty, all domains allowed except prohibited ones.
    pub allowed_domains: Option<HashSet<String>>,

    /// Prohibited domains (blacklist)
    pub prohibited_domains: Option<HashSet<String>>,

    /// Block IP addresses (localhost, 192.168.*, etc.)
    pub block_ip_addresses: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            allowed_domains: None,
            prohibited_domains: None,
            block_ip_addresses: false,
        }
    }
}

/// Security Watchdog - enforces URL access policies
pub struct SecurityWatchdog {
    policy: Arc<RwLock<SecurityPolicy>>,
}

impl SecurityWatchdog {
    /// Create new SecurityWatchdog with default policy (allow all)
    pub fn new() -> Self {
        Self {
            policy: Arc::new(RwLock::new(SecurityPolicy::default())),
        }
    }

    /// Create with custom security policy
    pub fn with_policy(policy: SecurityPolicy) -> Self {
        Self {
            policy: Arc::new(RwLock::new(policy)),
        }
    }

    /// Update security policy at runtime
    pub async fn update_policy(&self, policy: SecurityPolicy) {
        *self.policy.write().await = policy;
    }

    /// Check if a URL is allowed based on current policy
    pub async fn is_url_allowed(&self, url: &str) -> bool {
        let policy = self.policy.read().await;

        // Always allow internal browser URLs
        if matches!(
            url,
            "about:blank"
                | "chrome://new-tab-page/"
                | "chrome://new-tab-page"
                | "chrome://newtab/"
                | "chrome-extension://*"
        ) {
            return true;
        }

        // Parse URL
        let parsed = match url::Url::parse(url) {
            Ok(p) => p,
            Err(_) => return false,
        };

        // Allow data: and blob: URLs
        if matches!(parsed.scheme(), "data" | "blob") {
            return true;
        }

        // Get hostname
        let host = match parsed.host_str() {
            Some(h) => h,
            None => return false,
        };

        // Check if IP address and should be blocked
        if policy.block_ip_addresses && Self::is_ip_address(host) {
            return false;
        }

        // If no policies defined, allow all
        if policy.allowed_domains.is_none() && policy.prohibited_domains.is_none() {
            return true;
        }

        // Check allowed domains (whitelist takes precedence)
        if let Some(ref allowed) = policy.allowed_domains {
            return Self::is_domain_in_set(host, allowed);
        }

        // Check prohibited domains (blacklist)
        if let Some(ref prohibited) = policy.prohibited_domains {
            return !Self::is_domain_in_set(host, prohibited);
        }

        true
    }

    /// Check if hostname is an IP address
    fn is_ip_address(host: &str) -> bool {
        // Simple heuristic: if it parses as IP, it's an IP
        host.parse::<IpAddr>().is_ok()
    }

    /// Check if domain matches any pattern in the set
    fn is_domain_in_set(host: &str, domains: &HashSet<String>) -> bool {
        // Try exact match first (fast path)
        if domains.contains(host) {
            return true;
        }

        // Try with/without www prefix
        let (host_variant, host_alt) = Self::get_domain_variants(host);
        if domains.contains(host_variant) || domains.contains(&host_alt) {
            return true;
        }

        // Check for wildcard patterns
        for pattern in domains {
            if Self::matches_pattern(host, pattern) {
                return true;
            }
        }

        false
    }

    /// Get domain variants (with and without www)
    fn get_domain_variants(host: &str) -> (&str, String) {
        if host.starts_with("www.") {
            (host, host[4..].to_string())
        } else {
            (host, format!("www.{}", host))
        }
    }

    /// Check if hostname matches a pattern (supports wildcards)
    fn matches_pattern(host: &str, pattern: &str) -> bool {
        if !pattern.contains('*') {
            return host == pattern;
        }

        // Handle *.example.com pattern
        if pattern.starts_with("*.") {
            let domain_part = &pattern[2..];
            return host == domain_part || host.ends_with(&format!(".{}", domain_part));
        }

        // Handle other glob patterns (simple implementation)
        // For production, use a proper glob library
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let (prefix, suffix) = (parts[0], parts[1]);
                return host.starts_with(prefix) && host.ends_with(suffix);
            }
        }

        false
    }
}

impl Default for SecurityWatchdog {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Watchdog for SecurityWatchdog {
    fn name(&self) -> &str {
        "SecurityWatchdog"
    }

    async fn on_event(&self, event: &BrowserEvent) {
        match event {
            BrowserEvent::Started => {
                let policy = self.policy.read().await;
                tracing::info!(
                    "[SecurityWatchdog] Active - allowed_domains: {:?}, prohibited_domains: {:?}, block_ips: {}",
                    policy.allowed_domains.as_ref().map(|d| d.len()),
                    policy.prohibited_domains.as_ref().map(|d| d.len()),
                    policy.block_ip_addresses
                );
            }

            BrowserEvent::NavigationComplete { url } => {
                if !self.is_url_allowed(url).await {
                    tracing::warn!(
                        "[SecurityWatchdog] ⛔️ Navigation to blocked URL detected: {}",
                        url
                    );
                    // TODO: Navigate to about:blank or emit error event
                }
            }

            _ => {
                // Ignore other events
            }
        }
    }

    async fn on_attach(
        &self,
        _cdp_client: Arc<CDPClient>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("[SecurityWatchdog] Attached");
        Ok(())
    }

    async fn on_detach(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("[SecurityWatchdog] Detached");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_security_watchdog_default_allows_all() {
        let watchdog = SecurityWatchdog::new();

        assert!(watchdog.is_url_allowed("https://google.com").await);
        assert!(watchdog.is_url_allowed("https://example.com").await);
        assert!(watchdog.is_url_allowed("http://192.168.1.1").await);
    }

    #[tokio::test]
    async fn test_security_watchdog_allowed_domains() {
        let mut allowed = HashSet::new();
        allowed.insert("example.com".to_string());
        allowed.insert("test.org".to_string());

        let policy = SecurityPolicy {
            allowed_domains: Some(allowed),
            prohibited_domains: None,
            block_ip_addresses: false,
        };
        let watchdog = SecurityWatchdog::with_policy(policy);

        assert!(watchdog.is_url_allowed("https://example.com").await);
        assert!(watchdog.is_url_allowed("https://www.example.com").await);
        assert!(watchdog.is_url_allowed("https://test.org/path").await);
        assert!(!watchdog.is_url_allowed("https://google.com").await);
    }

    #[tokio::test]
    async fn test_security_watchdog_prohibited_domains() {
        let mut prohibited = HashSet::new();
        prohibited.insert("malicious.com".to_string());
        prohibited.insert("blocked.org".to_string());

        let policy = SecurityPolicy {
            allowed_domains: None,
            prohibited_domains: Some(prohibited),
            block_ip_addresses: false,
        };
        let watchdog = SecurityWatchdog::with_policy(policy);

        assert!(watchdog.is_url_allowed("https://google.com").await);
        assert!(!watchdog.is_url_allowed("https://malicious.com").await);
        assert!(!watchdog.is_url_allowed("https://www.blocked.org").await);
    }

    #[tokio::test]
    async fn test_security_watchdog_wildcard_patterns() {
        let mut allowed = HashSet::new();
        allowed.insert("*.example.com".to_string());

        let policy = SecurityPolicy {
            allowed_domains: Some(allowed),
            prohibited_domains: None,
            block_ip_addresses: false,
        };
        let watchdog = SecurityWatchdog::with_policy(policy);

        assert!(watchdog.is_url_allowed("https://example.com").await);
        assert!(watchdog.is_url_allowed("https://sub.example.com").await);
        assert!(
            watchdog
                .is_url_allowed("https://deep.sub.example.com")
                .await
        );
        assert!(!watchdog.is_url_allowed("https://other.com").await);
    }

    #[tokio::test]
    async fn test_security_watchdog_block_ips() {
        let policy = SecurityPolicy {
            allowed_domains: None,
            prohibited_domains: None,
            block_ip_addresses: true,
        };
        let watchdog = SecurityWatchdog::with_policy(policy);

        assert!(watchdog.is_url_allowed("https://example.com").await);
        assert!(!watchdog.is_url_allowed("http://192.168.1.1").await);
        assert!(!watchdog.is_url_allowed("http://127.0.0.1:8080").await);
    }

    #[tokio::test]
    async fn test_security_watchdog_internal_urls() {
        let mut allowed = HashSet::new();
        allowed.insert("example.com".to_string());

        let policy = SecurityPolicy {
            allowed_domains: Some(allowed),
            prohibited_domains: None,
            block_ip_addresses: true,
        };
        let watchdog = SecurityWatchdog::with_policy(policy);

        // Internal URLs should always be allowed
        assert!(watchdog.is_url_allowed("about:blank").await);
        assert!(watchdog.is_url_allowed("chrome://new-tab-page/").await);
        assert!(
            watchdog
                .is_url_allowed("data:text/html,<h1>Test</h1>")
                .await
        );
    }
}
