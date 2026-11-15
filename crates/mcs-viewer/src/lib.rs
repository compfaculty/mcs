//! Read-only paged viewing with one bounded, cancellable worker per application.
pub mod text;
use std::{
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc,
    },
    thread,
    time::Duration,
};
pub use text::{display_text, Movement, Page, Viewport};

pub enum Action {
    Open(PathBuf, Viewport),
    View {
        top: u64,
        movement: Movement,
        viewport: Viewport,
    },
}
pub enum Event {
    Progress { scanned: u64, total: u64 },
    Media(mcs_media::Kind),
    Video(mcs_media::video::Playback),
    Page(Page),
    Error(String),
}
struct Request {
    generation: u64,
    action: Action,
}

pub struct Client {
    requests: SyncSender<Request>,
    results: Receiver<(u64, Event)>,
    cancel: Arc<AtomicU64>,
    queued: Option<Request>,
    pub busy: bool,
}
impl Client {
    pub fn new() -> io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<Request>(1);
        let (sender, results) = mpsc::sync_channel(2);
        let cancel = Arc::new(AtomicU64::new(0));
        let token = cancel.clone();
        thread::Builder::new()
            .name("mcs-viewer".into())
            .spawn(move || {
                let mut document: Option<text::Document> = None;
                let mut active = 0;
                let mut last_viewport = Viewport::default();
                let mut last_progress = 0;
                loop {
                    match receiver.recv_timeout(Duration::from_millis(10)) {
                        Ok(request) => {
                            if token.load(Ordering::Relaxed) != request.generation {
                                continue;
                            }
                            active = request.generation;
                            let result = match request.action {
                                Action::Open(path, viewport) => {
                                    last_viewport = viewport;
                                    document = None;
                                    mcs_media::classify(&path).and_then(|kind| {
                                        if kind == mcs_media::Kind::Video {
                                            return mcs_media::video::Playback::prepare(
                                                &path, &token, active,
                                            )
                                            .map(Event::Video);
                                        }
                                        if kind != mcs_media::Kind::Text {
                                            return Ok(Event::Media(kind));
                                        }
                                        let mut doc = text::Document::open(&path)?;
                                        let page =
                                            doc.page(Movement::Home, viewport, &token, active)?;
                                        document = Some(doc);
                                        Ok(Event::Page(page))
                                    })
                                }
                                Action::View {
                                    top,
                                    movement,
                                    viewport,
                                } => {
                                    last_viewport = viewport;
                                    document
                                        .as_mut()
                                        .ok_or_else(|| io::Error::other("No text document open"))
                                        .and_then(|doc| {
                                            doc.set_top(top);
                                            doc.page_with_progress(
                                                movement,
                                                viewport,
                                                &token,
                                                active,
                                                |scanned, total| {
                                                    let _ = sender.try_send((
                                                        active,
                                                        Event::Progress { scanned, total },
                                                    ));
                                                },
                                            )
                                            .map(Event::Page)
                                        })
                                }
                            };
                            if token.load(Ordering::Relaxed) == active {
                                let event = match result {
                                    Ok(event) => event,
                                    Err(error) => Event::Error(error.to_string()),
                                };
                                // Bounded send is safe: dropping the client disconnects the receiver.
                                if sender.send((active, event)).is_err() {
                                    break;
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if token.load(Ordering::Relaxed) == active {
                                if let Some(doc) = document.as_mut() {
                                    match doc.index_step() {
                                        Ok(true) => {
                                            let progress = doc.progress();
                                            if progress != last_progress {
                                                last_progress = progress;
                                                if let Ok(page) = doc.page(
                                                    Movement::Stay,
                                                    last_viewport,
                                                    &token,
                                                    active,
                                                ) {
                                                    let _ = sender
                                                        .try_send((active, Event::Page(page)));
                                                }
                                            }
                                        }
                                        Ok(false) => {}
                                        Err(error) => {
                                            let _ = sender.try_send((
                                                active,
                                                Event::Error(error.to_string()),
                                            ));
                                            document = None;
                                        }
                                    }
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok(Self {
            requests,
            results,
            cancel,
            queued: None,
            busy: false,
        })
    }
    pub fn submit(&mut self, action: Action) {
        let generation = self.cancel.fetch_add(1, Ordering::Relaxed) + 1;
        self.queued = Some(Request { generation, action });
        self.busy = true;
        self.flush();
    }
    fn flush(&mut self) {
        if let Some(request) = self.queued.take() {
            match self.requests.try_send(request) {
                Ok(()) => {}
                Err(TrySendError::Full(request)) => self.queued = Some(request),
                Err(TrySendError::Disconnected(_)) => self.busy = false,
            }
        }
    }
    pub fn poll(&mut self) -> Option<Event> {
        let mut newest = None;
        while let Ok((generation, event)) = self.results.try_recv() {
            if generation == self.cancel.load(Ordering::Relaxed) {
                if !matches!(event, Event::Progress { .. }) {
                    self.busy = false;
                }
                newest = Some(event);
            }
        }
        self.flush();
        newest
    }
    pub fn cancel(&mut self) {
        self.cancel.fetch_add(1, Ordering::Relaxed);
        self.queued = None;
        self.busy = false;
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_results_and_cancelled_requests_never_replace_current_view() {
        let (requests, request_receiver) = mpsc::sync_channel(1);
        let (result_sender, results) = mpsc::sync_channel(2);
        let token = Arc::new(AtomicU64::new(2));
        let mut client = Client {
            requests,
            results,
            cancel: token,
            queued: None,
            busy: true,
        };
        result_sender
            .send((1, Event::Error("stale".into())))
            .ok()
            .unwrap();
        assert!(client.poll().is_none());
        assert!(client.busy);
        result_sender
            .send((
                2,
                Event::Page(Page {
                    len: 123,
                    ..Page::default()
                }),
            ))
            .ok()
            .unwrap();
        assert!(matches!(
            client.poll(),
            Some(Event::Page(Page { len: 123, .. }))
        ));
        assert!(!client.busy);
        client.submit(Action::Open("old".into(), Viewport::default()));
        client.submit(Action::Open("new".into(), Viewport::default()));
        assert!(client.queued.is_some());
        let old = request_receiver.try_recv().unwrap();
        assert_ne!(old.generation, client.cancel.load(Ordering::Relaxed));
        client.cancel();
        assert!(client.queued.is_none());
        assert!(!client.busy);
    }
}
