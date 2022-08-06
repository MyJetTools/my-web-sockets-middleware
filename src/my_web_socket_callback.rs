use std::sync::Arc;

use my_http_server::HttpFailResult;

use super::MyWebSocket;

#[derive(Debug)]
pub enum WebSocketMessage {
    String(String),
    Binary(Vec<u8>),
}

#[async_trait::async_trait]
pub trait MyWebSockeCallback {
    async fn connected(&self, my_web_socket: Arc<MyWebSocket>) -> Result<(), HttpFailResult>;
    async fn disconnected(&self, my_web_socket: Arc<MyWebSocket>);
    async fn on_message(&self, my_web_socket: Arc<MyWebSocket>, message: WebSocketMessage);
}
