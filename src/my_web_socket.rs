use std::net::SocketAddr;

use futures::{stream::SplitSink, SinkExt};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{
    tungstenite::{Error, Message},
    WebSocketStream,
};
use my_http_server::{UrlEncodedData, UrlEncodedDataSource};
use tokio::sync::Mutex;

pub struct MyWebSocket {
    pub write_stream: Mutex<SplitSink<WebSocketStream<Upgraded>, Message>>,
    pub addr: SocketAddr,
    pub id: i64,
    query_string: Option<String>,
}

impl MyWebSocket {
    pub fn new(
        id: i64,
        write_stream: SplitSink<WebSocketStream<Upgraded>, Message>,
        addr: SocketAddr,
        query_string: Option<String>,
    ) -> Self {
        Self {
            write_stream: Mutex::new(write_stream),
            addr,
            id,
            query_string,
        }
    }

    pub async fn send_message(&self, msg: Message) -> Result<(), Error> {
        let mut write_access = self.write_stream.lock().await;
        write_access.send(msg).await
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
}
