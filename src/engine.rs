use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use librqbit::{
    api::{ApiTorrentListOpts, TorrentIdOrHash},
    generate_azereus_style, AddTorrent, AddTorrentOptions, Api, Session, SessionOptions,
    SessionPersistenceConfig, TorrentStats, TorrentStatsState,
};

use crate::types::{TorrentRow, TorrentStatus};

/// Maps a librqbit `TorrentStats` into a display `TorrentRow`.
/// Extracted so it can be unit-tested with fake stats.
pub(crate) fn stats_to_row(id: usize, name: String, stats: TorrentStats) -> TorrentRow {
    let status = match stats.state {
        TorrentStatsState::Initializing => TorrentStatus::Initializing,
        TorrentStatsState::Live => {
            if stats.finished {
                TorrentStatus::Seeding
            } else {
                TorrentStatus::Downloading
            }
        }
        TorrentStatsState::Paused => TorrentStatus::Paused,
        TorrentStatsState::Error => {
            TorrentStatus::Error(stats.error.unwrap_or_else(|| "unknown error".into()))
        }
    };

    let live = stats.live.as_ref();

    let down_speed_bps = live
        .map(|l| (l.download_speed.mbps * 1024.0 * 1024.0) as u64)
        .unwrap_or(0);

    let peer_stats = live.map(|l| &l.snapshot.peer_stats);
    let peers_live = peer_stats.map(|p| p.live).unwrap_or(0);
    let peers_seen = peer_stats.map(|p| p.seen).unwrap_or(0);

    let progress_pct = if stats.total_bytes == 0 {
        0.0
    } else {
        stats.progress_bytes as f32 / stats.total_bytes as f32 * 100.0
    };

    TorrentRow {
        id,
        name,
        total_bytes: stats.total_bytes,
        progress_pct,
        down_speed_bps,
        peers_live,
        peers_seen,
        status,
    }
}

pub struct TorrentEngine {
    api: Arc<Api>,
}

impl TorrentEngine {
    pub async fn new(output_dir: PathBuf) -> anyhow::Result<Self> {
        Self::new_with_opts(output_dir, false).await
    }

    pub async fn new_with_opts(output_dir: PathBuf, disable_dht: bool) -> anyhow::Result<Self> {
        // Enable JSON persistence only in production (not tests — they use disable_dht=true)
        let persistence = if disable_dht {
            None
        } else {
            Some(SessionPersistenceConfig::Json { folder: None })
        };
        let session = Session::new_with_opts(
            output_dir,
            SessionOptions {
                disable_dht,
                disable_dht_persistence: disable_dht,
                fastresume: true,
                persistence,
                listen_port_range: Some(6881..6891),
                enable_upnp_port_forwarding: true,
                peer_id: Some(generate_azereus_style(*b"qB", (4, 6, 0, 0))),
                ..Default::default()
            },
        )
        .await
        .context("failed to create librqbit session")?;
        let api = Arc::new(Api::new(session, None));
        Ok(Self { api })
    }

    pub fn list_torrents(&self) -> Vec<TorrentRow> {
        let list = self
            .api
            .api_torrent_list_ext(ApiTorrentListOpts { with_stats: true });
        list.torrents
            .into_iter()
            .filter_map(|t| {
                let id = t.id?;
                let stats = t.stats?;
                let name = t.name.unwrap_or_else(|| t.info_hash.clone());
                Some(stats_to_row(id, name, stats))
            })
            .collect()
    }

    pub async fn add_torrent(&self, input: &str) -> anyhow::Result<()> {
        let add = AddTorrent::from_cli_argument(input)
            .with_context(|| format!("failed to parse torrent input: {input}"))?;
        self.api
            .api_add_torrent(add, Some(AddTorrentOptions::default()))
            .await
            .context("failed to add torrent")?;
        Ok(())
    }

    pub async fn pause(&self, id: usize) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_pause(TorrentIdOrHash::Id(id))
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub async fn resume(&self, id: usize) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_start(TorrentIdOrHash::Id(id))
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub async fn remove(&self, id: usize, delete_files: bool) -> anyhow::Result<()> {
        if delete_files {
            self.api
                .api_torrent_action_delete(TorrentIdOrHash::Id(id))
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}"))
        } else {
            self.api
                .api_torrent_action_forget(TorrentIdOrHash::Id(id))
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/harry-potter.torrent")
    }

    async fn make_engine() -> (TorrentEngine, TempDir) {
        let dir = TempDir::new().unwrap();
        let engine = TorrentEngine::new_with_opts(dir.path().to_owned(), true)
            .await
            .unwrap();
        (engine, dir)
    }

    #[tokio::test]
    async fn add_torrent_from_file_appears_in_list() {
        let (engine, _dir) = make_engine().await;
        engine
            .add_torrent(fixture_path().to_str().unwrap())
            .await
            .unwrap();
        let torrents = engine.list_torrents();
        assert_eq!(torrents.len(), 1);
        assert!(!torrents[0].name.is_empty());
    }

    #[tokio::test]
    async fn pause_and_resume_changes_status() {
        let (engine, _dir) = make_engine().await;
        engine
            .add_torrent(fixture_path().to_str().unwrap())
            .await
            .unwrap();
        let id = engine.list_torrents()[0].id;

        // Wait until torrent leaves the Initializing state (up to 2s)
        for _ in 0..20 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let rows = engine.list_torrents();
            if rows[0].status != TorrentStatus::Initializing {
                break;
            }
        }

        engine.pause(id).await.unwrap();
        let rows = engine.list_torrents();
        assert_eq!(rows[0].status, TorrentStatus::Paused);

        engine.resume(id).await.unwrap();
        let rows = engine.list_torrents();
        assert_ne!(rows[0].status, TorrentStatus::Paused);
    }

    #[tokio::test]
    async fn remove_torrent_disappears_from_list() {
        let (engine, _dir) = make_engine().await;
        engine
            .add_torrent(fixture_path().to_str().unwrap())
            .await
            .unwrap();
        let id = engine.list_torrents()[0].id;

        engine.remove(id, false).await.unwrap();
        assert!(engine.list_torrents().is_empty());
    }

    #[tokio::test]
    async fn remove_torrent_with_delete_files() {
        let (engine, _dir) = make_engine().await;
        engine
            .add_torrent(fixture_path().to_str().unwrap())
            .await
            .unwrap();
        let id = engine.list_torrents()[0].id;
        engine.remove(id, true).await.unwrap();
        assert!(engine.list_torrents().is_empty());
    }

    /// Real network test — runs DHT, waits up to 90s for peers.
    /// Run explicitly: cargo test -- --ignored real_dht
    #[tokio::test]
    #[ignore = "requires network; run with: cargo test -- --ignored --nocapture"]
    async fn real_dht_finds_peers_for_fixture_torrent() {
        let dir = TempDir::new().unwrap();
        tracing_subscriber::fmt()
            .with_env_filter("librqbit=debug")
            .with_writer(std::io::stderr)
            .try_init()
            .ok();

        // disable_dht_persistence so we don't collide with the running app's DHT port
        let session = Session::new_with_opts(
            dir.path().to_owned(),
            SessionOptions {
                disable_dht: false,
                disable_dht_persistence: true,
                fastresume: false,
                persistence: None,
                listen_port_range: Some(6881..6891),
                enable_upnp_port_forwarding: true,
                peer_id: Some(generate_azereus_style(*b"qB", (4, 6, 0, 0))),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let engine = TorrentEngine { api: Arc::new(Api::new(session, None)) };
        engine.add_torrent(fixture_path().to_str().unwrap()).await.unwrap();

        let start = std::time::Instant::now();
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            let rows = engine.list_torrents();
            let row = &rows[0];
            eprintln!(
                "[{:>3}s] status={:<12} peers={}/{} progress={:.1}% speed={}/s",
                start.elapsed().as_secs(),
                format!("{:?}", row.status),
                row.peers_live,
                row.peers_seen,
                row.progress_pct,
                bytesize::ByteSize(row.down_speed_bps),
            );
            if row.peers_seen > 0 {
                eprintln!("✓ peers discovered");
                return;
            }
            if start.elapsed().as_secs() >= 90 {
                break;
            }
        }
        let rows = engine.list_torrents();
        panic!(
            "No peers found after 90s — peers_live={} peers_seen={} status={:?}. \
             Check firewall (port 6881) or try a different torrent.",
            rows[0].peers_live, rows[0].peers_seen, rows[0].status
        );
    }

    // --- stats_to_row unit tests (no engine needed) ---

    fn fake_stats(
        state: TorrentStatsState,
        finished: bool,
        total_bytes: u64,
        progress_bytes: u64,
        error: Option<String>,
    ) -> librqbit::TorrentStats {
        librqbit::TorrentStats {
            state,
            finished,
            total_bytes,
            progress_bytes, // librqbit's own field — still needed to compute progress_pct
            uploaded_bytes: 0,
            error,
            file_progress: vec![],
            live: None,
        }
    }

    #[test]
    fn stats_to_row_initializing() {
        let stats = fake_stats(TorrentStatsState::Initializing, false, 0, 0, None);
        let row = super::stats_to_row(1, "t".into(), stats);
        assert_eq!(row.status, TorrentStatus::Initializing);
        assert_eq!(row.progress_pct, 0.0);
    }

    #[test]
    fn stats_to_row_downloading() {
        let stats = fake_stats(TorrentStatsState::Live, false, 1000, 500, None);
        let row = super::stats_to_row(0, "t".into(), stats);
        assert_eq!(row.status, TorrentStatus::Downloading);
        assert!((row.progress_pct - 50.0).abs() < 0.01);
    }

    #[test]
    fn stats_to_row_seeding() {
        let stats = fake_stats(TorrentStatsState::Live, true, 1000, 1000, None);
        let row = super::stats_to_row(0, "t".into(), stats);
        assert_eq!(row.status, TorrentStatus::Seeding);
    }

    #[test]
    fn stats_to_row_paused() {
        let stats = fake_stats(TorrentStatsState::Paused, false, 1000, 200, None);
        let row = super::stats_to_row(0, "t".into(), stats);
        assert_eq!(row.status, TorrentStatus::Paused);
    }

    #[test]
    fn stats_to_row_error_with_message() {
        let stats = fake_stats(TorrentStatsState::Error, false, 0, 0, Some("disk full".into()));
        let row = super::stats_to_row(0, "t".into(), stats);
        assert_eq!(row.status, TorrentStatus::Error("disk full".into()));
    }

    #[test]
    fn stats_to_row_error_without_message() {
        let stats = fake_stats(TorrentStatsState::Error, false, 0, 0, None);
        let row = super::stats_to_row(0, "t".into(), stats);
        assert_eq!(row.status, TorrentStatus::Error("unknown error".into()));
    }

    #[test]
    fn stats_to_row_zero_total_bytes_gives_zero_pct() {
        let stats = fake_stats(TorrentStatsState::Initializing, false, 0, 0, None);
        let row = super::stats_to_row(0, "t".into(), stats);
        assert_eq!(row.progress_pct, 0.0);
    }
}
