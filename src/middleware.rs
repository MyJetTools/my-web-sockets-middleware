use std::sync::Arc;

use my_http_server::{
    HttpContext, HttpFailResult, HttpOkResult, HttpServerMiddleware, HttpServerRequestFlow,
    RequestData,
};
use my_http_server_web_sockets::MyWebSockeCallback;
use tokio::sync::Mutex;

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
            .get_optional_header("sec-websocket-key")
            .is_some()
        {
            if ctx.request.get_path_lower_case() == self.path {
                if let RequestData::AsRaw(request) = &mut ctx.request.req {
                    let id = self.get_socket_id().await;
                    return my_http_server_web_sockets::handle_web_socket_upgrade(
                        request,
                        &self.callback,
                        id,
                        ctx.request.addr,
                    )
                    .await;
                }
            }
        }

        get_next.next(ctx).await
    }
}
