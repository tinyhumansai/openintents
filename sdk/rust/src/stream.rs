//! Live timelines over Server-Sent Events.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;
use reqwest::{Method, RequestBuilder, StatusCode};

use crate::client::{api_error, backoff, retry_after, retryable, Client};
use crate::error::{Error, Result};
use crate::sse::SseParser;
use crate::types::Event;

/// An intent's or a session's timeline: past events first, then new ones as
/// they happen. A stream that closes or drops is resumed after the last event
/// received (`Last-Event-ID`) until the API answers `204`: the timeline is
/// over and every event was sent. Connections that fail before sending
/// anything are retried like any request; an error ends the stream.
///
/// Read it with [`EventStream::next`], or as a [`Stream`]. Nothing is sent
/// until it is first polled, and dropping it closes the connection.
#[must_use = "an event stream does nothing until it is polled"]
pub struct EventStream {
    state: State,
}

type Step = Pin<Box<dyn Future<Output = (Box<Follower>, Option<Result<Event>>)> + Send>>;

enum State {
    Idle(Box<Follower>),
    Busy(Step),
    Done,
}

/// How one attempt to connect went, when it did not end the stream.
enum Connect {
    Open(Connection),
    /// `204`: the timeline is over and every event was sent.
    Over,
    Retry(Error, Option<f64>),
}

struct Connection {
    res: reqwest::Response,
    parser: SseParser,
    sent_anything: bool,
}

struct Follower {
    client: Client,
    path: String,
    last_event_id: Option<String>,
    failures: u32,
    conn: Option<Connection>,
    ready: VecDeque<Result<Event>>,
}

impl EventStream {
    pub(crate) fn new(client: Client, path: String, last_event_id: Option<&str>) -> Self {
        let follower = Follower {
            client,
            path,
            last_event_id: last_event_id.map(str::to_string),
            failures: 0,
            conn: None,
            ready: VecDeque::new(),
        };
        Self {
            state: State::Idle(Box::new(follower)),
        }
    }

    /// The next event, or `None` once the timeline is over or an error was
    /// returned.
    pub async fn next(&mut self) -> Option<Result<Event>> {
        std::future::poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }
}

impl Stream for EventStream {
    type Item = Result<Event>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match std::mem::replace(&mut self.state, State::Done) {
                State::Idle(follower) => self.state = State::Busy(Box::pin(follower.step())),
                State::Busy(mut step) => {
                    return match step.as_mut().poll(cx) {
                        Poll::Pending => {
                            self.state = State::Busy(step);
                            Poll::Pending
                        }
                        Poll::Ready((follower, Some(Ok(event)))) => {
                            self.state = State::Idle(follower);
                            Poll::Ready(Some(Ok(event)))
                        }
                        // The end of the timeline, or an error, ends the stream.
                        Poll::Ready((_, item)) => Poll::Ready(item),
                    };
                }
                State::Done => return Poll::Ready(None),
            }
        }
    }
}

/// One attempt to connect. An error that retrying cannot fix ends the stream.
async fn connect(req: RequestBuilder) -> Result<Connect> {
    let res = match req.send().await {
        Ok(res) => res,
        Err(err) => return Ok(Connect::Retry(err.into(), None)),
    };
    let status = res.status();
    if status == StatusCode::NO_CONTENT {
        return Ok(Connect::Over);
    }
    if status.is_success() {
        return Ok(Connect::Open(Connection {
            res,
            parser: SseParser::default(),
            sent_anything: false,
        }));
    }
    let after = retry_after(&res);
    let error = api_error(res).await;
    if retryable(status) {
        Ok(Connect::Retry(error, after))
    } else {
        Err(error)
    }
}

impl std::fmt::Debug for EventStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventStream").finish_non_exhaustive()
    }
}

impl Follower {
    async fn step(mut self: Box<Self>) -> (Box<Self>, Option<Result<Event>>) {
        let item = self.advance().await;
        (self, item)
    }

    async fn advance(&mut self) -> Option<Result<Event>> {
        loop {
            if let Some(item) = self.ready.pop_front() {
                return Some(item);
            }
            let Some(conn) = self.conn.as_mut() else {
                match connect(self.request()).await {
                    Ok(Connect::Open(conn)) => self.conn = Some(conn),
                    Ok(Connect::Over) => return None,
                    Ok(Connect::Retry(error, after)) => {
                        if let Err(error) = self.back_off(error, after).await {
                            return Some(Err(error));
                        }
                    }
                    Err(error) => return Some(Err(error)),
                }
                continue;
            };
            match conn.res.chunk().await {
                Ok(Some(chunk)) => {
                    if !chunk.is_empty() {
                        // Anything arriving, a keep-alive too, shows the stream works.
                        conn.sent_anything = true;
                        self.failures = 0;
                    }
                    let messages = conn.parser.feed(Some(&chunk));
                    self.queue(messages);
                }
                Ok(None) => {
                    let mut conn = self.conn.take().expect("a connection is open");
                    let messages = conn.parser.feed(None);
                    self.queue(messages);
                    // The API closes a stream now and then: resume at once. One
                    // that closes before sending a byte counts as a failure, so
                    // it cannot spin.
                    if !conn.sent_anything {
                        if let Err(error) = self.back_off(Error::StreamClosed, None).await {
                            return Some(Err(error));
                        }
                    }
                }
                Err(err) => {
                    self.conn = None;
                    if let Err(error) = self.back_off(err.into(), None).await {
                        return Some(Err(error));
                    }
                }
            }
        }
    }

    fn queue(&mut self, messages: Vec<(Option<String>, String)>) {
        for (id, data) in messages {
            match serde_json::from_str::<Event>(&data) {
                Ok(event) => {
                    self.last_event_id = Some(id.unwrap_or_else(|| event.id.clone()));
                    self.ready.push_back(Ok(event));
                }
                Err(err) => self.ready.push_back(Err(err.into())),
            }
        }
    }

    fn request(&self) -> RequestBuilder {
        let req = self.client.request(Method::GET, &self.path, "text/event-stream", None);
        match &self.last_event_id {
            Some(id) => req.header("Last-Event-ID", id),
            None => req,
        }
    }

    /// Waits before the next connection, or gives `error` back once the
    /// retries are spent.
    async fn back_off(&mut self, error: Error, retry_after: Option<f64>) -> Result<()> {
        if self.failures >= self.client.max_retries {
            return Err(error);
        }
        tokio::time::sleep(backoff(self.failures, retry_after)).await;
        self.failures += 1;
        Ok(())
    }
}
