use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use tokio::sync::oneshot;

use crate::types::{ClientProfile, SpooferSnapshot};

#[derive(Debug, Clone, PartialEq)]
pub struct SpooferConfig {
    pub tracker_url: String,
    pub info_hash_hex: String,
    pub total_bytes: u64,
    pub upload_rate_bps: u64,
    pub download_rate_bps: u64,
    pub initial_uploaded: u64,
    pub initial_downloaded: u64,
    pub client: ClientProfile,
    pub port: u16,
}

pub struct SpooferHandle {
    /// Dropping this sender signals the announce loop to stop.
    _shutdown: oneshot::Sender<()>,
    pub snapshot: Arc<Mutex<SpooferSnapshot>>,
}

impl SpooferHandle {
    pub fn spawn(config: SpooferConfig) -> Self {
        let snapshot = Arc::new(Mutex::new(SpooferSnapshot {
            uploaded: config.initial_uploaded,
            downloaded: config.initial_downloaded,
            running: true,
            ..Default::default()
        }));
        let (tx, rx) = oneshot::channel();
        let snap_clone = Arc::clone(&snapshot);
        tokio::spawn(announce_loop(config, snap_clone, rx));
        Self {
            _shutdown: tx,
            snapshot,
        }
    }
}

async fn announce_loop(
    config: SpooferConfig,
    snapshot: Arc<Mutex<SpooferSnapshot>>,
    shutdown: oneshot::Receiver<()>,
) {
    if let Err(e) = run_announce_loop(config, Arc::clone(&snapshot), shutdown).await
        && let Ok(mut snap) = snapshot.lock()
    {
        snap.last_error = Some(e.to_string());
        snap.running = false;
    }
}

async fn run_announce_loop(
    config: SpooferConfig,
    snapshot: Arc<Mutex<SpooferSnapshot>>,
    mut shutdown: oneshot::Receiver<()>,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent(config.client.user_agent())
        .timeout(Duration::from_secs(30))
        .build()?;

    let peer_id = generate_peer_id(config.client);
    let key = generate_key();

    let mut uploaded = config.initial_uploaded;
    let mut downloaded = config.initial_downloaded;
    let mut completed_sent = false;

    // Initial started announce
    let (interval_secs, seeders, leechers) =
        send_announce(&client, &config, &peer_id, &key, uploaded, downloaded, "started")
            .await
            .unwrap_or((1800, None, None));

    let mut countdown = interval_secs;
    set_snapshot(
        &snapshot,
        uploaded,
        downloaded,
        seeders,
        leechers,
        interval_secs,
        countdown,
        true,
        None,
    );

    let mut ticker = tokio::time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                let _ = send_announce(&client, &config, &peer_id, &key, uploaded, downloaded, "stopped").await;
                break;
            }
            _ = ticker.tick() => {
                // Accumulate upload (with ±10 % jitter)
                if config.upload_rate_bps > 0 {
                    let r: f64 = rand::random();
                    let jitter = (config.upload_rate_bps as f64 * 0.1 * (r - 0.5)) as i64;
                    let delta = (config.upload_rate_bps as i64 + jitter).max(0) as u64;
                    uploaded = uploaded.saturating_add(delta);
                }

                // Accumulate download (stop at total_bytes)
                if config.download_rate_bps > 0 && downloaded < config.total_bytes {
                    let r: f64 = rand::random();
                    let jitter = (config.download_rate_bps as f64 * 0.1 * (r - 0.5)) as i64;
                    let delta = (config.download_rate_bps as i64 + jitter).max(0) as u64;
                    downloaded = downloaded.saturating_add(delta).min(config.total_bytes);
                }

                // completed event (once, when download finishes)
                if !completed_sent && config.total_bytes > 0 && downloaded >= config.total_bytes {
                    let _ = send_announce(&client, &config, &peer_id, &key, uploaded, downloaded, "completed").await;
                    completed_sent = true;
                    countdown = interval_secs;
                }

                countdown = countdown.saturating_sub(1);

                if countdown == 0 {
                    let (new_interval, new_seeders, new_leechers) =
                        send_announce(&client, &config, &peer_id, &key, uploaded, downloaded, "")
                            .await
                            .unwrap_or((interval_secs, seeders, leechers));
                    countdown = new_interval;
                    set_snapshot(
                        &snapshot,
                        uploaded,
                        downloaded,
                        new_seeders,
                        new_leechers,
                        new_interval,
                        countdown,
                        true,
                        None,
                    );
                } else {
                    set_snapshot(
                        &snapshot,
                        uploaded,
                        downloaded,
                        seeders,
                        leechers,
                        interval_secs,
                        countdown,
                        true,
                        None,
                    );
                }
            }
        }
    }

    set_snapshot(
        &snapshot,
        uploaded,
        downloaded,
        None,
        None,
        0,
        0,
        false,
        None,
    );
    Ok(())
}

async fn send_announce(
    client: &reqwest::Client,
    config: &SpooferConfig,
    peer_id: &str,
    key: &str,
    uploaded: u64,
    downloaded: u64,
    event: &str,
) -> Result<(u64, Option<i64>, Option<i64>)> {
    let params = AnnounceParams { peer_id, key, uploaded, downloaded, event };
    let url = build_announce_url(config, &params)?;

    let resp = client.get(&url).send().await?;
    let bytes = resp.bytes().await?;

    if let Some(reason) = parse_bencode_string(&bytes, "failure reason") {
        anyhow::bail!("tracker: {reason}");
    }

    let interval = parse_bencode_int(&bytes, "interval").unwrap_or(1800).max(0) as u64;
    let seeders = parse_bencode_int(&bytes, "complete");
    let leechers = parse_bencode_int(&bytes, "incomplete");

    Ok((interval, seeders, leechers))
}

/// Per-announce parameters (change on every request).
pub struct AnnounceParams<'a> {
    pub peer_id: &'a str,
    pub key: &'a str,
    pub uploaded: u64,
    pub downloaded: u64,
    pub event: &'a str,
}

/// Build a BEP 3-compliant announce URL.
pub fn build_announce_url(config: &SpooferConfig, params: &AnnounceParams<'_>) -> Result<String> {
    let info_hash = percent_encode_hex(&config.info_hash_hex)?;
    let peer_id_enc = percent_encode_bytes(params.peer_id.as_bytes());
    let left = config.total_bytes.saturating_sub(params.downloaded);
    let sep = if config.tracker_url.contains('?') { '&' } else { '?' };

    let mut url = format!(
        "{}{sep}info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact=1&no_peer_id=1&key={}&numwant=200",
        config.tracker_url,
        info_hash,
        peer_id_enc,
        config.port,
        params.uploaded,
        params.downloaded,
        left,
        params.key,
    );

    if !params.event.is_empty() {
        url.push_str(&format!("&event={}", params.event));
    }

    Ok(url)
}

/// Percent-encode 20 raw bytes decoded from a 40-char hex string.
fn percent_encode_hex(hex: &str) -> Result<String> {
    if hex.len() != 40 {
        anyhow::bail!("info_hash_hex must be 40 chars, got {}", hex.len());
    }
    let mut out = String::with_capacity(60);
    for i in 0..20 {
        let byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| anyhow::anyhow!("invalid hex in info_hash"))?;
        out.push('%');
        out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0').to_ascii_uppercase());
        out.push(char::from_digit((byte & 0xf) as u32, 16).unwrap_or('0').to_ascii_uppercase());
    }
    Ok(out)
}

/// Percent-encode bytes, leaving unreserved chars (RFC 3986) as-is.
fn percent_encode_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for &b in bytes {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(char::from_digit((b >> 4) as u32, 16).unwrap_or('0').to_ascii_uppercase());
            out.push(char::from_digit((b & 0xf) as u32, 16).unwrap_or('0').to_ascii_uppercase());
        }
    }
    out
}

fn generate_peer_id(client: ClientProfile) -> String {
    let prefix = client.peer_id_prefix();
    let remaining = 20usize.saturating_sub(prefix.len());
    let suffix: String = (0..remaining)
        .map(|_| {
            let byte: u8 = rand::random::<u8>() % 10;
            (b'0' + byte) as char
        })
        .collect();
    format!("{prefix}{suffix}")
}

fn generate_key() -> String {
    let n: u32 = rand::random();
    format!("{n:08X}")
}

/// Extract a bencode integer value for `key` from raw bencode bytes.
pub fn parse_bencode_int(data: &[u8], key: &str) -> Option<i64> {
    let key_tag = format!("{}:{}", key.len(), key);
    let tag = key_tag.as_bytes();
    let pos = data.windows(tag.len()).position(|w| w == tag)?;
    let after = &data[pos + tag.len()..];
    if after.first() != Some(&b'i') {
        return None;
    }
    let end = after.iter().position(|&b| b == b'e')?;
    std::str::from_utf8(&after[1..end]).ok()?.parse().ok()
}

/// Extract a bencode string value for `key` from raw bencode bytes.
pub fn parse_bencode_string(data: &[u8], key: &str) -> Option<String> {
    let key_tag = format!("{}:{}", key.len(), key);
    let tag = key_tag.as_bytes();
    let pos = data.windows(tag.len()).position(|w| w == tag)?;
    let after = &data[pos + tag.len()..];
    let colon = after.iter().position(|&b| b == b':')?;
    let len: usize = std::str::from_utf8(&after[..colon]).ok()?.parse().ok()?;
    let start = colon + 1;
    if start + len > after.len() {
        return None;
    }
    String::from_utf8(after[start..start + len].to_vec()).ok()
}

#[allow(clippy::too_many_arguments)]
fn set_snapshot(
    snapshot: &Arc<Mutex<SpooferSnapshot>>,
    uploaded: u64,
    downloaded: u64,
    seeders: Option<i64>,
    leechers: Option<i64>,
    interval_secs: u64,
    countdown_secs: u64,
    running: bool,
    last_error: Option<String>,
) {
    if let Ok(mut snap) = snapshot.lock() {
        snap.uploaded = uploaded;
        snap.downloaded = downloaded;
        snap.seeders = seeders;
        snap.leechers = leechers;
        snap.interval_secs = interval_secs;
        snap.countdown_secs = countdown_secs;
        snap.running = running;
        if last_error.is_some() {
            snap.last_error = last_error;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(tracker_url: &str) -> SpooferConfig {
        SpooferConfig {
            tracker_url: tracker_url.into(),
            info_hash_hex: "aabbccddeeff00112233445566778899aabbccdd".into(),
            total_bytes: 500_000_000,
            upload_rate_bps: 0,
            download_rate_bps: 0,
            initial_uploaded: 0,
            initial_downloaded: 0,
            client: ClientProfile::QBittorrent4_6,
            port: 51413,
        }
    }

    fn make_params<'a>(
        peer_id: &'a str,
        uploaded: u64,
        downloaded: u64,
        event: &'a str,
    ) -> AnnounceParams<'a> {
        AnnounceParams { peer_id, key: "DEADBEEF", uploaded, downloaded, event }
    }

    #[test]
    fn build_announce_url_includes_required_fields() {
        let config = make_config("http://tracker.example.com/announce");
        let params = make_params("-qB4600-123456789012", 1000, 0, "started");
        let url = build_announce_url(&config, &params).unwrap();

        assert!(url.contains("info_hash="), "must contain info_hash");
        assert!(url.contains("peer_id="), "must contain peer_id");
        assert!(url.contains("port=51413"), "must contain port");
        assert!(url.contains("uploaded=1000"), "must contain uploaded");
        assert!(url.contains("downloaded=0"), "must contain downloaded");
        assert!(url.contains("left=500000000"), "must contain left");
        assert!(url.contains("compact=1"), "must contain compact");
        assert!(url.contains("no_peer_id=1"), "must contain no_peer_id");
        assert!(url.contains("numwant=200"), "must contain numwant");
        assert!(url.contains("event=started"), "must contain event");
        assert!(url.contains("key=DEADBEEF"), "must contain key");
    }

    #[test]
    fn build_announce_url_percent_encodes_info_hash() {
        let config = make_config("http://tracker.example.com/announce");
        let params = make_params("-qB4600-123456789012", 0, 0, "");
        let url = build_announce_url(&config, &params).unwrap();

        // Each byte is encoded as %XX — 20 bytes = 60 chars after "info_hash="
        let ih_pos = url.find("info_hash=").unwrap();
        let after = &url[ih_pos + 10..];
        let end = after.find('&').unwrap_or(after.len());
        let encoded = &after[..end];
        assert_eq!(encoded.len(), 60, "20 bytes × 3 chars (%XX) = 60");
        assert!(encoded.starts_with('%'), "must start with %");
    }

    #[test]
    fn build_announce_url_appends_to_existing_query() {
        let config = make_config("http://tracker.example.com/announce?passkey=secret");
        let params = make_params("-qB4600-123456789012", 0, 0, "");
        let url = build_announce_url(&config, &params).unwrap();
        assert!(url.starts_with("http://tracker.example.com/announce?passkey=secret&"));
    }

    #[test]
    fn build_announce_url_empty_event_omits_event_param() {
        let config = make_config("http://tracker.example.com/announce");
        let params = make_params("-qB4600-123456789012", 0, 0, "");
        let url = build_announce_url(&config, &params).unwrap();
        assert!(!url.contains("event="), "empty event must be omitted");
    }

    #[test]
    fn parse_bencode_int_extracts_interval() {
        let data = b"d8:intervali1800e8:completei10e10:incompletei5ee";
        assert_eq!(parse_bencode_int(data, "interval"), Some(1800));
        assert_eq!(parse_bencode_int(data, "complete"), Some(10));
        assert_eq!(parse_bencode_int(data, "incomplete"), Some(5));
    }

    #[test]
    fn parse_bencode_int_returns_none_for_missing_key() {
        let data = b"d8:intervali1800ee";
        assert_eq!(parse_bencode_int(data, "complete"), None);
    }

    #[test]
    fn parse_bencode_string_extracts_failure_reason() {
        let data = b"d14:failure reason3:fooe";
        assert_eq!(
            parse_bencode_string(data, "failure reason"),
            Some("foo".into())
        );
    }

    #[test]
    fn parse_bencode_string_returns_none_for_missing_key() {
        let data = b"d8:intervali1800ee";
        assert_eq!(parse_bencode_string(data, "failure reason"), None);
    }

    #[test]
    fn percent_encode_hex_encodes_all_bytes() {
        let hex = "aabbccddeeff00112233445566778899aabbccdd";
        let encoded = percent_encode_hex(hex).unwrap();
        assert_eq!(encoded.len(), 60);
        assert!(encoded.starts_with("%AA%BB"));
    }

    #[test]
    fn percent_encode_hex_rejects_wrong_length() {
        assert!(percent_encode_hex("tooshort").is_err());
    }
}
