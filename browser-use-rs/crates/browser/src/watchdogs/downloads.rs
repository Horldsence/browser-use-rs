//! Downloads Watchdog - Monitors and handles file downloads
//!
//! Responsibilities:
//! - Subscribe to Browser.downloadWillBegin and Browser.downloadProgress
//! - Track active downloads
//! - Handle PDF auto-download
//! - Emit download completion events

use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::cdp::CDPClient;
use crate::events::BrowserEvent;
use crate::watchdog::Watchdog;

/// Information about an active download
#[derive(Clone, Debug)]
struct DownloadInfo {
    guid: String,
    url: String,
    suggested_filename: String,
    total_bytes: i64,
    received_bytes: i64,
    state: DownloadState,
}

#[derive(Clone, Debug, PartialEq)]
enum DownloadState {
    InProgress,
    Completed,
    Canceled,
}

/// Downloads Watchdog - monitors browser downloads
pub struct DownloadsWatchdog {
    /// Directory where downloads are saved
    download_dir: PathBuf,

    /// Active downloads tracked by GUID
    active_downloads: Arc<RwLock<HashMap<String, DownloadInfo>>>,

    /// Whether PDF auto-download is enabled
    auto_download_pdfs: bool,
}

impl DownloadsWatchdog {
    /// Create new DownloadsWatchdog with specified download directory
    pub fn new(download_dir: PathBuf) -> Self {
        Self {
            download_dir,
            active_downloads: Arc::new(RwLock::new(HashMap::new())),
            auto_download_pdfs: true,
        }
    }

    /// Create with custom configuration
    pub fn with_config(download_dir: PathBuf, auto_download_pdfs: bool) -> Self {
        Self {
            download_dir,
            active_downloads: Arc::new(RwLock::new(HashMap::new())),
            auto_download_pdfs,
        }
    }

    /// Get count of active downloads (for testing)
    pub async fn active_download_count(&self) -> usize {
        self.active_downloads.read().await.len()
    }

    /// Get download info by GUID (for testing)
    pub async fn get_download(&self, guid: &str) -> Option<DownloadInfo> {
        self.active_downloads.read().await.get(guid).cloned()
    }
}

impl Default for DownloadsWatchdog {
    fn default() -> Self {
        Self::new(PathBuf::from("/tmp/browser-downloads"))
    }
}

#[async_trait]
impl Watchdog for DownloadsWatchdog {
    fn name(&self) -> &str {
        "DownloadsWatchdog"
    }

    async fn on_event(&self, event: &BrowserEvent) {
        match event {
            BrowserEvent::Started => {
                tracing::info!(
                    "[DownloadsWatchdog] Browser started, downloads will be saved to: {:?}",
                    self.download_dir
                );

                // Ensure download directory exists
                if let Err(e) = std::fs::create_dir_all(&self.download_dir) {
                    tracing::error!(
                        "[DownloadsWatchdog] Failed to create download directory: {}",
                        e
                    );
                }
            }

            BrowserEvent::Stopped => {
                tracing::info!("[DownloadsWatchdog] Browser stopped, clearing download state");
                self.active_downloads.write().await.clear();
            }

            _ => {
                // Ignore other events
            }
        }
    }

    async fn on_attach(
        &self,
        cdp_client: Arc<CDPClient>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("[DownloadsWatchdog] Attaching to CDP for download monitoring");

        // Set download behavior - allow downloads and set download path
        let download_path = self.download_dir.to_string_lossy().to_string();

        // Note: Browser.setDownloadBehavior requires browser-level session (sessionId = None)
        match cdp_client
            .send_request(
                "Browser.setDownloadBehavior",
                Some(json!({
                    "behavior": "allowAndName",
                    "downloadPath": download_path,
                    "eventsEnabled": true
                })),
                None, // No session ID = browser-level command
            )
            .await
        {
            Ok(_) => tracing::info!("[DownloadsWatchdog] Set download behavior successfully"),
            Err(e) => tracing::warn!("[DownloadsWatchdog] Failed to set download behavior: {}", e),
        }

        // Subscribe to downloadWillBegin event
        let downloads = self.active_downloads.clone();
        cdp_client.subscribe(
            "Browser.downloadWillBegin",
            Arc::new(move |event| {
                let downloads = downloads.clone();
                tokio::spawn(async move {
                    if let Some(params) = event.params.as_ref() {
                        let guid = params["guid"].as_str().unwrap_or("").to_string();
                        let url = params["url"].as_str().unwrap_or("").to_string();
                        let suggested_filename = params["suggestedFilename"]
                            .as_str()
                            .unwrap_or("download")
                            .to_string();

                        let info = DownloadInfo {
                            guid: guid.clone(),
                            url: url.clone(),
                            suggested_filename: suggested_filename.clone(),
                            total_bytes: 0,
                            received_bytes: 0,
                            state: DownloadState::InProgress,
                        };

                        downloads.write().await.insert(guid.clone(), info);
                        tracing::info!(
                            "[DownloadsWatchdog] Download started: {} -> {}",
                            url,
                            suggested_filename
                        );
                    }
                });
            }),
        );

        // Subscribe to downloadProgress event
        let downloads = self.active_downloads.clone();
        let download_dir = self.download_dir.clone();
        cdp_client.subscribe(
            "Browser.downloadProgress",
            Arc::new(move |event| {
                let downloads = downloads.clone();
                let download_dir = download_dir.clone();
                tokio::spawn(async move {
                    if let Some(params) = event.params.as_ref() {
                        let guid = params["guid"].as_str().unwrap_or("");
                        let state = params["state"].as_str().unwrap_or("inProgress");
                        let total_bytes = params["totalBytes"].as_i64().unwrap_or(0);
                        let received_bytes = params["receivedBytes"].as_i64().unwrap_or(0);

                        let mut downloads_guard = downloads.write().await;
                        if let Some(info) = downloads_guard.get_mut(guid) {
                            info.total_bytes = total_bytes;
                            info.received_bytes = received_bytes;

                            match state {
                                "completed" => {
                                    info.state = DownloadState::Completed;
                                    let final_path = download_dir.join(&info.suggested_filename);
                                    tracing::info!(
										"[DownloadsWatchdog] Download completed: {} ({}/{} bytes) -> {:?}",
										info.url,
										received_bytes,
										total_bytes,
										final_path
									);

                                    // TODO: Emit FileDownloadedEvent to event bus
                                }
                                "canceled" => {
                                    info.state = DownloadState::Canceled;
                                    tracing::warn!(
                                        "[DownloadsWatchdog] Download canceled: {}",
                                        info.url
                                    );
                                }
                                "inProgress" => {
                                    if total_bytes > 0 {
                                        let progress =
                                            (received_bytes as f64 / total_bytes as f64) * 100.0;
                                        tracing::debug!(
											"[DownloadsWatchdog] Download progress: {} - {:.1}% ({}/{} bytes)",
											info.suggested_filename,
											progress,
											received_bytes,
											total_bytes
										);
                                    }
                                }
                                _ => {}
                            }

                            // Remove completed/canceled downloads from tracking after a delay
                            if matches!(
                                info.state,
                                DownloadState::Completed | DownloadState::Canceled
                            ) {
                                let guid = guid.to_string();
                                let downloads_cleanup = downloads.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                    downloads_cleanup.write().await.remove(&guid);
                                });
                            }
                        }
                    }
                });
            }),
        );

        tracing::info!("[DownloadsWatchdog] Successfully attached to download events");
        Ok(())
    }

    async fn on_detach(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("[DownloadsWatchdog] Detaching from CDP");
        self.active_downloads.write().await.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_downloads_watchdog_creation() {
        let download_dir = PathBuf::from("/tmp/test-downloads");
        let watchdog = DownloadsWatchdog::new(download_dir.clone());

        assert_eq!(watchdog.name(), "DownloadsWatchdog");
        assert_eq!(watchdog.download_dir, download_dir);
        assert_eq!(watchdog.active_download_count().await, 0);
    }

    #[tokio::test]
    async fn test_downloads_watchdog_events() {
        let watchdog = DownloadsWatchdog::default();

        // Test Started event
        watchdog.on_event(&BrowserEvent::Started).await;
        assert_eq!(watchdog.active_download_count().await, 0);

        // Test Stopped event
        watchdog.on_event(&BrowserEvent::Stopped).await;
        assert_eq!(watchdog.active_download_count().await, 0);
    }
}
