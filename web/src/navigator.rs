//! Navigator backend for web
use async_channel::Receiver;
use js_sys::{Array, ArrayBuffer, Uint8Array, Promise, Function};
use ruffle_core::backend::navigator::{
    async_return, create_fetch_error, create_specific_fetch_error, ErrorResponse, NavigationMethod,
    NavigatorBackend, OpenURLMode, OwnedFuture, Request, SuccessResponse,
};
use ruffle_core::config::NetworkingAccessMode;
use ruffle_core::indexmap::IndexMap;
use ruffle_core::loader::Error;
use ruffle_core::socket::{ConnectionState, SocketAction, SocketHandle};
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::layer::Layered;
use tracing_subscriber::Registry;
use tracing_wasm::WASMLayer;
use url::{ParseError, Url};
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    window, Blob, BlobPropertyBag, HtmlFormElement, HtmlInputElement, Request as WebRequest,
    RequestInit, Response as WebResponse,
};

pub struct WebNavigatorBackend {
    log_subscriber: Arc<Layered<WASMLayer, Registry>>,
    allow_script_access: bool,
    allow_networking: NetworkingAccessMode,
    upgrade_to_https: bool,
    base_url: Option<Url>,
    open_url_mode: OpenURLMode,
    socket_callback: Function,
}

impl WebNavigatorBackend {
    pub fn new(
        allow_script_access: bool,
        allow_networking: NetworkingAccessMode,
        upgrade_to_https: bool,
        base_url: Option<String>,
        log_subscriber: Arc<Layered<WASMLayer, Registry>>,
        open_url_mode: OpenURLMode,
        socket_callback: Function,
    ) -> Self {
        let window = web_sys::window().expect("window()");

        // Upgrade to HTTPS takes effect if the current page is hosted on HTTPS.
        let upgrade_to_https =
            upgrade_to_https && window.location().protocol().expect("protocol()") == "https:";

        // Retrieve and parse `document.baseURI`.
        let document_base_uri = || {
            let document = window.document().expect("document()");
            if let Ok(Some(base_uri)) = document.base_uri() {
                return Url::parse(&base_uri).ok();
            }

            None
        };

        let base_url = if let Some(mut base_url) = base_url {
            // Adding trailing slash so `Url::parse` will not drop the last part.
            if !base_url.ends_with('/') {
                base_url.push('/');
            }

            Url::parse(&base_url)
                .ok()
                .or_else(|| document_base_uri().and_then(|base_uri| base_uri.join(&base_url).ok()))
        } else {
            document_base_uri()
        };

        if base_url.is_none() {
            tracing::error!("Could not get base URL for base directory inference.");
        }

        Self {
            allow_script_access,
            allow_networking,
            upgrade_to_https,
            base_url,
            log_subscriber,
            open_url_mode,
            socket_callback
        }
    }
}

impl NavigatorBackend for WebNavigatorBackend {
    fn navigate_to_url(
        &self,
        url: &str,
        target: &str,
        vars_method: Option<(NavigationMethod, IndexMap<String, String>)>,
    ) {
        // If the URL is empty, ignore the request.
        if url.is_empty() {
            return;
        }

        let url = match self.resolve_url(url) {
            Ok(url) => {
                if url.scheme() == "file" {
                    tracing::error!(
                        "Can't open the local URL {} on WASM target",
                        url.to_string()
                    );
                    return;
                } else {
                    url
                }
            }
            Err(e) => {
                tracing::error!(
                    "Could not parse URL because of {}, the corrupt URL was: {}",
                    e,
                    url
                );
                return;
            }
        };

        // If `allowNetworking` is set to `internal` or `none`, block all `navigate_to_url` calls.
        if self.allow_networking != NetworkingAccessMode::All {
            tracing::warn!("SWF tried to open a URL, but opening URLs is not allowed");
            return;
        }

        // If `allowScriptAccess` is disabled, reject the `javascript:` scheme.
        // Also reject any attempt to open a URL when `target` is a keyword that affects the current tab.
        if !self.allow_script_access {
            if url.scheme() == "javascript" {
                tracing::warn!("SWF tried to run a script, but script access is not allowed");
                return;
            } else {
                match target.to_lowercase().as_str() {
                    "_parent" | "_self" | "_top" | "" => {
                        tracing::warn!("SWF tried to open a URL, but opening URLs in the current tab is prevented by script access");
                        return;
                    }
                    _ => (),
                }
            }
        }

        let window = window().expect("window()");

        if url.scheme() != "javascript" {
            if self.open_url_mode == OpenURLMode::Confirm {
                let message = format!("The SWF file wants to open the website {}", &url);
                // TODO: Add a checkbox with a GUI toolkit
                let confirm = window
                    .confirm_with_message(&message)
                    .expect("confirm_with_message()");
                if !confirm {
                    tracing::info!(
                        "SWF tried to open a website, but the user declined the request"
                    );
                    return;
                }
            } else if self.open_url_mode == OpenURLMode::Deny {
                tracing::warn!("SWF tried to open a website, but opening a website is not allowed");
                return;
            }
            // If the user confirmed or if in `Allow` mode, open the website.
        }

        // TODO: Should we return a result for failed opens? Does Flash care?
        match vars_method {
            Some((navmethod, formvars)) => {
                let document = window.document().expect("document()");
                let body = match document.body() {
                    Some(body) => body,
                    None => return,
                };

                let form: HtmlFormElement = document
                    .create_element("form")
                    .expect("create_element() must succeed")
                    .dyn_into()
                    .expect("create_element(\"form\") didn't give us a form");

                form.set_method(&navmethod.to_string());
                form.set_action(url.as_str());

                if !target.is_empty() {
                    form.set_target(target);
                }

                for (key, value) in formvars {
                    let hidden: HtmlInputElement = document
                        .create_element("input")
                        .expect("create_element() must succeed")
                        .dyn_into()
                        .expect("create_element(\"input\") didn't give us an input");

                    hidden.set_type("hidden");
                    hidden.set_name(&key);
                    hidden.set_value(&value);

                    let _ = form.append_child(&hidden);
                }

                let _ = body.append_child(&form);
                let _ = form.submit();
            }
            None => {
                if target.is_empty() {
                    let _ = window.location().assign(url.as_str());
                } else {
                    let _ = window.open_with_url_and_target(url.as_str(), target);
                }
            }
        };
    }

    fn fetch(&self, request: Request) -> OwnedFuture<SuccessResponse, ErrorResponse> {
        let url = match self.resolve_url(request.url()) {
            Ok(url) => {
                if url.scheme() == "file" {
                    return async_return(create_specific_fetch_error(
                        "WASM target can't fetch local URL",
                        url.as_str(),
                        "",
                    ));
                } else {
                    url
                }
            }
            Err(e) => {
                return async_return(create_fetch_error(request.url(), e));
            }
        };

        Box::pin(async move {
            let mut init = RequestInit::new();

            init.method(&request.method().to_string());

            if let Some((data, mime)) = request.body() {
                let blob = Blob::new_with_buffer_source_sequence_and_options(
                    &Array::from_iter([Uint8Array::from(data.as_slice()).buffer()]),
                    BlobPropertyBag::new().type_(mime),
                )
                .map_err(|_| ErrorResponse {
                    url: url.to_string(),
                    error: Error::FetchError("Got JS error".to_string()),
                })?
                .dyn_into()
                .map_err(|_| ErrorResponse {
                    url: url.to_string(),
                    error: Error::FetchError("Got JS error".to_string()),
                })?;

                init.body(Some(&blob));
            }

            let web_request = match WebRequest::new_with_str_and_init(url.as_str(), &init) {
                Ok(web_request) => web_request,
                Err(_) => {
                    return create_specific_fetch_error(
                        "Unable to create request for",
                        url.as_str(),
                        "",
                    )
                }
            };

            let headers = web_request.headers();

            for (header_name, header_val) in request.headers() {
                headers
                    .set(header_name, header_val)
                    .map_err(|_| ErrorResponse {
                        url: url.to_string(),
                        error: Error::FetchError("Got JS error".to_string()),
                    })?;
            }

            let window = web_sys::window().expect("window()");
            let fetchval = JsFuture::from(window.fetch_with_request(&web_request))
                .await
                .map_err(|_| ErrorResponse {
                    url: url.to_string(),
                    error: Error::FetchError("Got JS error".to_string()),
                })?;

            let response: WebResponse = fetchval.dyn_into().map_err(|_| ErrorResponse {
                url: url.to_string(),
                error: Error::FetchError("Fetch result wasn't a WebResponse".to_string()),
            })?;
            let url = response.url();
            let status = response.status();
            let redirected = response.redirected();
            if !response.ok() {
                let error = Error::HttpNotOk(
                    format!("HTTP status is not ok, got {}", response.status_text()),
                    status,
                    redirected,
                );
                return Err(ErrorResponse { url, error });
            }

            let body: ArrayBuffer = JsFuture::from(response.array_buffer().map_err(|_| {
                ErrorResponse {
                    url: url.clone(),
                    error: Error::FetchError("Got JS error".to_string()),
                }
            })?)
            .await
            .map_err(|_| ErrorResponse {
                url: url.clone(),
                error: Error::FetchError(
                    "Could not allocate array buffer for response".to_string(),
                ),
            })?
            .dyn_into()
            .map_err(|_| ErrorResponse {
                url: url.clone(),
                error: Error::FetchError("array_buffer result wasn't an ArrayBuffer".to_string()),
            })?;
            let body = Uint8Array::new(&body).to_vec();

            Ok(SuccessResponse {
                url,
                body,
                status,
                redirected,
            })
        })
    }

    fn resolve_url(&self, url: &str) -> Result<Url, ParseError> {
        if let Some(base_url) = &self.base_url {
            match base_url.join(url) {
                Ok(full_url) => Ok(self.pre_process_url(full_url)),
                Err(error) => Err(error),
            }
        } else {
            match Url::parse(url) {
                Ok(parsed_url) => Ok(self.pre_process_url(parsed_url)),
                Err(error) => Err(error),
            }
        }
    }

    fn spawn_future(&mut self, future: OwnedFuture<(), Error>) {
        let subscriber = self.log_subscriber.clone();
        spawn_local(async move {
            let _subscriber = tracing::subscriber::set_default(subscriber);
            if let Err(e) = future.await {
                tracing::error!("Asynchronous error occurred: {}", e);
            }
        })
    }

    fn pre_process_url(&self, mut url: Url) -> Url {
        if self.upgrade_to_https && url.scheme() == "http" && url.set_scheme("https").is_err() {
            tracing::error!("Url::set_scheme failed on: {}", url);
        }
        url
    }

    fn connect_socket(
        &mut self,
        host: String,
        port: u16,
        _timeout: Duration,
        handle: SocketHandle,
        receiver: Receiver<Vec<u8>>,
        sender: Sender<SocketAction>,
    ) {
        let out_stream = ReadableStream::new(WrappedReceiver { inner: Rc::new(receiver) }, QueuingStrategy { high_water_mark: 0.0 });
        let in_stream = WritableStream::new(WrappedSender { inner: sender.clone(), handle }, QueuingStrategy { high_water_mark: 1.0 });

        let options = SocketConnectOptions {
            host,
            port,
            readable: out_stream.into(),
            writable: in_stream.into(),
        };
        let options = match serde_wasm_bindgen::to_value(&options) {
            Ok(x) => x,
            Err(e) => {
                sender.send(SocketAction::Connect(handle, ConnectionState::Failed)).expect("working channel send");
                tracing::error!("Failed to serialize SocketConnectOptions: {}", e);
                return;
            }
        };

        let promise = match self.socket_callback.call1(&JsValue::null(), &options) {
            Ok(x) => x,
            Err(e) => {
                sender.send(SocketAction::Connect(handle, ConnectionState::Failed)).expect("working channel send");
                tracing::warn!("Failed to call socket callback: {:?}", e);
                return;
            }
        };
        let promise = match promise.dyn_into::<Promise>() {
            Ok(x) => x,
            Err(_) => {
                sender.send(SocketAction::Connect(handle, ConnectionState::Failed)).expect("working channel send");
                tracing::warn!("Socket callback did not return a Promise");
                return;
            }
        };

        self.spawn_future(Box::pin(async move {
            let res = wasm_bindgen_futures::JsFuture::from(promise).await;
            let res = match res {
                Ok(x) => x,
                Err(e) => {
                    tracing::warn!("Socket callback promise failed {:?}", e);
                    sender.send(SocketAction::Connect(handle, ConnectionState::Failed)).expect("working channel send");
                    return Ok(());
                }
            };

            let success = res.as_bool().unwrap_or(false);
            if success {
                sender.send(SocketAction::Connect(handle, ConnectionState::Connected)).expect("working channel send");
            } else {
                sender.send(SocketAction::Connect(handle, ConnectionState::Failed)).expect("working channel send");
            }

            Ok(())
        }));
    }
}

#[derive(serde::Serialize)]
struct SocketConnectOptions {
    pub host: String,
    pub port: u16,

    #[serde(with = "serde_wasm_bindgen::preserve")]
    pub readable: JsValue,
    #[serde(with = "serde_wasm_bindgen::preserve")]
    pub writable: JsValue,
}

#[wasm_bindgen]
#[derive(Debug)]
pub struct QueuingStrategy {
    high_water_mark: f64,
}

#[wasm_bindgen]
impl QueuingStrategy {
    #[wasm_bindgen(getter, js_name = highWaterMark)]
    pub fn high_water_mark(&self) -> f64 {
        self.high_water_mark
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    pub type ReadableStreamDefaultController;

    #[wasm_bindgen(method, js_name = enqueue)]
    pub fn enqueue(this: &ReadableStreamDefaultController, chunk: &JsValue);

    #[wasm_bindgen(method, js_name = close)]
    pub fn close(this: &ReadableStreamDefaultController);
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = ReadableStream, typescript_type = "ReadableStream")]
    #[derive(Clone, Debug)]
    pub type ReadableStream;

    #[wasm_bindgen(constructor)]
    pub fn new(receiver: WrappedReceiver, strategy: QueuingStrategy) -> ReadableStream;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = WritableStream, typescript_type = "WritableStream")]
    #[derive(Clone, Debug)]
    pub type WritableStream;

    #[wasm_bindgen(constructor)]
    pub fn new(sender: WrappedSender, strategy: QueuingStrategy) -> WritableStream;
}

#[wasm_bindgen]
pub struct WrappedReceiver {
    inner: Rc<Receiver<Vec<u8>>>,
}

#[wasm_bindgen]
impl WrappedReceiver {
    pub fn pull(&mut self, controller: ReadableStreamDefaultController) -> Promise {
        let inner = self.inner.clone();

        wasm_bindgen_futures::future_to_promise(async move {
            match inner.recv().await {
                Ok(v) => {
                    let buffer = Uint8Array::new_with_length(v.len() as u32);
                    buffer.copy_from(&v);
                    controller.enqueue(&buffer.into());
                }
                Err(_) => { 
                    controller.close();
                },
            };

            Ok(JsValue::undefined())
        })
    }
}

#[wasm_bindgen]
pub struct WrappedSender {
    inner: Sender<SocketAction>,
    handle: SocketHandle,
}

#[wasm_bindgen]
impl WrappedSender {
    pub fn write(&mut self, chunk: JsValue) {
        if let Some(array) = chunk.dyn_ref::<Uint8Array>() {
            tracing::error!("Received data");
            self.inner.send(SocketAction::Data(self.handle, array.to_vec())).expect("working channel send");
        } else {
            tracing::warn!("Socket WritableStream was given a non-Uint8Array value: {:?}", chunk);
        }
    }

    pub fn close(self) {
        self.inner.send(SocketAction::Close(self.handle)).expect("working channel send");
    }

    pub fn abort(self, _reason: JsValue) {
        self.inner.send(SocketAction::Close(self.handle)).expect("working channel send");
    }
}