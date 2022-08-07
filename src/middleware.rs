use std::sync::Arc;

use futures::{stream::SplitStream, StreamExt};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{
    tungstenite::{Error, Message},
    WebSocketStream,
};
use my_http_server::{
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput, HttpServerMiddleware,
    HttpServerRequestFlow, RequestData, WebContentType,
};
use tokio::sync::Mutex;

use crate::{MyWebSockeCallback, MyWebSocket, WebSocketMessage};

pub struct MyWebSocketsMiddleware {
    path: String,
    callback: Arc<dyn MyWebSockeCallback + Send + Sync + 'static>,
    socket_id: Mutex<i64>,
}

impl MyWebSocketsMiddleware {
    pub fn new(path: &str, callback: Arc<dyn MyWebSockeCallback + Send + Sync + 'static>) -> Self {
        Self {
            path: path.to_string(),
            callback,
            socket_id: Mutex::new(0),
        }
    }

    async fn get_socket_id(&self) -> i64 {
        let mut socket_no = self.socket_id.lock().await;
        *socket_no += 1;
        *socket_no
    }

    async fn handle_web_socket_path(
        &self,
        ctx: &mut HttpContext,
    ) -> Result<HttpOkResult, HttpFailResult> {
        if let RequestData::AsRaw(req) = &mut ctx.request.req {
            let query_string = if let Some(query_string) = req.uri().query() {
                Some(query_string.to_string())
            } else {
                None
            };

            let upgrade_result = hyper_tungstenite::upgrade(req, None);

            println!("Upgrade result: {:?}", upgrade_result);

            if let Err(err) = upgrade_result {
                let content = format!("Can not upgrade websocket. Reason: {}", err);
                println!("{}", content);
                return Err(HttpFailResult {
                    content_type: WebContentType::Text,
                    status_code: 400,
                    content: content.into_bytes(),
                    write_telemetry: false,
                });
            }

            let (response, web_socket) = upgrade_result.unwrap();

            let callback = self.callback.clone();
            let id = self.get_socket_id().await;
            let addr = ctx.request.addr;

            tokio::spawn(async move {
                let web_socket = web_socket.await.unwrap();

                let (write, read_stream) = web_socket.split();

                let my_web_socket = MyWebSocket::new(id, write, addr, query_string);
                let my_web_socket = Arc::new(my_web_socket);

                callback.connected(my_web_socket.clone()).await.unwrap();

                let serve_socket_result = tokio::spawn(serve_websocket(
                    my_web_socket.clone(),
                    read_stream,
                    callback.clone(),
                ))
                .await;

                callback.disconnected(my_web_socket.clone()).await;

                if let Err(err) = serve_socket_result {
                    println!(
                        "Execution of websocket {} is finished with panic. {}",
                        id, err
                    );
                }
            });

            return Ok(HttpOkResult {
                write_telemetry: false,
                output: HttpOutput::Raw(response),
            });
        }

        return Err(HttpFailResult {
            content_type: WebContentType::Text,
            status_code: 400,
            content: "Request can not be used to upgrade to websocket. This middleware has to be first in the line".to_string().into_bytes(),
            write_telemetry: false,
        });
    }
}

#[async_trait::async_trait]
impl HttpServerMiddleware for MyWebSocketsMiddleware {
    async fn handle_request(
        &self,
        ctx: &mut HttpContext,
        get_next: &mut HttpServerRequestFlow,
    ) -> Result<HttpOkResult, HttpFailResult> {
        if let Some(_) = ctx.request.get_optional_header("sec-websocket-key") {
            if ctx.request.get_path_lower_case() == self.path {
                return self.handle_web_socket_path(ctx).await;
            }
        }

        get_next.next(ctx).await
    }
}

/// Handle a websocket connection.
async fn serve_websocket(
    my_web_socket: Arc<MyWebSocket>,
    mut read_stream: SplitStream<WebSocketStream<Upgraded>>,
    callback: Arc<dyn MyWebSockeCallback + Send + Sync + 'static>,
) -> Result<(), Error> {
    while let Some(message) = read_stream.next().await {
        let result = match message? {
            Message::Text(msg) => {
                send_message(
                    my_web_socket.clone(),
                    WebSocketMessage::String(msg),
                    callback.clone(),
                )
                .await
            }
            Message::Binary(msg) => {
                send_message(
                    my_web_socket.clone(),
                    WebSocketMessage::Binary(msg),
                    callback.clone(),
                )
                .await
            }
            Message::Ping(_) => Ok(()),
            Message::Pong(_) => Ok(()),
            Message::Close(_) => Ok(()),
            Message::Frame(_) => Ok(()),
        };

        if let Err(err) = result {
            eprintln!("Error in websocket connection: {}", err);
            break;
        }
    }

    Ok(())
}

async fn send_message(
    web_socket: Arc<MyWebSocket>,
    message: WebSocketMessage,
    callback: Arc<dyn MyWebSockeCallback + Send + Sync + 'static>,
) -> Result<(), String> {
    let result = tokio::spawn(async move {
        callback.on_message(web_socket, message).await;
    })
    .await;

    if let Err(err) = result {
        return Err(format!("Error in on_message: {}", err));
    }

    Ok(())
}
