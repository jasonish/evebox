// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! The live event feed behind the Live and Top pages and the firehose
//! endpoints.
//!
//! Events reach the feed from one of two sources, never both at once, so
//! an event is not delivered twice:
//!
//! - **Ingest**: events that pass through EveBox, read by its own EVE file
//!   and socket inputs or submitted by agents, are published as they are
//!   ingested. This is the only source with SQLite.
//! - **Datastore**: with Elasticsearch or OpenSearch, when nothing is
//!   ingested through EveBox (for example Filebeat or Logstash write the
//!   events), the datastore is polled for new documents while anyone is
//!   subscribed.
//!
//! Ingest wins: while EVE inputs are configured, or events are being or
//! have recently been ingested, the datastore is not polled. Users don't
//! normally mix the two, and an event ingested by EveBox would otherwise
//! be seen twice, once as it is ingested and again when polled.
//!
//! Polling is demand driven and shared. The first subscriber starts a
//! single feed task, later subscribers attach to the same broadcast, and
//! the last one leaving stops the task, cancelling any request in flight
//! and discarding its state. The next subscriber starts over with a new
//! baseline: documents already in the datastore are not replayed, and
//! nothing is collected while nobody is listening.
//!
//! Polling is best effort, see [`Poller`] for how and what can be missed.

use crate::prelude::*;

use futures::FutureExt;
use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::{Notify, broadcast, watch};
use tokio_util::sync::CancellationToken;

/// An event as broadcast to subscribers, shared as every subscriber gets
/// the same one.
pub(crate) type FeedEvent = Arc<serde_json::Value>;

/// Milliseconds since the Unix epoch.
pub(crate) type Millis = i64;

/// The wall clock, in milliseconds since the Unix epoch. Replaceable for
/// tests.
pub(crate) type Clock = Arc<dyn Fn() -> Millis + Send + Sync>;

/// `last_ingest` when nothing has been ingested.
const NEVER: i64 = i64::MIN;

/// Longest error message sent to clients in the status.
const MAX_ERROR_LEN: usize = 300;

/// Times a baseline answered from only part of the data is retried before
/// it is used anyway. A part that keeps failing, such as an index whose
/// mapping the scan can't use, must not stop the feed for good.
const PARTIAL_BASELINE_RETRIES: u32 = 3;

/// Polls a document found by a scan but not returned by the fetch is
/// fetched again before it is given up on. The copies of a shard can
/// briefly disagree on what is searchable.
const FETCH_ATTEMPTS: u32 = 3;

/// A document in the datastore. The same `_id` in two indices is two
/// documents.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DocRef {
    pub index: String,
    pub id: String,
}

/// A document found by a scan, with its event timestamp.
#[derive(Debug, Clone)]
pub(crate) struct ScannedDoc {
    pub doc: DocRef,
    pub ts: Millis,
}

/// A page of a scan.
#[derive(Debug, Default)]
pub(crate) struct ScanPage {
    pub docs: Vec<ScannedDoc>,
    /// Set, to the reason, when the datastore answered from only part of
    /// the data, such as when some shards failed.
    pub partial: Option<String>,
}

/// A datastore that can be polled for the live feed.
///
/// Errors are shown to users in the feed status, so they must not carry
/// secrets such as credentials in a URL.
#[async_trait::async_trait]
pub(crate) trait FeedSource: Send + Sync + 'static {
    /// Documents with an event timestamp in `[gte, lt)`, newest first, at
    /// most `size` of them. Identities and timestamps only: the events are
    /// fetched separately, for the few documents that are new.
    async fn scan_page(&self, gte: Millis, lt: Millis, size: usize) -> Result<ScanPage>;

    /// The events of these documents, normalized to the shape of an EVE
    /// event as ingested. Documents that no longer exist are left out.
    async fn fetch(&self, docs: &[DocRef]) -> Result<Vec<(DocRef, serde_json::Value)>>;
}

/// Where the feed's events come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FeedSourceKind {
    /// Events published as EveBox ingests them.
    Ingest,
    /// Events found by polling the datastore.
    Datastore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FeedState {
    /// Establishing the baseline: nothing is emitted until it's done.
    Starting,
    Ok,
    /// The last poll failed and will be retried.
    Retrying,
}

/// The feed's health, sent to subscribers when they connect and whenever
/// it changes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct FeedStatus {
    pub source: FeedSourceKind,
    pub state: FeedState,
    /// Seconds between polls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<f64>,
    /// Seconds of event time the last poll looked back over for documents
    /// that arrived late. Less than the configured lookback when it held
    /// more documents than a poll scans.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lookback: Option<u64>,
    /// Events are being, or may have been, skipped: more arrive than a
    /// poll can scan, or part of the datastore didn't answer (`partial`).
    pub behind: bool,
    /// Why the last poll was answered from only part of the data, such as
    /// an index whose shards failed: events stored there are skipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial: Option<String>,
    /// Why the last poll failed, while retrying.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl FeedStatus {
    pub(crate) fn ingest() -> Self {
        Self {
            source: FeedSourceKind::Ingest,
            state: FeedState::Ok,
            interval: None,
            lookback: None,
            behind: false,
            partial: None,
            error: None,
        }
    }

    fn starting(config: &FeedConfig) -> Self {
        Self {
            source: FeedSourceKind::Datastore,
            state: FeedState::Starting,
            interval: Some(config.interval.as_secs_f64()),
            lookback: Some(config.lookback.as_secs()),
            behind: false,
            partial: None,
            error: None,
        }
    }
}

/// Cut a reason shown to users down to a reasonable length.
fn truncate_reason(reason: &str) -> String {
    if reason.len() <= MAX_ERROR_LEN {
        return reason.to_string();
    }
    let mut end = MAX_ERROR_LEN;
    while !reason.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &reason[..end])
}

/// Polling cadence and resource limits.
#[derive(Clone)]
pub(crate) struct FeedConfig {
    /// From the start of one poll to the start of the next.
    pub interval: Duration,
    /// Least pause between polls, when a poll takes longer than the
    /// interval.
    pub min_gap: Duration,
    /// How far back in event time each poll looks for documents it hasn't
    /// seen, for documents that arrive late or out of order.
    pub lookback: Duration,
    /// How far ahead of the clock, for sensors whose clock runs ahead.
    pub future: Duration,
    /// Documents per scan request.
    pub page_size: usize,
    /// Most hits one poll reads, repeats included. A window holding more
    /// documents is covered from the newest down to this many.
    pub max_scan: usize,
    /// Most documents read at once for a millisecond that fills a page.
    /// At most the datastore's result window, 10000 by default.
    pub tie_page: usize,
    /// Documents per fetch request.
    pub fetch_chunk: usize,
    /// Most documents remembered as seen. Well above what a poll scans, so
    /// only reached if something is off.
    pub max_seen: usize,
    pub backoff_min: Duration,
    pub backoff_max: Duration,
    /// Longest a poll may take before it is abandoned as failed.
    pub poll_timeout: Duration,
    /// A wall clock that moves this much more or less than the time that
    /// passed between two polls has been stepped, and polling starts over
    /// with a new baseline.
    pub clock_step: Duration,
    /// Ingest counts as active for this long after the last ingested
    /// event.
    pub ingest_idle: Duration,
    /// How often a feed fed by ingest checks whether ingest went idle.
    pub ingest_check: Duration,
    /// Events held for subscribers that fall behind; beyond that they are
    /// told how many they missed.
    pub capacity: usize,
    pub clock: Clock,
}

impl Default for FeedConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(1),
            min_gap: Duration::from_millis(250),
            lookback: Duration::from_secs(300),
            future: Duration::from_secs(300),
            page_size: 5000,
            max_scan: 20000,
            tie_page: 10000,
            fetch_chunk: 1000,
            max_seen: 80000,
            backoff_min: Duration::from_secs(2),
            backoff_max: Duration::from_secs(10),
            poll_timeout: Duration::from_secs(60),
            clock_step: Duration::from_secs(30),
            ingest_idle: Duration::from_secs(120),
            ingest_check: Duration::from_secs(5),
            capacity: 8192,
            clock: Arc::new(|| chrono::Utc::now().timestamp_millis()),
        }
    }
}

/// Counters, for tests and debugging.
#[derive(Debug, Default)]
pub(crate) struct FeedStats {
    /// Feed tasks started, one per period with subscribers.
    pub tasks: AtomicU64,
    /// Baselines established.
    pub baselines: AtomicU64,
    /// Polls completed, baselines included.
    pub polls: AtomicU64,
    /// Events emitted from the datastore.
    pub emitted: AtomicU64,
    /// Documents currently remembered as seen.
    pub seen: AtomicUsize,
}

/// The live feed. Cheap to clone; clones share the feed.
#[derive(Clone)]
pub(crate) struct LiveFeed {
    shared: Arc<Shared>,
}

struct Shared {
    events: broadcast::Sender<FeedEvent>,
    status: watch::Sender<FeedStatus>,
    /// The datastore to poll, if it can be: Elasticsearch and OpenSearch.
    source: Option<Arc<dyn FeedSource>>,
    /// EveBox reads EVE files or sockets itself.
    ingest_configured: AtomicBool,
    /// Submissions being ingested right now, see [`IngestGuard`].
    ingest_in_flight: AtomicUsize,
    /// When an event was last ingested, in milliseconds since `epoch`.
    last_ingest: AtomicI64,
    /// Counts ingest activity, so a poll can tell that ingest happened
    /// while it ran, however long that took.
    ingest_epoch: AtomicU64,
    epoch: Instant,
    /// Wakes the feed task when ingest becomes active.
    ingest_notify: Notify,
    state: Mutex<State>,
    stats: FeedStats,
    config: FeedConfig,
}

#[derive(Default)]
struct State {
    subscribers: usize,
    generation: u64,
    task: Option<Task>,
}

/// The feed task of the current period with subscribers.
struct Task {
    generation: u64,
    cancel: CancellationToken,
    handle: tokio::task::JoinHandle<()>,
}

/// A subscription to the feed. Dropping it unsubscribes, and dropping the
/// last one stops polling. Moving a field out of it is a compile error, as
/// it would end the subscription while the field is still in use.
pub(crate) struct Subscription {
    pub events: broadcast::Receiver<FeedEvent>,
    pub status: watch::Receiver<FeedStatus>,
    shared: Arc<Shared>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let task = {
            let mut state = self.shared.lock_state();
            state.subscribers = state.subscribers.saturating_sub(1);
            if state.subscribers == 0 {
                state.task.take()
            } else {
                None
            }
        };
        if let Some(task) = task {
            // Cancelling stops the task at its next await, dropping any
            // request in flight. Abort as well, in case it is somewhere
            // that doesn't watch the token.
            task.cancel.cancel();
            task.handle.abort();
            // Its state goes with it.
            self.shared.stats.seen.store(0, Ordering::Relaxed);
        }
    }
}

/// Held while events are ingested, from before any of them can be indexed
/// until all of them are, so the datastore isn't polled meanwhile however
/// long indexing takes.
pub(crate) struct IngestGuard {
    shared: Arc<Shared>,
}

impl Drop for IngestGuard {
    fn drop(&mut self) {
        self.shared.ingest_in_flight.fetch_sub(1, Ordering::SeqCst);
        // Ingest stays active for the idle period from here.
        self.shared.note_ingest();
    }
}

impl LiveFeed {
    /// A feed for a datastore, polling `source` when nothing is ingested
    /// through EveBox. Without a source only ingested events are fed.
    pub(crate) fn new(source: Option<Arc<dyn FeedSource>>) -> Self {
        Self::with_config(source, FeedConfig::default())
    }

    pub(crate) fn with_config(source: Option<Arc<dyn FeedSource>>, config: FeedConfig) -> Self {
        let (events, _) = broadcast::channel(config.capacity.max(1));
        let initial = if source.is_some() {
            FeedStatus::starting(&config)
        } else {
            FeedStatus::ingest()
        };
        let (status, _) = watch::channel(initial);
        Self {
            shared: Arc::new(Shared {
                events,
                status,
                source,
                ingest_configured: AtomicBool::new(false),
                ingest_in_flight: AtomicUsize::new(0),
                last_ingest: AtomicI64::new(NEVER),
                ingest_epoch: AtomicU64::new(0),
                epoch: Instant::now(),
                ingest_notify: Notify::new(),
                state: Mutex::new(State::default()),
                stats: FeedStats::default(),
                config,
            }),
        }
    }

    /// Subscribe to the feed, starting polling if needed.
    pub(crate) fn subscribe(&self) -> Subscription {
        let shared = &self.shared;
        let mut state = shared.lock_state();
        // Subscribed before any polling starts, so nothing is missed.
        let events = shared.events.subscribe();
        if let Some(source) = &shared.source {
            // Normally only the first subscriber starts a task, but a task
            // that ended by itself, which it shouldn't, is replaced too.
            let running = state
                .task
                .as_ref()
                .is_some_and(|task| !task.handle.is_finished());
            if !running {
                state.generation += 1;
                let generation = state.generation;
                let initial = if shared.ingest_active() {
                    FeedStatus::ingest()
                } else {
                    FeedStatus::starting(&shared.config)
                };
                shared.status.send_replace(initial);
                let cancel = CancellationToken::new();
                let handle = tokio::spawn(run(
                    shared.clone(),
                    source.clone(),
                    generation,
                    cancel.clone(),
                ));
                state.task = Some(Task {
                    generation,
                    cancel,
                    handle,
                });
                shared.stats.tasks.fetch_add(1, Ordering::Relaxed);
                shared.stats.seen.store(0, Ordering::Relaxed);
            }
        }
        // Only counted once the task is running, so a failure to start it
        // can't leave a subscriber that never was.
        state.subscribers += 1;
        // After any reset above, so the current status is this period's.
        let status = shared.status.subscribe();
        drop(state);
        Subscription {
            events,
            status,
            shared: shared.clone(),
        }
    }

    /// Publish an event as EveBox ingests it, before it is indexed. While
    /// events are being ingested the datastore isn't polled.
    pub(crate) fn publish_ingested(&self, event: &serde_json::Value) {
        self.shared.note_ingest();
        // Avoid the clone when nobody is listening.
        if self.shared.events.receiver_count() > 0 {
            let _ = self.shared.events.send(Arc::new(event.clone()));
        }
    }

    /// Publish events EveBox has ingested, taking ownership. With
    /// [`LiveFeed::ingesting`] held from before they were indexed, they
    /// can be published after.
    pub(crate) fn publish_ingested_all(&self, events: Vec<serde_json::Value>) {
        self.shared.note_ingest();
        for event in events {
            if self.shared.events.receiver_count() == 0 {
                break;
            }
            let _ = self.shared.events.send(Arc::new(event));
        }
    }

    /// Mark events as being ingested until the guard is dropped. Take it
    /// before any of them can be indexed.
    pub(crate) fn ingesting(&self) -> IngestGuard {
        self.shared.ingest_in_flight.fetch_add(1, Ordering::SeqCst);
        self.shared.note_ingest();
        IngestGuard {
            shared: self.shared.clone(),
        }
    }

    /// Whether anyone is subscribed, so ingested events are worth keeping
    /// a copy of to publish.
    pub(crate) fn listening(&self) -> bool {
        self.shared.events.receiver_count() > 0
    }

    /// Whether EveBox reads EVE files or sockets itself, in which case
    /// ingest is always the source.
    pub(crate) fn set_ingest_configured(&self, configured: bool) {
        self.shared
            .ingest_configured
            .store(configured, Ordering::SeqCst);
    }

    /// Current number of subscribers.
    pub(crate) fn subscribers(&self) -> usize {
        self.shared.lock_state().subscribers
    }

    pub(crate) fn stats(&self) -> &FeedStats {
        &self.shared.stats
    }

    /// For the metrics endpoint.
    pub(crate) fn metrics(&self) -> FeedMetrics {
        let subscribers = self.subscribers();
        let stats = self.stats();
        FeedMetrics {
            subscribers,
            status: (subscribers > 0).then(|| self.shared.status.borrow().clone()),
            periods: stats.tasks.load(Ordering::Relaxed),
            polls: stats.polls.load(Ordering::Relaxed),
            emitted: stats.emitted.load(Ordering::Relaxed),
            seen: stats.seen.load(Ordering::Relaxed),
        }
    }
}

/// The live feed in the metrics endpoint.
#[derive(Debug, Serialize)]
pub(crate) struct FeedMetrics {
    pub subscribers: usize,
    /// The current status, while anyone is subscribed.
    pub status: Option<FeedStatus>,
    /// Times polling has been started for a first subscriber.
    pub periods: u64,
    /// Datastore polls completed.
    pub polls: u64,
    /// Events emitted from datastore polls.
    pub emitted: u64,
    /// Documents remembered as seen by the current poller.
    pub seen: usize,
}

impl Shared {
    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        // Nothing panics while holding the lock, but if something did the
        // state is still consistent; don't let it take the feed down.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn mono_ms(&self) -> i64 {
        i64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(i64::MAX)
    }

    fn within_idle(&self, last: i64, now: i64) -> bool {
        last != NEVER && now.saturating_sub(last) < millis(self.config.ingest_idle)
    }

    fn note_ingest(&self) {
        let now = self.mono_ms();
        self.ingest_epoch.fetch_add(1, Ordering::SeqCst);
        let previous = self.last_ingest.swap(now, Ordering::SeqCst);
        if !self.within_idle(previous, now) {
            // Ingest just became active: stop polling now rather than at
            // the next check. Not stored for later: a feed task that isn't
            // waiting checks for ingest anyway before it polls or emits.
            self.ingest_notify.notify_waiters();
        }
    }

    /// Whether events are being ingested through EveBox, so the datastore
    /// shouldn't be polled.
    ///
    /// Connected agents don't count by themselves: an agent can connect
    /// only to serve packet captures or files while something else ships
    /// the events. One that ships events submits them every few seconds,
    /// if only Suricata's stats.
    fn ingest_active(&self) -> bool {
        self.ingest_configured.load(Ordering::SeqCst)
            || self.ingest_in_flight.load(Ordering::SeqCst) > 0
            || self.within_idle(self.last_ingest.load(Ordering::SeqCst), self.mono_ms())
    }

    fn is_current(state: &State, generation: u64) -> bool {
        state
            .task
            .as_ref()
            .is_some_and(|task| task.generation == generation)
    }

    /// Broadcast events found by a poll of the feed task of `generation`
    /// that started at ingest `epoch`.
    ///
    /// They are discarded, returning false, if that task's period has
    /// ended, so a stopped task can't publish to the subscribers of a new
    /// one, or if anything was ingested since the poll started. EveBox
    /// marks ingest before an event can be indexed, so a poll that could
    /// have found an event ingested by EveBox always sees the mark.
    fn emit(&self, generation: u64, epoch: u64, events: Vec<FeedEvent>) -> bool {
        let state = self.lock_state();
        if !Self::is_current(&state, generation)
            || self.ingest_epoch.load(Ordering::SeqCst) != epoch
            || self.ingest_active()
        {
            return false;
        }
        self.stats
            .emitted
            .fetch_add(events.len() as u64, Ordering::Relaxed);
        for event in events {
            let _ = self.events.send(event);
        }
        true
    }

    /// Set the status, if `generation` is the current period.
    fn set_status(&self, generation: u64, status: FeedStatus) {
        let state = self.lock_state();
        if Self::is_current(&state, generation) {
            self.status.send_if_modified(|current| {
                if *current == status {
                    false
                } else {
                    *current = status;
                    true
                }
            });
        }
    }
}

fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

fn backoff(config: &FeedConfig, failures: u32) -> Duration {
    let factor = 2u32.saturating_pow(failures.saturating_sub(1).min(16));
    config
        .backoff_min
        .saturating_mul(factor)
        .min(config.backoff_max)
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic")
}

/// The feed task for one period with subscribers, polling while nothing
/// is ingested through EveBox. Runs until cancelled.
async fn run(
    shared: Arc<Shared>,
    source: Arc<dyn FeedSource>,
    generation: u64,
    cancel: CancellationToken,
) {
    let config = shared.config.clone();
    let mut poller: Option<Poller> = None;
    let mut failures: u32 = 0;
    loop {
        if shared.ingest_active() {
            if poller.take().is_some() {
                info!(
                    "Live feed: events are being ingested by EveBox, stopped polling the datastore"
                );
                shared.stats.seen.store(0, Ordering::Relaxed);
            }
            failures = 0;
            shared.set_status(generation, FeedStatus::ingest());
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(config.ingest_check) => {}
            }
            continue;
        }

        let current = poller.get_or_insert_with(|| {
            debug!("Live feed: polling the datastore");
            shared.set_status(generation, FeedStatus::starting(&config));
            Poller::new(config.clone())
        });
        let started = Instant::now();
        let epoch = shared.ingest_epoch.load(Ordering::SeqCst);
        let poll = AssertUnwindSafe(current.poll(source.as_ref(), &shared, generation, epoch))
            .catch_unwind();
        let outcome = tokio::select! {
            _ = cancel.cancelled() => return,
            outcome = tokio::time::timeout(config.poll_timeout, poll) => outcome,
        };
        let result = match outcome {
            Ok(Ok(result)) => result,
            Ok(Err(panic)) => {
                error!(
                    "Live feed: polling the datastore failed unexpectedly: {}",
                    panic_message(panic.as_ref())
                );
                // Its state can't be trusted after that.
                poller = None;
                Err(anyhow!("internal error"))
            }
            Err(_) => Err(anyhow!(
                "a poll took longer than {} seconds",
                config.poll_timeout.as_secs()
            )),
        };
        shared.stats.seen.store(
            poller.as_ref().map_or(0, |poller| poller.seen.len()),
            Ordering::Relaxed,
        );
        if matches!(result, Ok(Polled::Discarded)) {
            // Something was ingested while the poll ran. What it found
            // can't be told apart from what was ingested, so start over.
            // Kept rather than dropped, so switching to ingest is logged
            // and resets the stats as usual.
            if let Some(poller) = poller.as_mut() {
                poller.reset();
            }
            continue;
        }
        if shared.ingest_active() {
            continue;
        }
        let delay = match result {
            Ok(_) => {
                if failures > 0 {
                    info!("Live feed: polling the datastore recovered");
                }
                failures = 0;
                if let Some(poller) = &poller {
                    shared.set_status(generation, poller.status());
                }
                config
                    .interval
                    .saturating_sub(started.elapsed())
                    .max(config.min_gap)
            }
            Err(err) => {
                failures += 1;
                if failures == 1 {
                    warn!("Live feed: failed to poll the datastore, will retry: {err:#}");
                } else {
                    debug!("Live feed: failed to poll the datastore again: {err:#}");
                }
                let status = match &poller {
                    Some(poller) => poller.failed_status(&err),
                    None => Poller::new(config.clone()).failed_status(&err),
                };
                shared.set_status(generation, status);
                backoff(&config, failures)
            }
        };
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(delay) => {}
            _ = shared.ingest_notify.notified() => {}
        }
    }
}

/// How a poll went, when it didn't fail.
#[derive(Debug, PartialEq, Eq)]
enum Polled {
    Done,
    /// Ingest happened while it ran, so what it found was thrown away.
    Discarded,
}

/// Finds new documents in the datastore.
///
/// Without an arrival time to go by, and as documents can arrive late or
/// out of order, a strict "newer than the last one seen" cursor would miss
/// documents. Instead each poll scans a rolling window of event time, the
/// lookback, newest first, and emits the documents it hasn't seen before.
/// Documents are told apart by index and id; a document seen before is not
/// emitted again, even if it changed.
///
/// The first poll is a baseline: what it finds is already there, and is
/// only remembered. Nothing is emitted until it has completed. Polling
/// also starts over with a new baseline when the clock is stepped. A
/// baseline answered from part of the data marks what the missing part
/// held before it as history, so that isn't emitted once the part answers
/// again; what the part got since is emitted then, late.
///
/// Each poll reads at most `max_scan` hits. When the window holds more
/// documents, the poll stops there and a floor is set just above the
/// millisecond it stopped in, as nothing below was scanned. Later polls
/// don't reach below the floor, so forgetting what is below it can't cause
/// a document to be emitted twice. Documents that arrive below it are
/// missed. If the poll stopped before reaching any document an earlier
/// poll had seen, events are being skipped, which is reported as `behind`.
/// If documents dated in the future used up the scan, the present is
/// scanned again on its own, so a sensor whose clock runs ahead can't hide
/// the others.
///
/// What is missed: documents that arrive with an event time older than the
/// lookback (or the floor), more documents in one millisecond than
/// `tie_page`, documents the fetch doesn't return after a few tries, and
/// anything that arrived while nobody was subscribed.
struct Poller {
    config: FeedConfig,
    /// Documents seen, by their (latest seen) event timestamp. Forgotten
    /// once no poll can find them again.
    seen: HashMap<DocRef, Millis>,
    /// Documents found but not returned by the fetch: attempts so far, and
    /// the timestamp, to be forgotten with the window.
    pending: HashMap<DocRef, (u32, Millis)>,
    /// Polls don't look below this, see above.
    floor: Millis,
    /// The lower bound of the last poll. Never moves back, so a clock
    /// stepping back can't bring forgotten documents into a poll again.
    prev_lower: Millis,
    /// Newest event timestamp, not in the future, found by the polls.
    head: Option<Millis>,
    /// When the last poll started, by the monotonic and wall clocks.
    last_poll: Option<(Instant, Millis)>,
    baseline: bool,
    /// Baselines that failed for being answered from part of the data.
    partial_baselines: u32,
    /// Why the last poll was answered from part of the data, if it was.
    partial: Option<String>,
    /// Set by a baseline answered from part of the data: documents older
    /// than this, first found once all of it answers, are history.
    history_before: Option<Millis>,
    behind: bool,
    /// How far back the last poll covered.
    covered: Duration,
}

/// The documents found by one poll.
struct Scan {
    /// Distinct documents, newest first.
    docs: Vec<ScannedDoc>,
    /// Where the scan stopped, when it reached `max_scan` hits before the
    /// bottom of the window. Older documents weren't scanned.
    truncated_at: Option<Millis>,
    /// The newest millisecond whose documents couldn't all be read, as
    /// more shared it than `tie_page`.
    skipped_at: Option<Millis>,
    /// Why the datastore answered from part of the data, if it did.
    partial: Option<String>,
}

impl Scan {
    /// Take in a scan of the part of the window below this one's, which
    /// then decides where the scan stopped.
    fn append(&mut self, below: Scan) {
        let mut keys: HashSet<DocRef> = self.docs.iter().map(|doc| doc.doc.clone()).collect();
        for doc in below.docs {
            // Only a document re-indexed between the two could repeat.
            if keys.insert(doc.doc.clone()) {
                self.docs.push(doc);
            }
        }
        self.truncated_at = below.truncated_at;
        self.skipped_at = self.skipped_at.or(below.skipped_at);
        if self.partial.is_none() {
            self.partial = below.partial;
        }
    }
}

impl Poller {
    fn new(config: FeedConfig) -> Self {
        let covered = config.lookback;
        Self {
            config,
            seen: HashMap::new(),
            pending: HashMap::new(),
            floor: Millis::MIN,
            prev_lower: Millis::MIN,
            head: None,
            last_poll: None,
            baseline: false,
            partial_baselines: 0,
            partial: None,
            history_before: None,
            behind: false,
            covered,
        }
    }

    /// Forget everything, so the next poll is a new baseline.
    fn reset(&mut self) {
        *self = Self::new(self.config.clone());
    }

    async fn poll(
        &mut self,
        source: &dyn FeedSource,
        shared: &Shared,
        generation: u64,
        epoch: u64,
    ) -> Result<Polled> {
        let now = (self.config.clock)();
        let started = Instant::now();
        if let Some((mono, wall)) = self.last_poll {
            let passed = millis(started.duration_since(mono));
            let moved = now.saturating_sub(wall);
            if moved.saturating_sub(passed).abs() > millis(self.config.clock_step) {
                info!(
                    "Live feed: the clock was stepped by {} seconds, starting over",
                    (moved - passed) / 1000
                );
                self.reset();
            }
        }
        self.last_poll = Some((started, now));

        let lookback = millis(self.config.lookback);
        let upper = now.saturating_add(millis(self.config.future));
        let lower = self
            .prev_lower
            .max(now.saturating_sub(lookback))
            .max(self.floor);

        let mut scan = scan_window(source, &self.config, lower, upper).await?;
        if scan.truncated_at.is_some_and(|stopped| stopped > now) {
            // Documents dated in the future, from a sensor whose clock runs
            // ahead, used up the scan. The present is scanned on its own,
            // so they can't hide everything else. The rest of them are
            // found once they're in the present.
            scan.append(scan_window(source, &self.config, lower, now).await?);
        }

        match &scan.partial {
            Some(reason) if !self.baseline && self.partial_baselines < PARTIAL_BASELINE_RETRIES => {
                // What is missing would be emitted as new once its part
                // answers again.
                self.partial_baselines += 1;
                bail!("incomplete answer from the datastore: {reason}");
            }
            Some(reason) if self.partial.is_none() => {
                warn!("Live feed: the datastore answered a poll from part of the data: {reason}");
            }
            _ => {}
        }
        self.partial = scan.partial.clone();
        let baseline = !self.baseline;
        if baseline && scan.partial.is_some() {
            // What didn't answer has documents from before the baseline,
            // which aren't new once it answers again.
            self.history_before = Some(now);
        }

        // Answered from part of the data, events in the rest are skipped.
        let mut behind = scan.partial.is_some();
        if !baseline {
            // Stopped before reaching anything an earlier poll had seen:
            // more arrived since than a poll reads.
            let overrun = scan.truncated_at.is_some()
                && scan
                    .docs
                    .iter()
                    .rev()
                    .take(self.config.page_size.max(1))
                    .all(|doc| !self.seen.contains_key(&doc.doc));
            let skipped_new = scan
                .skipped_at
                .is_some_and(|skipped| self.head.is_none_or(|head| skipped > head));
            behind |= overrun || skipped_new;
        }

        if baseline {
            shared.stats.baselines.fetch_add(1, Ordering::Relaxed);
            for doc in &scan.docs {
                self.remember(doc);
            }
            self.pending.clear();
        } else {
            // Once all of it answers, what the baseline couldn't see is
            // remembered rather than emitted. What the missing part got
            // since is emitted, late.
            let history_before = match scan.partial {
                None => self.history_before.take(),
                Some(_) => None,
            };
            let mut new: Vec<&ScannedDoc> = Vec::new();
            for doc in &scan.docs {
                if let Some(ts) = self.seen.get_mut(&doc.doc) {
                    // A document re-indexed with a later timestamp must be
                    // remembered for as long as polls can find it.
                    *ts = (*ts).max(doc.ts);
                } else if history_before.is_some_and(|before| doc.ts < before) {
                    self.seen.insert(doc.doc.clone(), doc.ts);
                } else {
                    new.push(doc);
                }
            }
            // Oldest first, so the feed reads in order.
            new.sort_by_key(|doc| doc.ts);
            // Emitted a chunk at a time, so subscribers aren't handed a
            // whole poll at once, and only one chunk is held.
            for chunk in new.chunks(self.config.fetch_chunk.max(1)) {
                let refs: Vec<DocRef> = chunk.iter().map(|doc| doc.doc.clone()).collect();
                let mut found: HashMap<DocRef, serde_json::Value> =
                    source.fetch(&refs).await?.into_iter().collect();
                let mut events = Vec::with_capacity(chunk.len());
                let mut returned = Vec::with_capacity(chunk.len());
                for doc in chunk {
                    let event = found.remove(&doc.doc);
                    returned.push(event.is_some());
                    if let Some(event) = event {
                        events.push(Arc::new(event));
                    }
                }
                if !shared.emit(generation, epoch, events) {
                    return Ok(Polled::Discarded);
                }
                for (doc, returned) in chunk.iter().zip(returned) {
                    if returned {
                        self.pending.remove(&doc.doc);
                        self.remember(doc);
                        continue;
                    }
                    // Not searchable on the copy that answered the fetch
                    // yet, say. Try again next poll, a few times.
                    let attempts =
                        &mut self.pending.entry(doc.doc.clone()).or_insert((0, doc.ts)).0;
                    *attempts += 1;
                    if *attempts >= FETCH_ATTEMPTS {
                        self.pending.remove(&doc.doc);
                        self.remember(doc);
                    }
                }
                tokio::task::yield_now().await;
            }
        }

        if let Some(stopped) = scan.truncated_at {
            // Above the millisecond the scan stopped in, as it may not have
            // read all of it: the rest, if ever found, could be history.
            // Not above the clock, so documents dated in the future can't
            // push the window past current ones.
            self.floor = self.floor.max(stopped.saturating_add(1).min(now));
        }
        let head = scan
            .docs
            .iter()
            .map(|doc| doc.ts)
            .filter(|ts| *ts <= now)
            .max();
        self.head = self.head.max(head);
        self.baseline = true;

        // Forget what no later poll can find again.
        let keep_from = lower.max(self.floor);
        self.seen.retain(|_, ts| *ts >= keep_from);
        self.pending.retain(|_, (_, ts)| *ts >= keep_from);
        if self.seen.len() > self.config.max_seen {
            self.forget_oldest();
            behind = true;
        }
        self.behind = behind;
        self.prev_lower = lower;

        let bottom = scan.truncated_at.unwrap_or(lower).max(self.floor);
        let covered = now.saturating_sub(bottom).clamp(0, lookback);
        self.covered = Duration::from_millis(covered as u64);
        shared.stats.polls.fetch_add(1, Ordering::Relaxed);
        Ok(Polled::Done)
    }

    fn remember(&mut self, doc: &ScannedDoc) {
        if let Some(ts) = self.seen.get_mut(&doc.doc) {
            *ts = (*ts).max(doc.ts);
        } else {
            self.seen.insert(doc.doc.clone(), doc.ts);
        }
    }

    /// Forget the oldest documents to get back to `max_seen`, raising the
    /// floor over them so they can't be found, and emitted, again.
    fn forget_oldest(&mut self) {
        let mut stamps: Vec<Millis> = self.seen.values().copied().collect();
        let excess = stamps.len() - self.config.max_seen;
        let (_, oldest_kept, _) = stamps.select_nth_unstable(excess);
        let oldest_kept = *oldest_kept;
        self.floor = self.floor.max(oldest_kept);
        self.seen.retain(|_, ts| *ts >= oldest_kept);
        self.pending.retain(|_, (_, ts)| *ts >= oldest_kept);
    }

    fn lookback_secs(&self) -> u64 {
        let configured = self.config.lookback.as_secs();
        let covered = self.covered.as_secs();
        if covered >= configured {
            configured
        } else {
            // Rounded down to 10 seconds, so a busy window doesn't send a
            // status change every poll.
            covered / 10 * 10
        }
    }

    fn status(&self) -> FeedStatus {
        FeedStatus {
            source: FeedSourceKind::Datastore,
            state: if self.baseline {
                FeedState::Ok
            } else {
                FeedState::Starting
            },
            interval: Some(self.config.interval.as_secs_f64()),
            lookback: Some(self.lookback_secs()),
            behind: self.behind,
            partial: self.partial.as_deref().map(truncate_reason),
            error: None,
        }
    }

    /// Only the failure: whatever the last poll found out about the
    /// datastore is stale until one succeeds again.
    fn failed_status(&self, err: &anyhow::Error) -> FeedStatus {
        FeedStatus {
            state: FeedState::Retrying,
            behind: false,
            partial: None,
            error: Some(truncate_reason(&format!("{err:#}"))),
            ..self.status()
        }
    }
}

/// Scan `[lower, upper]` newest first, page by page, reading at most
/// `max_scan` hits.
///
/// Each page after the first starts at the last millisecond of the page
/// before, inclusive, so documents sharing that millisecond across the
/// page boundary aren't skipped; the repeats are dropped. Event
/// timestamps aren't unique, and no unique field can be sorted on in
/// every datastore, so this is how the scan pages without skipping. A
/// page all in one millisecond can't be paged past that way, so the rest
/// of that millisecond is read in one larger request.
async fn scan_window(
    source: &dyn FeedSource,
    config: &FeedConfig,
    lower: Millis,
    upper: Millis,
) -> Result<Scan> {
    let page_size = config.page_size.max(1);
    let max_scan = config.max_scan.max(1);
    let max_pages = max_scan.div_ceil(page_size) * 2 + 2;
    let mut keys: HashSet<DocRef> = HashSet::new();
    let mut scan = Scan {
        docs: Vec::new(),
        truncated_at: None,
        skipped_at: None,
        partial: None,
    };
    // Hits read, repeats included: the work budget.
    let mut read = 0;
    let mut pages = 0;
    let mut lt = upper.saturating_add(1);
    while lt > lower {
        let size = page_size.min(max_scan - read);
        let page = source.scan_page(lower, lt, size).await?;
        pages += 1;
        read += page.docs.len();
        let full = page.docs.len() >= size;
        let bounds = page
            .docs
            .first()
            .zip(page.docs.last())
            .map(|(first, last)| (first.ts, last.ts));
        collect(&mut scan, &mut keys, page);
        let Some((first, last)) = bounds else {
            break;
        };
        if !full {
            // The bottom of the window.
            break;
        }
        let next = if first == last {
            let size = config.tie_page.min(max_scan - read);
            if size > page_size {
                let ties = source.scan_page(last, last.saturating_add(1), size).await?;
                pages += 1;
                read += ties.docs.len();
                if ties.docs.len() >= size {
                    scan.skipped_at.get_or_insert(last);
                }
                collect(&mut scan, &mut keys, ties);
            } else {
                scan.skipped_at.get_or_insert(last);
            }
            last
        } else {
            last.saturating_add(1)
        };
        if read >= max_scan || pages >= max_pages {
            // Unless already at the bottom of the window.
            if last > lower {
                scan.truncated_at = Some(last);
            }
            break;
        }
        if next >= lt {
            // No progress, the datastore returned something unexpected.
            scan.skipped_at.get_or_insert(last);
            break;
        }
        lt = next;
    }
    Ok(scan)
}

/// Add a page's documents to a scan, dropping repeats.
fn collect(scan: &mut Scan, keys: &mut HashSet<DocRef>, page: ScanPage) {
    if scan.partial.is_none() {
        scan.partial = page.partial;
    }
    for doc in page.docs {
        if keys.insert(doc.doc.clone()) {
            scan.docs.push(doc);
        }
    }
}

/// A mock datastore and a harness, for tests of the live feed and of what
/// is built on it.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use serde_json::json;

    pub(crate) const NOW: Millis = 1_800_000_000_000;
    pub(crate) const WAIT: Duration = Duration::from_secs(5);

    pub(crate) struct MockDoc {
        pub doc: DocRef,
        pub ts: Millis,
        pub event: serde_json::Value,
        pub visible: bool,
    }

    #[derive(Default)]
    pub(crate) struct MockState {
        pub docs: Vec<MockDoc>,
        pub scan_error: Option<String>,
        /// Fail scans once this many more have succeeded.
        pub fail_scan_after: Option<usize>,
        /// Answer this many more scans from part of the data.
        pub partial_scans: usize,
        /// The index left out of answers from part of the data.
        pub partial_index: Option<String>,
        /// Panic in this many more scans.
        pub panic_scans: usize,
        pub fetch_error: Option<String>,
        /// Leave these documents out of this many more fetches.
        pub fetch_misses: HashMap<DocRef, usize>,
    }

    /// An in-memory datastore.
    #[derive(Default)]
    pub(crate) struct MockSource {
        pub state: Mutex<MockState>,
        pub scans: AtomicUsize,
        /// Hits returned by scans.
        pub hits: AtomicUsize,
        pub fetches: AtomicUsize,
        /// While set, scans wait for a permit.
        pub scan_gate: Mutex<Option<Arc<tokio::sync::Semaphore>>>,
        /// While set, fetches wait for a permit.
        pub fetch_gate: Mutex<Option<Arc<tokio::sync::Semaphore>>>,
        pub in_flight: AtomicUsize,
        pub cancelled: AtomicUsize,
    }

    /// Tracks a request, counting it as cancelled if dropped unfinished.
    struct InFlight<'a> {
        source: &'a MockSource,
        done: bool,
    }

    impl<'a> InFlight<'a> {
        fn new(source: &'a MockSource) -> Self {
            source.in_flight.fetch_add(1, Ordering::SeqCst);
            Self {
                source,
                done: false,
            }
        }
    }

    impl Drop for InFlight<'_> {
        fn drop(&mut self) {
            self.source.in_flight.fetch_sub(1, Ordering::SeqCst);
            if !self.done {
                self.source.cancelled.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    impl MockSource {
        pub(crate) fn add(&self, index: &str, id: &str, ts: Millis) {
            self.add_event(index, id, ts, json!({"event_type": "alert", "id": id}));
        }

        pub(crate) fn add_event(
            &self,
            index: &str,
            id: &str,
            ts: Millis,
            event: serde_json::Value,
        ) {
            self.state.lock().unwrap().docs.push(MockDoc {
                doc: doc(index, id),
                ts,
                event,
                visible: true,
            });
        }

        pub(crate) fn add_hidden(&self, index: &str, id: &str, ts: Millis) {
            self.add(index, id, ts);
            self.state.lock().unwrap().docs.last_mut().unwrap().visible = false;
        }

        pub(crate) fn reveal(&self, index: &str, id: &str) {
            for d in self.state.lock().unwrap().docs.iter_mut() {
                if d.doc == doc(index, id) {
                    d.visible = true;
                }
            }
        }

        pub(crate) fn scans(&self) -> usize {
            self.scans.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl FeedSource for MockSource {
        async fn scan_page(&self, gte: Millis, lt: Millis, size: usize) -> Result<ScanPage> {
            self.scans.fetch_add(1, Ordering::SeqCst);
            let mut flight = InFlight::new(self);
            let gate = self.scan_gate.lock().unwrap().clone();
            if let Some(gate) = gate {
                let _permit = gate.acquire().await.unwrap();
            }
            flight.done = true;
            let mut state = self.state.lock().unwrap();
            if state.panic_scans > 0 {
                state.panic_scans -= 1;
                drop(state);
                panic!("mock scan panicked");
            }
            if let Some(err) = &state.scan_error {
                bail!("{err}");
            }
            if let Some(n) = state.fail_scan_after.as_mut() {
                if *n == 0 {
                    state.fail_scan_after = None;
                    bail!("scan failed part way");
                }
                *n -= 1;
            }
            let partial = (state.partial_scans > 0).then(|| {
                state.partial_scans -= 1;
                "1 of 2 shards failed".to_string()
            });
            let left_out = partial.as_ref().and(state.partial_index.clone());
            let mut docs: Vec<&MockDoc> = state
                .docs
                .iter()
                .filter(|d| d.visible && d.ts >= gte && d.ts < lt)
                .filter(|d| left_out.as_ref() != Some(&d.doc.index))
                .collect();
            // Stable, so ties keep a consistent order.
            docs.sort_by_key(|d| std::cmp::Reverse(d.ts));
            let docs: Vec<ScannedDoc> = docs
                .into_iter()
                .take(size)
                .map(|d| ScannedDoc {
                    doc: d.doc.clone(),
                    ts: d.ts,
                })
                .collect();
            self.hits.fetch_add(docs.len(), Ordering::SeqCst);
            Ok(ScanPage { docs, partial })
        }

        async fn fetch(&self, docs: &[DocRef]) -> Result<Vec<(DocRef, serde_json::Value)>> {
            self.fetches.fetch_add(1, Ordering::SeqCst);
            let mut flight = InFlight::new(self);
            let gate = self.fetch_gate.lock().unwrap().clone();
            if let Some(gate) = gate {
                let _permit = gate.acquire().await.unwrap();
            }
            flight.done = true;
            let mut state = self.state.lock().unwrap();
            if let Some(err) = &state.fetch_error {
                bail!("{err}");
            }
            let mut missed = HashSet::new();
            for doc in docs {
                if let Some(n) = state.fetch_misses.get_mut(doc)
                    && *n > 0
                {
                    *n -= 1;
                    missed.insert(doc.clone());
                }
            }
            Ok(state
                .docs
                .iter()
                .filter(|d| docs.contains(&d.doc) && !missed.contains(&d.doc))
                .map(|d| (d.doc.clone(), d.event.clone()))
                .collect())
        }
    }

    pub(crate) fn doc(index: &str, id: &str) -> DocRef {
        DocRef {
            index: index.to_string(),
            id: id.to_string(),
        }
    }

    /// A feed polling a mock datastore, with a clock that only moves when
    /// told to.
    pub(crate) struct Harness {
        pub source: Arc<MockSource>,
        pub clock: Arc<AtomicI64>,
        pub feed: LiveFeed,
    }

    impl Harness {
        pub(crate) fn new() -> Self {
            Self::with(|_| {})
        }

        pub(crate) fn with(tweak: impl FnOnce(&mut FeedConfig)) -> Self {
            let source = Arc::new(MockSource::default());
            let clock = Arc::new(AtomicI64::new(NOW));
            let mut config = test_config(&clock);
            tweak(&mut config);
            let feed = LiveFeed::with_config(Some(source.clone()), config);
            Self {
                source,
                clock,
                feed,
            }
        }

        pub(crate) fn advance(&self, ms: i64) {
            self.clock.fetch_add(ms, Ordering::SeqCst);
        }

        pub(crate) fn stats(&self) -> &FeedStats {
            self.feed.stats()
        }

        pub(crate) fn polls(&self) -> u64 {
            self.stats().polls.load(Ordering::SeqCst)
        }

        /// Wait for at least `n` more polls to complete.
        pub(crate) async fn wait_polls(&self, n: u64) {
            let target = self.polls() + n;
            wait_for(|| self.polls() >= target).await;
        }
    }

    pub(crate) fn test_config(clock: &Arc<AtomicI64>) -> FeedConfig {
        let clock = clock.clone();
        FeedConfig {
            interval: Duration::from_millis(10),
            min_gap: Duration::from_millis(1),
            lookback: Duration::from_secs(300),
            future: Duration::from_secs(300),
            page_size: 4,
            max_scan: 1000,
            tie_page: 100,
            fetch_chunk: 2,
            max_seen: 4000,
            backoff_min: Duration::from_millis(10),
            backoff_max: Duration::from_millis(40),
            poll_timeout: Duration::from_secs(5),
            clock_step: Duration::from_secs(30),
            ingest_idle: Duration::from_millis(300),
            ingest_check: Duration::from_millis(10),
            capacity: 1024,
            clock: Arc::new(move || clock.load(Ordering::SeqCst)),
        }
    }

    pub(crate) async fn wait_for(condition: impl Fn() -> bool) {
        let deadline = Instant::now() + WAIT;
        while !condition() {
            assert!(Instant::now() < deadline, "timed out waiting");
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }

    pub(crate) async fn wait_status(
        subscription: &mut Subscription,
        want: impl Fn(&FeedStatus) -> bool,
    ) {
        let result = tokio::time::timeout(WAIT, async {
            loop {
                if want(&subscription.status.borrow_and_update()) {
                    return;
                }
                subscription.status.changed().await.unwrap();
            }
        })
        .await;
        assert!(
            result.is_ok(),
            "timed out waiting for status, last: {:?}",
            *subscription.status.borrow()
        );
    }

    pub(crate) async fn wait_ok(subscription: &mut Subscription) {
        wait_status(subscription, |s| {
            s.source == FeedSourceKind::Datastore && s.state == FeedState::Ok
        })
        .await;
    }
}

#[cfg(test)]
mod test {
    use super::testing::*;
    use super::*;
    use serde_json::json;
    use tokio::sync::broadcast::error::{RecvError, TryRecvError};

    async fn next_id(subscription: &mut Subscription) -> String {
        let event = tokio::time::timeout(WAIT, subscription.events.recv())
            .await
            .expect("timed out waiting for an event")
            .unwrap();
        event["id"].as_str().unwrap().to_string()
    }

    async fn next_ids(subscription: &mut Subscription, n: usize) -> Vec<String> {
        let mut ids = Vec::new();
        for _ in 0..n {
            ids.push(next_id(subscription).await);
        }
        ids
    }

    /// Assert nothing more is emitted over a few polls.
    async fn assert_quiet(harness: &Harness, subscription: &mut Subscription) {
        harness.wait_polls(3).await;
        match subscription.events.try_recv() {
            Err(TryRecvError::Empty) => {}
            other => panic!("expected no event, got {other:?}"),
        }
    }

    fn sorted(mut ids: Vec<String>) -> Vec<String> {
        ids.sort();
        ids
    }

    #[tokio::test]
    async fn without_a_source_ingested_events_are_fed() {
        let feed = LiveFeed::new(None);
        let mut subscription = feed.subscribe();
        assert_eq!(*subscription.status.borrow(), FeedStatus::ingest());
        feed.publish_ingested(&json!({"id": "a"}));
        assert_eq!(next_id(&mut subscription).await, "a");
        feed.publish_ingested_all(vec![json!({"id": "b"}), json!({"id": "c"})]);
        assert_eq!(next_ids(&mut subscription, 2).await, ["b", "c"]);
        assert_eq!(feed.stats().tasks.load(Ordering::SeqCst), 0);
        drop(subscription);
        assert_eq!(feed.subscribers(), 0);
        assert!(!feed.listening());
        // Nobody listening: dropped, nothing queued for later.
        feed.publish_ingested(&json!({"id": "d"}));
        let mut subscription = feed.subscribe();
        assert!(matches!(
            subscription.events.try_recv(),
            Err(TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn the_first_poll_is_a_baseline() {
        let h = Harness::new();
        h.source.add("i", "old-1", NOW - 10_000);
        h.source.add("i", "old-2", NOW - 5_000);
        let mut subscription = h.feed.subscribe();
        assert_eq!(subscription.status.borrow().state, FeedState::Starting);
        wait_ok(&mut subscription).await;
        assert_quiet(&h, &mut subscription).await;

        h.source.add("i", "new-1", NOW - 1_000);
        assert_eq!(next_id(&mut subscription).await, "new-1");
        assert_quiet(&h, &mut subscription).await;
        assert_eq!(h.stats().baselines.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn no_subscribers_means_no_polling() {
        let h = Harness::new();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(h.source.scans(), 0);

        let subscription = h.feed.subscribe();
        wait_for(|| h.polls() >= 2).await;
        drop(subscription);
        wait_for(|| h.source.in_flight.load(Ordering::SeqCst) == 0).await;
        let scans = h.source.scans();
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(h.source.scans(), scans);
        assert_eq!(h.feed.subscribers(), 0);
        assert!(h.feed.shared.lock_state().task.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_first_subscribers_start_one_task() {
        let h = Harness::new();
        let barrier = Arc::new(tokio::sync::Barrier::new(16));
        let mut handles = Vec::new();
        for _ in 0..16 {
            let feed = h.feed.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                feed.subscribe()
            }));
        }
        let mut subscriptions = Vec::new();
        for handle in handles {
            subscriptions.push(handle.await.unwrap());
        }
        assert_eq!(h.stats().tasks.load(Ordering::SeqCst), 1);
        assert_eq!(h.feed.subscribers(), 16);
        for subscription in subscriptions.iter_mut() {
            wait_ok(subscription).await;
        }

        h.source.add("i", "a", NOW);
        for subscription in subscriptions.iter_mut() {
            assert_eq!(next_id(subscription).await, "a");
        }
        assert_eq!(h.stats().baselines.load(Ordering::SeqCst), 1);
        assert_eq!(h.stats().emitted.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn subscribers_share_the_feed_and_one_leaving_keeps_it() {
        let h = Harness::new();
        let mut a = h.feed.subscribe();
        wait_ok(&mut a).await;
        let mut b = h.feed.subscribe();
        // A new subscriber gets the current status, not a fresh start.
        assert_eq!(b.status.borrow_and_update().state, FeedState::Ok);

        h.source.add("i", "1", NOW);
        assert_eq!(next_id(&mut a).await, "1");
        assert_eq!(next_id(&mut b).await, "1");

        drop(a);
        h.source.add("i", "2", NOW);
        assert_eq!(next_id(&mut b).await, "2");
        assert_eq!(h.stats().tasks.load(Ordering::SeqCst), 1);
        assert_eq!(h.stats().baselines.load(Ordering::SeqCst), 1);
        assert_eq!(h.stats().emitted.load(Ordering::SeqCst), 2);
        assert_quiet(&h, &mut b).await;
    }

    #[tokio::test]
    async fn the_last_subscriber_leaving_cancels_a_request_in_flight() {
        let h = Harness::new();
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        *h.source.scan_gate.lock().unwrap() = Some(gate);
        let subscription = h.feed.subscribe();
        wait_for(|| h.source.in_flight.load(Ordering::SeqCst) == 1).await;
        drop(subscription);
        wait_for(|| h.source.in_flight.load(Ordering::SeqCst) == 0).await;
        assert_eq!(h.source.cancelled.load(Ordering::SeqCst), 1);
        assert_eq!(h.source.scans(), 1);
        assert!(h.feed.shared.lock_state().task.is_none());
    }

    #[tokio::test]
    async fn the_last_subscriber_leaving_stops_retrying() {
        let h = Harness::new();
        h.source.state.lock().unwrap().scan_error = Some("connection refused".into());
        let mut subscription = h.feed.subscribe();
        wait_status(&mut subscription, |s| s.state == FeedState::Retrying).await;
        assert_eq!(
            subscription.status.borrow().error.as_deref(),
            Some("connection refused")
        );
        drop(subscription);
        wait_for(|| h.source.in_flight.load(Ordering::SeqCst) == 0).await;
        let scans = h.source.scans();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(h.source.scans(), scans);
    }

    #[tokio::test]
    async fn resubscribing_starts_over_with_a_new_baseline() {
        let h = Harness::new();
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        h.source.add("i", "seen", NOW);
        assert_eq!(next_id(&mut subscription).await, "seen");
        drop(subscription);

        // Arrives while nobody is listening: not collected or replayed.
        h.source.add("i", "unheard", NOW);
        let mut subscription = h.feed.subscribe();
        assert_eq!(subscription.status.borrow().state, FeedState::Starting);
        wait_ok(&mut subscription).await;
        assert_quiet(&h, &mut subscription).await;
        h.source.add("i", "fresh", NOW);
        assert_eq!(next_id(&mut subscription).await, "fresh");
        assert_eq!(h.stats().tasks.load(Ordering::SeqCst), 2);
        assert_eq!(h.stats().baselines.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_stopped_task_cannot_reach_a_new_period() {
        let h = Harness::new();
        let mut first = h.feed.subscribe();
        wait_ok(&mut first).await;
        let old = h.feed.shared.lock_state().generation;
        drop(first);
        let mut second = h.feed.subscribe();
        assert_ne!(h.feed.shared.lock_state().generation, old);
        let epoch = h.feed.shared.ingest_epoch.load(Ordering::SeqCst);
        assert!(
            !h.feed
                .shared
                .emit(old, epoch, vec![Arc::new(json!({"id": "stale"}))])
        );
        h.feed.shared.set_status(
            old,
            FeedStatus {
                error: Some("stale".into()),
                ..FeedStatus::ingest()
            },
        );
        assert!(second.status.borrow().error.is_none());
        wait_ok(&mut second).await;
        h.source.add("i", "fresh", NOW);
        assert_eq!(next_id(&mut second).await, "fresh");
    }

    #[tokio::test]
    async fn a_panicking_poll_is_reported_and_polling_goes_on() {
        let h = Harness::new();
        h.source.state.lock().unwrap().panic_scans = 1;
        let mut subscription = h.feed.subscribe();
        wait_status(&mut subscription, |s| {
            s.state == FeedState::Retrying && s.error.as_deref() == Some("internal error")
        })
        .await;
        wait_ok(&mut subscription).await;
        h.source.add("i", "after", NOW);
        assert_eq!(next_id(&mut subscription).await, "after");
        assert_eq!(h.stats().tasks.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn ties_across_page_boundaries_are_not_skipped() {
        // Pages of 4, with the boundaries inside groups of equal timestamps.
        let h = Harness::new();
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        let mut want = Vec::new();
        for (ts, n) in [
            (NOW, 1),
            (NOW - 1, 3),
            (NOW - 2, 3),
            (NOW - 3, 2),
            (NOW - 9, 3),
        ] {
            for i in 0..n {
                let id = format!("{ts}-{i}");
                h.source.add("i", &id, ts);
                want.push(id);
            }
        }
        let got = next_ids(&mut subscription, want.len()).await;
        assert_eq!(sorted(got), sorted(want));
        assert_quiet(&h, &mut subscription).await;
        assert!(!subscription.status.borrow().behind);
    }

    #[tokio::test]
    async fn more_ties_than_a_page_are_read_at_once() {
        let h = Harness::new();
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        let mut want = Vec::new();
        for i in 0..7 {
            let id = format!("tie-{i}");
            h.source.add("i", &id, NOW - 5);
            want.push(id);
        }
        h.source.add("i", "older", NOW - 10);
        want.push("older".to_string());
        let got = next_ids(&mut subscription, want.len()).await;
        assert_eq!(sorted(got), sorted(want));
        assert_quiet(&h, &mut subscription).await;
        assert!(!subscription.status.borrow().behind);
    }

    #[tokio::test]
    async fn more_ties_than_can_be_read_are_reported() {
        let h = Harness::with(|config| config.tie_page = 5);
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        for i in 0..7 {
            h.source.add("i", &format!("tie-{i}"), NOW - 5);
        }
        h.source.add("i", "older", NOW - 10);
        // Five of the ties fit, then past them to the older one.
        let got = next_ids(&mut subscription, 6).await;
        assert!(got.contains(&"older".to_string()));
        wait_status(&mut subscription, |s| s.behind).await;
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn a_poll_reads_at_most_max_scan_hits() {
        let h = Harness::with(|config| config.max_scan = 10);
        for i in 0..40 {
            h.source.add("i", &format!("{i}"), NOW - i * 10);
        }
        let subscription = h.feed.subscribe();
        h.wait_polls(1).await;
        drop(subscription);
        wait_for(|| h.source.in_flight.load(Ordering::SeqCst) == 0).await;
        // Repeats from the overlap count against the budget too.
        let polls = h.polls() as usize;
        assert!(h.source.hits.load(Ordering::SeqCst) <= polls * 10);
    }

    #[tokio::test]
    async fn late_documents_within_the_lookback_are_found() {
        let h = Harness::new();
        h.source.add("i", "baseline", NOW - 1_000);
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;

        // Older than what was already seen, but within the lookback.
        h.source.add("i", "late", NOW - 120_000);
        assert_eq!(next_id(&mut subscription).await, "late");

        // Indexed, but not yet visible to search.
        h.source.add_hidden("i", "refresh", NOW - 2_000);
        assert_quiet(&h, &mut subscription).await;
        h.source.reveal("i", "refresh");
        assert_eq!(next_id(&mut subscription).await, "refresh");

        // Older than the lookback: missed.
        h.source.add("i", "too-late", NOW - 301_000);
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn documents_are_emitted_once_as_polls_overlap() {
        let h = Harness::new();
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        let mut want = Vec::new();
        for i in 0..20 {
            let id = format!("doc-{i}");
            h.source.add("i", &id, NOW + i);
            want.push(id);
            h.advance(1_000);
            h.wait_polls(1).await;
        }
        let got = next_ids(&mut subscription, want.len()).await;
        assert_eq!(sorted(got), sorted(want));
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn the_same_id_in_two_indices_is_two_documents() {
        let h = Harness::new();
        h.source.add("logstash-2026.11.07", "x", NOW - 1_000);
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        // Same id in the next day's index, as after a rollover.
        h.source
            .add_event("logstash-2026.11.08", "x", NOW, json!({"id": "x-next-day"}));
        assert_eq!(next_id(&mut subscription).await, "x-next-day");
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn changed_documents_are_not_emitted_again() {
        let h = Harness::with(|config| config.clock_step = Duration::from_secs(3600));
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        h.source.add("i", "a", NOW - 1_000);
        assert_eq!(next_id(&mut subscription).await, "a");

        // Archived, say: same document, new content.
        h.source.state.lock().unwrap().docs[0].event = json!({"id": "a", "tags": ["archived"]});
        assert_quiet(&h, &mut subscription).await;

        // Re-indexed with a later timestamp, then the old one ages out.
        h.source.state.lock().unwrap().docs[0].ts = NOW + 200_000;
        h.wait_polls(2).await;
        h.advance(250_000);
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn a_failed_baseline_is_retried_without_a_burst() {
        let h = Harness::new();
        for i in 0..10 {
            h.source.add("i", &format!("old-{i}"), NOW - i);
        }
        h.source.state.lock().unwrap().scan_error = Some("unavailable".into());
        let mut subscription = h.feed.subscribe();
        wait_status(&mut subscription, |s| s.state == FeedState::Retrying).await;
        h.source.state.lock().unwrap().scan_error = None;
        wait_ok(&mut subscription).await;
        assert_quiet(&h, &mut subscription).await;
        assert_eq!(h.stats().emitted.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_partial_baseline_is_not_used() {
        let h = Harness::new();
        // Three pages of four.
        for i in 0..10 {
            h.source.add("i", &format!("old-{i}"), NOW - i * 10);
        }
        // The first poll fails on its second page.
        h.source.state.lock().unwrap().fail_scan_after = Some(1);
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        assert_quiet(&h, &mut subscription).await;
        assert_eq!(h.stats().emitted.load(Ordering::SeqCst), 0);
        assert_eq!(h.stats().baselines.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_baseline_from_part_of_the_data_is_retried_then_used() {
        let h = Harness::new();
        h.source.add("i", "old", NOW - 1_000);
        // Fewer than the retries, leaving out the only index: retried until
        // complete, so the baseline has the document.
        {
            let mut state = h.source.state.lock().unwrap();
            state.partial_scans = 2;
            state.partial_index = Some("i".into());
        }
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        assert_eq!(h.source.scans(), 3);
        assert_eq!(h.stats().seen.load(Ordering::SeqCst), 1);
        assert_eq!(h.stats().baselines.load(Ordering::SeqCst), 1);
        assert_quiet(&h, &mut subscription).await;
        drop(subscription);

        // A part that keeps failing doesn't stop the feed, but is reported.
        let h = Harness::new();
        {
            let mut state = h.source.state.lock().unwrap();
            state.partial_scans = 1000;
            state.partial_index = Some("broken".into());
        }
        let mut subscription = h.feed.subscribe();
        wait_status(&mut subscription, |s| s.state == FeedState::Retrying).await;
        wait_status(&mut subscription, |s| {
            s.state == FeedState::Ok && s.behind && s.partial.is_some()
        })
        .await;
        h.source.add("i", "new", NOW);
        assert_eq!(next_id(&mut subscription).await, "new");
    }

    #[tokio::test]
    async fn a_part_that_stops_answering_is_reported() {
        let h = Harness::new();
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        {
            let mut state = h.source.state.lock().unwrap();
            state.partial_index = Some("b".into());
            state.partial_scans = 1_000_000;
        }
        // Behind, with the reason, while the rest is still polled.
        wait_status(&mut subscription, |s| {
            s.state == FeedState::Ok
                && s.behind
                && s.partial.as_deref() == Some("1 of 2 shards failed")
        })
        .await;
        h.source.add("a", "answering", NOW);
        assert_eq!(next_id(&mut subscription).await, "answering");
        // Clear once it answers again.
        h.source.state.lock().unwrap().partial_scans = 0;
        wait_status(&mut subscription, |s| !s.behind && s.partial.is_none()).await;
    }

    #[tokio::test]
    async fn what_a_missing_part_got_meanwhile_is_emitted_once_it_answers() {
        let h = Harness::new();
        h.source.add("a", "a-old", NOW - 1_000);
        h.source.add("b", "b-old", NOW - 1_000);
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;

        // Index "b" stops answering, and gets a document the polls can't
        // see, while the rest is still polled.
        {
            let mut state = h.source.state.lock().unwrap();
            state.partial_index = Some("b".into());
            state.partial_scans = 1_000_000;
        }
        h.source.add("b", "b-meanwhile", NOW - 500);
        h.source.add("a", "a-new", NOW);
        assert_eq!(next_id(&mut subscription).await, "a-new");
        // Once it answers again, that is emitted, late, and new documents
        // elsewhere still are.
        h.source.state.lock().unwrap().partial_scans = 0;
        assert_eq!(next_id(&mut subscription).await, "b-meanwhile");
        wait_status(&mut subscription, |s| !s.behind && s.partial.is_none()).await;
        h.source.add("a", "a-newer", NOW);
        assert_eq!(next_id(&mut subscription).await, "a-newer");
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn what_a_partial_baseline_missed_is_not_emitted() {
        let h = Harness::new();
        h.source.add("b", "b-old", NOW - 1_000);
        // Index "b" doesn't answer the baseline, even when retried.
        {
            let mut state = h.source.state.lock().unwrap();
            state.partial_index = Some("b".into());
            state.partial_scans = 1_000_000;
        }
        let mut subscription = h.feed.subscribe();
        wait_status(&mut subscription, |s| s.state == FeedState::Ok).await;
        h.source.add("b", "b-meanwhile", NOW);
        h.source.add("a", "a-new", NOW);
        assert_eq!(next_id(&mut subscription).await, "a-new");
        // Once it answers, what it had before the baseline is history, and
        // what it got since is new.
        h.source.state.lock().unwrap().partial_scans = 0;
        assert_eq!(next_id(&mut subscription).await, "b-meanwhile");
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn documents_dated_in_the_future_cant_hide_the_present() {
        // A sensor whose clock runs ahead has more documents in the future
        // than a poll reads.
        let h = Harness::with(|config| config.max_scan = 8);
        for i in 0..12 {
            h.source
                .add("ahead", &format!("ahead-{i}"), NOW + 60_000 + i);
        }
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        h.source.add("i", "present", NOW - 10);
        assert_eq!(next_id(&mut subscription).await, "present");
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn a_baseline_cut_short_in_a_millisecond_doesnt_replay_the_rest_of_it() {
        let h = Harness::with(|config| config.max_scan = 8);
        for i in 0..6 {
            h.source.add("i", &format!("top-{i}"), NOW - 10 - i);
        }
        // The baseline's budget runs out after the first of these.
        for i in 0..4 {
            h.source.add("i", &format!("cut-{i}"), NOW - 100);
        }
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        // With what was above gone, polls could read further into that
        // millisecond.
        h.source
            .state
            .lock()
            .unwrap()
            .docs
            .retain(|d| !d.doc.id.starts_with("top-"));
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn a_newer_millisecond_too_full_to_read_is_reported() {
        let h = Harness::with(|config| config.tie_page = 5);
        for i in 0..7 {
            h.source.add("i", &format!("old-{i}"), NOW - 10);
        }
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        assert!(!subscription.status.borrow().behind);
        // Too many to read in a newer millisecond, above another one.
        for i in 0..7 {
            h.source.add("i", &format!("new-{i}"), NOW - 5);
        }
        wait_status(&mut subscription, |s| s.behind).await;
    }

    #[tokio::test]
    async fn a_failed_fetch_is_retried() {
        let h = Harness::new();
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        h.source.state.lock().unwrap().fetch_error = Some("timed out".into());
        h.source.add("i", "a", NOW);
        wait_status(&mut subscription, |s| s.state == FeedState::Retrying).await;
        h.source.state.lock().unwrap().fetch_error = None;
        assert_eq!(next_id(&mut subscription).await, "a");
        wait_ok(&mut subscription).await;
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn a_document_the_fetch_misses_is_fetched_again() {
        let h = Harness::new();
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        // Searchable on one copy of a shard, not yet on another.
        h.source
            .state
            .lock()
            .unwrap()
            .fetch_misses
            .insert(doc("i", "lagging"), 2);
        h.source.add("i", "lagging", NOW);
        assert_eq!(next_id(&mut subscription).await, "lagging");
        assert_quiet(&h, &mut subscription).await;

        // One that never comes back is given up on, rather than fetched
        // forever.
        h.source
            .state
            .lock()
            .unwrap()
            .fetch_misses
            .insert(doc("i", "gone"), 1000);
        h.source.add("i", "gone", NOW);
        h.wait_polls(FETCH_ATTEMPTS as u64 + 1).await;
        let fetches = h.source.fetches.load(Ordering::SeqCst);
        assert_quiet(&h, &mut subscription).await;
        assert_eq!(h.source.fetches.load(Ordering::SeqCst), fetches);
    }

    #[tokio::test]
    async fn a_busy_window_raises_the_floor_without_duplicates() {
        // A poll reads at most 8 hits, two pages.
        let h = Harness::with(|config| config.max_scan = 8);
        for i in 0..20 {
            h.source
                .add("i", &format!("old-{i}"), NOW - 1_000 - i * 100);
        }
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        // Only the newest fit, so the lookback is cut short.
        assert!(subscription.status.borrow().lookback.unwrap() < 300);

        h.source.add("i", "new", NOW);
        assert_eq!(next_id(&mut subscription).await, "new");
        // The load drops, so a poll could reach further back, but the
        // documents below the floor are not emitted as new.
        h.source.state.lock().unwrap().docs.retain(|d| {
            let n: Option<i64> = d.doc.id.strip_prefix("old-").and_then(|n| n.parse().ok());
            n.is_none_or(|n| n >= 10)
        });
        h.wait_polls(2).await;
        h.advance(5_000);
        assert_quiet(&h, &mut subscription).await;
        assert!(!subscription.status.borrow().behind);
    }

    #[tokio::test]
    async fn falling_behind_is_reported() {
        let h = Harness::with(|config| config.max_scan = 8);
        h.source.add("i", "baseline", NOW - 1_000);
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        // More new documents at once than a poll reads.
        for i in 0..12 {
            h.source.add("i", &format!("burst-{i}"), NOW - 500 + i);
        }
        let mut got = next_ids(&mut subscription, 4).await;
        wait_status(&mut subscription, |s| s.behind).await;
        // Caught up once the burst is over.
        wait_status(&mut subscription, |s| !s.behind).await;
        while let Ok(event) = subscription.events.try_recv() {
            got.push(event["id"].as_str().unwrap().to_string());
        }
        // Each once, and the oldest of the burst were skipped.
        assert!(got.iter().all(|id| id.starts_with("burst-")));
        let distinct: HashSet<&String> = got.iter().collect();
        assert_eq!(distinct.len(), got.len());
        assert!(got.len() < 12);
        assert!(!got.contains(&"burst-0".to_string()));
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn what_is_remembered_is_bounded() {
        let h = Harness::with(|config| config.max_seen = 5);
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        let mut want = Vec::new();
        for i in 0..12 {
            let id = format!("doc-{i:02}");
            h.source.add("i", &id, NOW + i);
            want.push(id);
            h.wait_polls(1).await;
        }
        let got = next_ids(&mut subscription, want.len()).await;
        assert_eq!(sorted(got), sorted(want));
        assert!(h.stats().seen.load(Ordering::SeqCst) <= 5);
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn a_clock_stepped_forward_starts_over_without_a_burst() {
        let h = Harness::new();
        // Dated beyond the future allowance of a clock running 20 minutes
        // slow, as before NTP has synchronized it.
        for i in 0..5 {
            h.source
                .add("i", &format!("ahead-{i}"), NOW + 1_200_000 - i);
        }
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        h.advance(1_200_000);
        wait_for(|| h.stats().baselines.load(Ordering::SeqCst) == 2).await;
        assert_quiet(&h, &mut subscription).await;
        h.source.add("i", "after", NOW + 1_200_000);
        assert_eq!(next_id(&mut subscription).await, "after");
    }

    #[tokio::test]
    async fn a_clock_stepped_back_starts_over_and_repeats_nothing() {
        let h = Harness::new();
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        h.source.add("i", "a", NOW);
        assert_eq!(next_id(&mut subscription).await, "a");
        // Back far enough that nothing current would be in the window, if
        // it didn't start over.
        h.advance(-1_200_000);
        wait_for(|| h.stats().baselines.load(Ordering::SeqCst) == 2).await;
        assert_quiet(&h, &mut subscription).await;
        h.source.add("i", "b", NOW - 1_200_000);
        assert_eq!(next_id(&mut subscription).await, "b");
    }

    #[tokio::test]
    async fn a_slow_subscriber_does_not_hold_up_the_others() {
        let h = Harness::with(|config| config.capacity = 4);
        let mut slow = h.feed.subscribe();
        let mut fast = h.feed.subscribe();
        wait_ok(&mut fast).await;
        let mut got = Vec::new();
        for i in 0..12 {
            h.source.add("i", &format!("{i}"), NOW + i);
            got.push(next_id(&mut fast).await);
        }
        assert_eq!(got.len(), 12);
        assert!(matches!(
            slow.events.recv().await,
            Err(RecvError::Lagged(_))
        ));
    }

    #[tokio::test]
    async fn configured_inputs_mean_no_polling() {
        let h = Harness::new();
        h.feed.set_ingest_configured(true);
        let mut subscription = h.feed.subscribe();
        assert_eq!(
            *subscription.status.borrow_and_update(),
            FeedStatus::ingest()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(h.source.scans(), 0);
        h.feed.publish_ingested(&json!({"id": "ingested"}));
        assert_eq!(next_id(&mut subscription).await, "ingested");
    }

    #[tokio::test]
    async fn ingest_stops_polling_and_idle_ingest_resumes_it() {
        let h = Harness::with(|config| config.ingest_idle = Duration::from_millis(500));
        h.source.add("i", "before", NOW - 1_000);
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;

        h.feed.publish_ingested(&json!({"id": "ingested"}));
        assert_eq!(next_id(&mut subscription).await, "ingested");
        wait_status(&mut subscription, |s| s.source == FeedSourceKind::Ingest).await;
        let scans = h.source.scans();
        // Indexed by EveBox after publishing: not emitted again by a poll.
        h.source
            .add_event("i", "ingested", NOW, json!({"id": "ingested"}));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(h.source.scans(), scans);

        // Ingest goes idle: back to polling, with a new baseline, so what
        // was ingested isn't emitted.
        wait_ok(&mut subscription).await;
        assert_eq!(h.stats().baselines.load(Ordering::SeqCst), 2);
        assert_quiet(&h, &mut subscription).await;
        h.source.add("i", "external", NOW);
        assert_eq!(next_id(&mut subscription).await, "external");
    }

    #[tokio::test]
    async fn ingest_in_progress_means_no_polling_however_long_it_takes() {
        let h = Harness::with(|config| config.ingest_idle = Duration::from_millis(50));
        let ingesting = h.feed.ingesting();
        let mut subscription = h.feed.subscribe();
        assert_eq!(
            subscription.status.borrow_and_update().source,
            FeedStatus::ingest().source
        );
        // Well past the idle period: still ingesting.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(h.source.scans(), 0);
        // Indexed, then published once ingest completes.
        h.source
            .add_event("i", "ingested", NOW, json!({"id": "ingested"}));
        drop(ingesting);
        h.feed.publish_ingested_all(vec![json!({"id": "ingested"})]);
        assert_eq!(next_id(&mut subscription).await, "ingested");
        // Once idle, polling starts over and doesn't repeat it.
        wait_ok(&mut subscription).await;
        assert_quiet(&h, &mut subscription).await;
    }

    #[tokio::test]
    async fn a_poll_racing_ingest_is_discarded() {
        let h = Harness::new();
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        *h.source.fetch_gate.lock().unwrap() = Some(gate.clone());
        h.source.add("i", "polled", NOW);
        wait_for(|| h.source.fetches.load(Ordering::SeqCst) == 1).await;
        // Ingest starts while the poll is fetching.
        h.feed.publish_ingested(&json!({"id": "ingested"}));
        gate.add_permits(100);
        assert_eq!(next_id(&mut subscription).await, "ingested");
        wait_status(&mut subscription, |s| s.source == FeedSourceKind::Ingest).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(matches!(
            subscription.events.try_recv(),
            Err(TryRecvError::Empty)
        ));
        assert_eq!(h.stats().emitted.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_poll_outlasting_brief_ingest_is_still_discarded() {
        // Ingest that is idle again by the time a slow poll finishes.
        let h = Harness::with(|config| config.ingest_idle = Duration::from_millis(1));
        let mut subscription = h.feed.subscribe();
        wait_ok(&mut subscription).await;
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        *h.source.fetch_gate.lock().unwrap() = Some(gate.clone());
        h.source.add("i", "ingested", NOW);
        wait_for(|| h.source.fetches.load(Ordering::SeqCst) == 1).await;
        h.feed.publish_ingested(&json!({"id": "ingested"}));
        assert_eq!(next_id(&mut subscription).await, "ingested");
        tokio::time::sleep(Duration::from_millis(20)).await;
        gate.add_permits(100);
        // Starts over instead, so it's never emitted.
        wait_for(|| h.stats().baselines.load(Ordering::SeqCst) == 2).await;
        assert_quiet(&h, &mut subscription).await;
        assert_eq!(h.stats().emitted.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn recent_ingest_means_no_polling_for_a_new_subscriber() {
        let h = Harness::new();
        h.feed.publish_ingested(&json!({"id": "earlier"}));
        let mut subscription = h.feed.subscribe();
        assert_eq!(
            subscription.status.borrow_and_update().source,
            FeedSourceKind::Ingest
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(h.source.scans(), 0);
    }

    #[test]
    fn status_serializes_compactly() {
        assert_eq!(
            serde_json::to_value(FeedStatus::ingest()).unwrap(),
            json!({"source": "ingest", "state": "ok", "behind": false})
        );
        let config = FeedConfig::default();
        assert_eq!(
            serde_json::to_value(FeedStatus::starting(&config)).unwrap(),
            json!({
                "source": "datastore",
                "state": "starting",
                "interval": 1.0,
                "lookback": 300,
                "behind": false,
            })
        );
    }

    #[test]
    fn a_failed_poll_reports_only_the_failure() {
        let mut poller = Poller::new(FeedConfig::default());
        poller.baseline = true;
        poller.behind = true;
        poller.partial = Some("1 of 2 shards failed".into());
        let status = poller.failed_status(&anyhow!("connection refused"));
        assert_eq!(status.state, FeedState::Retrying);
        assert_eq!(status.error.as_deref(), Some("connection refused"));
        assert!(!status.behind);
        assert_eq!(status.partial, None);
    }

    #[test]
    fn backoff_doubles_up_to_the_cap() {
        let config = FeedConfig::default();
        assert_eq!(backoff(&config, 1), Duration::from_secs(2));
        assert_eq!(backoff(&config, 2), Duration::from_secs(4));
        assert_eq!(backoff(&config, 3), Duration::from_secs(8));
        assert_eq!(backoff(&config, 4), Duration::from_secs(10));
        assert_eq!(backoff(&config, 100), Duration::from_secs(10));
    }

    #[test]
    fn long_errors_are_truncated() {
        let poller = Poller::new(FeedConfig::default());
        let status = poller.failed_status(&anyhow!("é".repeat(400)));
        let error = status.error.unwrap();
        assert!(error.len() <= MAX_ERROR_LEN + '…'.len_utf8());
        assert!(error.ends_with('…'));
    }
}
