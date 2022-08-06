use std::{net::SocketAddr, sync::Arc};

use futures::StreamExt;
use hyper_tungstenite::{
    tungstenite::{Error, Message},
    HyperWebsocket,
};
use my_http_server::{
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput, HttpServerMiddleware,
    HttpServerRequestFlow, RequestData, WebContentType,
};

use crate::{MyWebSockeCallback, MyWebSocket, WebSocketMessage};

pub struct MyWebSocketsMiddleware {
    path: String,
    callback: Arc<dyn MyWebSockeCallback + Send + Sync + 'static>,
}

impl MyWebSocketsMiddleware {
    pub fn new(path: &str, callback: Arc<dyn MyWebSockeCallback + Send + Sync + 'static>) -> Self {
        Self {
            path: path.to_string(),
            callback,
        }
    }

    fn handle_web_socket_path(
        &self,
        ctx: &mut HttpContext,
    ) -> Result<HttpOkResult, HttpFailResult> {
        if let RequestData::AsRaw(req) = &mut ctx.request.req {
            match hyper_tungstenite::upgrade(req, None) {
                Ok((response, websocket)) => {
                    let addr = ctx.request.addr;
                    let callback = self.callback.clone();
                    tokio::spawn(async move {
                        if let Err(e) = serve_websocket(websocket, callback, addr).await {
                            eprintln!("Error in websocket connection: {}", e);
                        }
                    });

                    return Ok(HttpOkResult {
                        write_telemetry: false,
                        output: HttpOutput::Raw(response),
                    });
                }
                Err(err) => {
                    return Err(HttpFailResult {
                        content_type: WebContentType::Text,
                        status_code: 400,
                        content: format!("{}", err).into_bytes(),
                        write_telemetry: false,
                    });
                }
            }
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
        if ctx
            .request
            .get_optional_header("Sec-WebSocket-Version")
            .is_none()
        {
            return get_next.next(ctx).await;
        }

        if ctx.request.get_path_lower_case() == self.path {
            return self.handle_web_socket_path(ctx);
        }

        get_next.next(ctx).await
    }
}

/// Handle a websocket connection.
async fn serve_websocket(
    web_socket: HyperWebsocket,
    callback: Arc<dyn MyWebSockeCallback + Send + Sync + 'static>,
    addr: SocketAddr,
) -> Result<(), Error> {
    let websocket = web_socket.await?;

    let (write, mut read) = websocket.split();

    let my_web_socket = MyWebSocket::new(write, addr);

    let my_web_socket = Arc::new(my_web_socket);

    callback.connected(my_web_socket.clone()).await;

    while let Some(message) = read.next().await {
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

    callback.disconnected(my_web_socket.clone()).await;

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
