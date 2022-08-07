use std::{net::SocketAddr, sync::atomic::AtomicBool};

use futures::{stream::SplitSink, SinkExt};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{
    tungstenite::{Error, Message},
    WebSocketStream,
};
use my_http_server::{UrlEncodedData, UrlEncodedDataSource};
use tokio::sync::Mutex;

pub struct MyWebSocket {
    pub write_stream: Mutex<Option<SplitSink<WebSocketStream<Upgraded>, Message>>>,
    pub addr: SocketAddr,
    pub id: i64,
    query_string: Option<String>,
    connected: AtomicBool,
}

impl MyWebSocket {
    pub fn new(
        id: i64,
        write_stream: SplitSink<WebSocketStream<Upgraded>, Message>,
        addr: SocketAddr,
        query_string: Option<String>,
    ) -> Self {
        Self {
            write_stream: Mutex::new(Some(write_stream)),
            addr,
            id,
            query_string,
            connected: AtomicBool::new(true),
        }
    }

    async fn send_message_and_if_conntext(&self, msg: Message) -> Result<(), Error> {
        let mut write_access = self.write_stream.lock().await;
        if let Some(stream) = &mut *write_access {
            return stream.send(msg).await;
        }

        Ok(())
    }

    pub async fn send_message(&self, msg: Message) {
        let result = self.send_message_and_if_conntext(msg).await;

        if let Err(err) = result {
            println!("Error sending message to websocket {}: {:?}", self.id, err);
            self.disconnect().await;
        }
    }

    pub fn get_query_string<'s>(&'s self) -> Option<UrlEncodedData<'s>> {
        let str = self.query_string.as_ref()?;

        match UrlEncodedData::new(str, UrlEncodedDataSource::QueryString) {
            Ok(result) => Some(result),
            Err(_) => {
                println!("Can not parse query string: {}", str);
                return None;
            }
        }
    }

    pub async fn disconnect(&self) {
        self.connected
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let mut write_access = self.write_stream.lock().await;
        if let Some(mut item) = write_access.take() {
            let result = item.close().await;

            if let Err(err) = result {
                println!("Can not close websocket {}. Reason: {:?}", self.id, err);
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }
}
