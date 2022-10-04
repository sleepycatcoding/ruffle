//! Navigator backend for web

use crate::custom_event::RuffleEvent;
use isahc::http::{HeaderName, HeaderValue};
use isahc::{
    config::RedirectPolicy, prelude::*, AsyncReadResponseExt, HttpClient, Request as IsahcRequest,
};
use rfd::{MessageButtons, MessageDialog, MessageLevel};
use ruffle_core::backend::navigator::{
    NavigationMethod, NavigatorBackend, OpenURLMode, XmlSocketBehavior, OwnedFuture, Request, Response,
};
use ruffle_core::indexmap::IndexMap;
use ruffle_core::loader::Error;
use ruffle_core::socket::XmlSocketConnection;
use std::collections::{HashSet, VecDeque};
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use url::Url;
use winit::event_loop::EventLoopProxy;

/// Implementation of `NavigatorBackend` for non-web environments that can call
/// out to a web browser.
pub struct ExternalNavigatorBackend {
    /// Sink for tasks sent to us through `spawn_future`.
    channel: Sender<OwnedFuture<(), Error>>,

    /// Event sink to trigger a new task poll.
    event_loop: EventLoopProxy<RuffleEvent>,

    /// The url to use for all relative fetches.
    base_url: Url,

    // Client to use for network requests
    client: Option<Rc<HttpClient>>,

    xml_socket_allowed: HashSet<String>,

    xml_sockets_behavior: XmlSocketBehavior,

    upgrade_to_https: bool,

    open_url_mode: OpenURLMode,
}

impl ExternalNavigatorBackend {
    /// Construct a navigator backend with fetch and async capability.
    pub fn new(
        mut base_url: Url,
        channel: Sender<OwnedFuture<(), Error>>,
        event_loop: EventLoopProxy<RuffleEvent>,
        proxy: Option<Url>,
        upgrade_to_https: bool,
        open_url_mode: OpenURLMode,
        xml_socket_allowed: HashSet<String>,
        xml_sockets_behavior: XmlSocketBehavior,
    ) -> Self {
        let proxy = proxy.and_then(|url| url.as_str().parse().ok());
        let builder = HttpClient::builder()
            .proxy(proxy)
            .redirect_policy(RedirectPolicy::Follow);

        let client = builder.build().ok().map(Rc::new);

        // Force replace the last segment with empty. //

        if let Ok(mut base_url) = base_url.path_segments_mut() {
            base_url.pop().pop_if_empty().push("");
        }

        Self {
            channel,
            event_loop,
            client,
            base_url,
            upgrade_to_https,
            open_url_mode,
            xml_socket_allowed,
            xml_sockets_behavior,
        }
    }
}

impl NavigatorBackend for ExternalNavigatorBackend {
    fn navigate_to_url(
        &self,
        url: &str,
        _target: &str,
        vars_method: Option<(NavigationMethod, IndexMap<String, String>)>,
    ) {
        //TODO: Should we return a result for failed opens? Does Flash care?

        //NOTE: Flash desktop players / projectors ignore the window parameter,
        //      unless it's a `_layer`, and we shouldn't handle that anyway.
        let mut parsed_url = match self.base_url.join(url) {
            Ok(parsed_url) => parsed_url,
            Err(e) => {
                tracing::error!(
                    "Could not parse URL because of {}, the corrupt URL was: {}",
                    e,
                    url
                );
                return;
            }
        };

        let modified_url = match vars_method {
            Some((_, query_pairs)) => {
                {
                    //lifetime limiter because we don't have NLL yet
                    let mut modifier = parsed_url.query_pairs_mut();

                    for (k, v) in query_pairs.iter() {
                        modifier.append_pair(k, v);
                    }
                }

                parsed_url
            }
            None => parsed_url,
        };

        let processed_url = self.pre_process_url(modified_url);

        if processed_url.scheme() == "javascript" {
            tracing::warn!(
                "SWF tried to run a script on desktop, but javascript calls are not allowed"
            );
            return;
        }

        if self.open_url_mode == OpenURLMode::Confirm {
            let message = format!("The SWF file wants to open the website {}", processed_url);
            // TODO: Add a checkbox with a GUI toolkit
            let confirm = MessageDialog::new()
                .set_title("Open website?")
                .set_level(MessageLevel::Info)
                .set_description(&message)
                .set_buttons(MessageButtons::OkCancel)
                .show();
            if !confirm {
                tracing::info!("SWF tried to open a website, but the user declined the request");
                return;
            }
        } else if self.open_url_mode == OpenURLMode::Deny {
            tracing::warn!("SWF tried to open a website, but opening a website is not allowed");
            return;
        }

        // If the user confirmed or if in Allow mode, open the website
        match webbrowser::open(processed_url.as_ref()) {
            Ok(_output) => {}
            Err(e) => tracing::error!("Could not open URL {}: {}", processed_url.as_str(), e),
        };
    }

    fn fetch(&self, request: Request) -> OwnedFuture<Response, Error> {
        // TODO: honor sandbox type (local-with-filesystem, local-with-network, remote, ...)
        let full_url = match self.base_url.join(request.url()) {
            Ok(url) => url,
            Err(e) => {
                let msg = format!("Invalid URL {}: {e}", request.url());
                return Box::pin(async move { Err(Error::FetchError(msg)) });
            }
        };

        let processed_url = self.pre_process_url(full_url);

        let client = self.client.clone();

        match processed_url.scheme() {
            "file" => Box::pin(async move {
                let path = processed_url.to_file_path().unwrap_or_default();

                let url = processed_url.into();

                let body = std::fs::read(&path).or_else(|e| {
                    if cfg!(feature = "sandbox") {
                        use rfd::FileDialog;
                        use std::io::ErrorKind;

                        if e.kind() == ErrorKind::PermissionDenied {
                            let attempt_sandbox_open = MessageDialog::new()
                                .set_level(MessageLevel::Warning)
                                .set_description(&format!("The current movie is attempting to read files stored in {}.\n\nTo allow it to do so, click Yes, and then Open to grant read access to that directory.\n\nOtherwise, click No to deny access.", path.parent().unwrap_or(&path).to_string_lossy()))
                                .set_buttons(MessageButtons::YesNo)
                                .show();

                            if attempt_sandbox_open {
                                FileDialog::new().set_directory(&path).pick_folder();

                                return std::fs::read(&path);
                            }
                        }
                    }

                    Err(e)
                }).map_err(|e| Error::FetchError(e.to_string()))?;

                Ok(Response { url, body })
            }),
            _ => Box::pin(async move {
                let client =
                    client.ok_or_else(|| Error::FetchError("Network unavailable".to_string()))?;

                let mut isahc_request = match request.method() {
                    NavigationMethod::Get => IsahcRequest::get(processed_url.to_string()),
                    NavigationMethod::Post => IsahcRequest::post(processed_url.to_string()),
                };
                if let Some(headers) = isahc_request.headers_mut() {
                    for (name, val) in request.headers().iter() {
                        headers.insert(
                            HeaderName::from_str(name)
                                .map_err(|e| Error::FetchError(e.to_string()))?,
                            HeaderValue::from_str(val)
                                .map_err(|e| Error::FetchError(e.to_string()))?,
                        );
                    }
                }

                let (body_data, _) = request.body().clone().unwrap_or_default();
                let body = isahc_request
                    .body(body_data)
                    .map_err(|e| Error::FetchError(e.to_string()))?;

                let mut response = client
                    .send_async(body)
                    .await
                    .map_err(|e| Error::FetchError(e.to_string()))?;

                if !response.status().is_success() {
                    return Err(Error::FetchError(format!(
                        "HTTP status is not ok, got {}",
                        response.status()
                    )));
                }

                let url = if let Some(uri) = response.effective_uri() {
                    uri.to_string()
                } else {
                    processed_url.into()
                };

                let mut body = vec![];
                response
                    .copy_to(&mut body)
                    .await
                    .map_err(|e| Error::FetchError(e.to_string()))?;

                Ok(Response { url, body })
            }),
        }
    }

    fn spawn_future(&mut self, future: OwnedFuture<(), Error>) {
        self.channel.send(future).expect("working channel send");

        if self.event_loop.send_event(RuffleEvent::TaskPoll).is_err() {
            tracing::warn!(
                "A task was queued on an event loop that has already ended. It will not be polled."
            );
        }
    }

    fn pre_process_url(&self, mut url: Url) -> Url {
        if self.upgrade_to_https && url.scheme() == "http" && url.set_scheme("https").is_err() {
            tracing::error!("Url::set_scheme failed on: {}", url);
        }
        url
    }

    fn connect_xml_socket(
        &mut self,
        host: &str,
        port: u16,
    ) -> Option<Box<dyn XmlSocketConnection>> {
        let addr = format!("{}:{}", host, port);
        let is_allowed = self.xml_socket_allowed.contains(&addr);

        match (is_allowed, self.xml_sockets_behavior) {
            (false, XmlSocketBehavior::Unrestricted) | (true, _) => {
                Some(Box::new(TcpXmlSocket::connect(host, port)))
            }
            (false, XmlSocketBehavior::Disabled) => None,
            (false, XmlSocketBehavior::Deny) => Some(Box::new(DenySocket)),
            (false, XmlSocketBehavior::Ask) => {
                let mutex = Arc::new(Mutex::new(None));

                {
                    let host = host.to_string();
                    let mutex: Arc<Mutex<Option<Box<dyn XmlSocketConnection>>>> = mutex.clone();

                    self.spawn_future(Box::pin(async move {
                        use rfd::{MessageButtons, AsyncMessageDialog, MessageLevel};

                        let attempt_sandbox_connect = AsyncMessageDialog::new()
                            .set_level(MessageLevel::Warning)
                            .set_description(&format!("The current movie is attempting to connect to {:?} (port {}).\n\nTo allow it to do so, click Yes to grant network access to that host.\n\nOtherwise, click No to deny access.", host, port))
                            .set_buttons(MessageButtons::YesNo)
                            .show()
                            .await;

                        if let Ok(mut lock) = mutex.try_lock() {
                            if !attempt_sandbox_connect {
                                *lock = Some(Box::new(DenySocket));
                            } else {
                                *lock = Some(Box::new(TcpXmlSocket::connect(host.as_str(), port)));
                            }
                        }

                        Ok(())
                    }));
                }

                Some(Box::new(PendingConnectSocket(mutex)))
            }
        }
    }
}

struct PendingConnectSocket(Arc<Mutex<Option<Box<dyn XmlSocketConnection>>>>);

impl XmlSocketConnection for PendingConnectSocket {
    fn is_connected(&self) -> Option<bool> {
        self.0
            .try_lock()
            .ok()
            .map(|lock| lock.as_ref().and_then(|s| s.is_connected()))
            .unwrap_or_else(|| Some(false))
    }

    fn send(&mut self, buf: Vec<u8>) {
        if let Ok(mut lock) = self.0.try_lock() {
            if let Some(ref mut socket) = *lock {
                socket.send(buf);
            }
        }
    }

    fn poll(&mut self) -> Option<Vec<u8>> {
        if let Ok(mut lock) = self.0.try_lock() {
            if let Some(ref mut socket) = *lock {
                return socket.poll();
            }
        }
        None
    }
}

struct DenySocket;

impl XmlSocketConnection for DenySocket {
    fn is_connected(&self) -> Option<bool> {
        Some(false)
    }

    fn send(&mut self, _buf: Vec<u8>) {}

    fn poll(&mut self) -> Option<Vec<u8>> {
        None
    }
}

struct TcpXmlSocket {
    stream: Option<TcpStream>,
    pending_write: Vec<u8>,
    pending_read: VecDeque<u8>,
}

impl TcpXmlSocket {
    fn connect(host: &str, port: u16) -> Self {
        // FIXME: make connect asynchronous
        Self {
            stream: TcpStream::connect((host, port)).ok().and_then(|socket| {
                if socket.set_nonblocking(true).is_ok() {
                    Some(socket)
                } else {
                    None
                }
            }),
            pending_write: Default::default(),
            pending_read: Default::default(),
        }
    }
}

impl XmlSocketConnection for TcpXmlSocket {
    fn is_connected(&self) -> Option<bool> {
        Some(self.stream.is_some())
    }

    fn send(&mut self, buf: Vec<u8>) {
        if self.stream.is_some() {
            self.pending_write.extend(buf);
            self.pending_write.push(0);
        }
    }

    fn poll(&mut self) -> Option<Vec<u8>> {
        if let Some(stream) = &mut self.stream {
            if !self.pending_write.is_empty() {
                match stream.write(&self.pending_write) {
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {} // just try later
                    Err(_) | Ok(0) => {
                        self.stream = None;
                        return None;
                    }
                    Ok(written) => {
                        let _ = self.pending_write.drain(..written);
                    }
                }
            }

            match process_next_message(&mut self.pending_read) {
                Some(msg) => Some(msg),
                None => {
                    let mut buffer = [0; 2048];

                    match stream.read(&mut buffer) {
                        Err(e) if e.kind() == ErrorKind::WouldBlock => None, // just try later
                        Err(_) | Ok(0) => {
                            self.stream = None;
                            None
                        }
                        Ok(read) => {
                            self.pending_read.extend(buffer.into_iter().take(read));
                            process_next_message(&mut self.pending_read)
                        }
                    }
                }
            }
        } else {
            None
        }
    }
}

fn process_next_message(pending_read: &mut VecDeque<u8>) -> Option<Vec<u8>> {
    if let Some((index, _)) = pending_read.iter().enumerate().find(|(_, &b)| b == 0) {
        let buffer = pending_read.drain(..index).collect::<Vec<_>>();
        let _ = pending_read.pop_front(); // remove the separator
        Some(buffer)
    } else {
        None
    }
}
