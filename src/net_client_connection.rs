use anyhow::{Context, Result};
#[allow(unused_imports)]
use log::{error, info};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{channel, Receiver, Sender};
#[cfg(not(target_arch = "wasm32"))]
use ws;

#[cfg(target_arch = "wasm32")]
const SOCKET_CONNECTING: u32 = 0;
#[cfg(target_arch = "wasm32")]
const SOCKET_OPEN: u32 = 1;
#[cfg(target_arch = "wasm32")]
const SOCKET_CLOSING: u32 = 2;
#[cfg(target_arch = "wasm32")]
const SOCKET_CLOSED: u32 = 3;

// implemented in roper.js
#[cfg(target_arch = "wasm32")]
extern "C" {
    fn socket_connect(url: *const u8, url_len: u32) -> bool;
    // pops error message, otherwise return 0
    fn socket_get_error(buf: *mut u8, buf_len: u32) -> u32;

    // -1 for missing message
    fn socket_get_message_len() -> i32;
    fn socket_copy_message(dest_ptr: *mut u8, expected_len: u32) -> bool;

    fn socket_send_binary(data: *const u8, len: u32) -> bool;
    #[allow(dead_code)]
    fn socket_send_string(data: *const u8, len: u32) -> bool;

    fn socket_close();

    fn socket_ready_state() -> u32;
}

#[derive(Debug)]
pub enum ConnectionEvent {
    Received(Vec<u8>),
    #[cfg(target_arch = "wasm32")]
    Connected(()),
    #[cfg(not(target_arch = "wasm32"))]
    Connected(ws::Sender),
    FailedToConnect(String),
    Disconnected(String),
}

#[derive(Debug, Copy, Clone)]
pub enum ConnectionState {
    Offline,
    Connecting,
    Connected,
}

pub struct ClientConnection {
    pub state: ConnectionState,
    #[cfg(not(target_arch = "wasm32"))]
    thread: Option<std::thread::JoinHandle<()>>,
    #[cfg(not(target_arch = "wasm32"))]
    event_rx: Option<Receiver<ConnectionEvent>>,
    #[cfg(not(target_arch = "wasm32"))]
    socket_tx: Option<ws::Sender>,
}

#[cfg(not(target_arch = "wasm32"))]
struct WebsocketClient {
    socket_tx: ws::Sender,
    event_tx: Sender<ConnectionEvent>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ws::Handler for WebsocketClient {
    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        self.event_tx
            .send(ConnectionEvent::Connected(self.socket_tx.clone()))
            .map_err(|err| {
                ws::Error::new(
                    ws::ErrorKind::Internal,
                    format!("Unable to communicate between threads: {:?}.", err),
                )
            })
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        match msg {
            ws::Message::Binary(blob) => {
                self.event_tx.send(ConnectionEvent::Received(blob)).unwrap();
            }
            ws::Message::Text(text) => {
                info!("unexpected text message: {}", text);
            }
        }
        Ok(())
    }

    fn on_close(&mut self, _code: ws::CloseCode, reason: &str) {
        self.event_tx
            .send(ConnectionEvent::Disconnected(reason.to_string()))
            .unwrap();
    }

    fn on_error(&mut self, err: ws::Error) {
        self.event_tx
            .send(ConnectionEvent::Disconnected(err.to_string()))
            .unwrap();
    }
}

impl ClientConnection {
    pub fn new() -> Self {
        ClientConnection {
            state: ConnectionState::Offline,
            #[cfg(not(target_arch = "wasm32"))]
            thread: None,
            #[cfg(not(target_arch = "wasm32"))]
            socket_tx: None,
            #[cfg(not(target_arch = "wasm32"))]
            event_rx: None,
        }
    }

    pub fn connect(&mut self, addr: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.state = ConnectionState::Connecting;

            let (event_tx, event_rx) = channel::<ConnectionEvent>();

            let addr = addr.to_owned();
            self.thread = Some(std::thread::spawn(move || {
                let result: Result<()> = (|| {
                    let mut ws = ws::Builder::new()
                        .with_settings(ws::Settings {
                            tcp_nodelay: true,
                            ..ws::Settings::default()
                        })
                        .build(|socket_sender| {
                            let event_tx = event_tx.clone();
                            WebsocketClient {
                                socket_tx: socket_sender,
                                event_tx,
                            }
                        })?;

                    ws.connect(url::Url::parse(&addr).context("Parsing WS url")?)
                        .context("Connecting to WebSocket")?;
                    ws.run().context("Polling WebSocket")?;
                    Ok(())
                })();
                if let Err(e) = result {
                    event_tx
                        .clone()
                        .send(ConnectionEvent::FailedToConnect(e.to_string()))
                        .unwrap();
                }
            }));
            self.event_rx = Some(event_rx);
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.state = ConnectionState::Connecting;

            if !unsafe { socket_connect(addr.as_ptr(), addr.len() as _) } {
                error!("failed to connect to {}", addr);
                self.state = ConnectionState::Offline;
            }
        }
    }

    pub fn disconnect(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(tx) = self.socket_tx.take() {
                tx.close(ws::CloseCode::Normal).unwrap();
            }
            if let Some(t) = self.thread.take() {
                t.join().unwrap();
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            unsafe { socket_close() };
        }
    }

    pub fn send(&mut self, message: Vec<u8>) {
        #[cfg(not(target_arch = "wasm32"))]
        match self.socket_tx.as_mut() {
            Some(socket) => {
                let _ = socket
                    .send(ws::Message::Binary(message))
                    .map_err(|e| error!("socket send failed: {}", e));
            }
            None => {
                error!("sending while disconnected");
            }
        }
        #[cfg(target_arch = "wasm32")]
        unsafe {
            socket_send_binary(message.as_ptr(), message.len() as _)
        };
    }

    pub fn poll(&mut self) -> Option<ConnectionEvent> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(event_rx) = self.event_rx.as_mut() {
                match event_rx.try_recv().ok() {
                    Some(reply) => {
                        match &reply {
                            ConnectionEvent::Connected(tx) => {
                                self.socket_tx = Some(tx.clone());
                                self.state = ConnectionState::Connected;
                            }
                            ConnectionEvent::Disconnected(_) => {
                                self.socket_tx = None;
                                self.state = ConnectionState::Offline;
                            }
                            ConnectionEvent::FailedToConnect(_) => {
                                self.socket_tx = None;
                                self.state = ConnectionState::Offline;
                            }
                            ConnectionEvent::Received(_) => {}
                        }
                        Some(reply)
                    }
                    None => None,
                }
            } else {
                None
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let result = match self.state.borrow() {
                ConnectionState::Connecting => match unsafe { socket_ready_state() } {
                    SOCKET_OPEN => {
                        self.state = ConnectionState::Connected;
                        Some(ConnectionEvent::Connected(()))
                    }
                    SOCKET_CLOSED | SOCKET_CLOSING => None,
                    SOCKET_CONNECTING => None,
                    _ => None,
                },
                _ => {
                    // handle below
                    None
                }
            };
            if result.is_some() {
                return result;
            }

            let message_len = unsafe { socket_get_message_len() };
            if message_len >= 0 {
                let mut message = Vec::<u8>::with_capacity(message_len as _);
                message.resize(message_len as usize, 0);
                if !unsafe { socket_copy_message(message.as_mut_ptr(), message.len() as _) } {
                    error!("failed to receive mesage of length {}", message_len);
                }
                return Some(ConnectionEvent::Received(message));
            }

            match self.state {
                ConnectionState::Connecting | ConnectionState::Connected => {
                    match unsafe { socket_ready_state() } {
                        SOCKET_OPEN => None,
                        SOCKET_CONNECTING => None,
                        SOCKET_CLOSED | SOCKET_CLOSING => {
                            let mut reason = "Connection closed.".to_owned();
                            loop {
                                let mut err_buf = [0u8; 512];
                                let err_len = unsafe {
                                    socket_get_error(err_buf.as_mut_ptr(), err_buf.len() as _)
                                };
                                if err_len == 0 {
                                    break;
                                }
                                reason = String::from_utf8_lossy(&err_buf[0..err_len as usize])
                                    .to_string();
                            }
                            if matches!(self.state, ConnectionState::Connecting) {
                                self.state = ConnectionState::Offline;
                                Some(ConnectionEvent::FailedToConnect(reason))
                            } else {
                                self.state = ConnectionState::Offline;
                                Some(ConnectionEvent::Disconnected(reason))
                            }
                        }
                        state @ _ => {
                            error!("Unexpected ready state: {}", state);
                            None
                        }
                    }
                }
                ConnectionState::Offline => None,
            }
        }
    }
}

impl Drop for ClientConnection {
    fn drop(&mut self) {
        self.disconnect();
    }
}
