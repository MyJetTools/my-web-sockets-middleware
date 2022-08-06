use std::net::SocketAddr;

use futures::{stream::SplitSink, SinkExt};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{
    tungstenite::{Error, Message},
    WebSocketStream,
};
use tokio::sync::Mutex;

pub struct MyWebSocket {
    pub write_stream: Mutex<SplitSink<WebSocketStream<Upgraded>, Message>>,
    pub addr: SocketAddr,
}

impl MyWebSocket {
    pub fn new(
        write_stream: SplitSink<WebSocketStream<Upgraded>, Message>,
        addr: SocketAddr,
    ) -> Self {
        Self {
            write_stream: Mutex::new(write_stream),
            addr,
        }
    }

    pub async fn send_message(&self, msg: Message) -> Result<(), Error> {
        let mut write_access = self.write_stream.lock().await;
        write_access.send(msg).await
    }
}
