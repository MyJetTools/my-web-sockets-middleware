use std::sync::Arc;

use my_http_server::{
    HttpContext, HttpFailResult, HttpOkResult, HttpPath, HttpServerMiddleware,
    HttpServerRequestFlow, RequestData,
};
use my_http_server_web_sockets::MyWebSocketCallback;
use tokio::sync::Mutex;

pub struct MyWebSocketsMiddleware<TMyWebSocketCallback>
where
    TMyWebSocketCallback: MyWebSocketCallback + Send + Sync + 'static,
{
    path: HttpPath,
    callback: Arc<TMyWebSocketCallback>,
    socket_id: Mutex<i64>,
}

impl<TMyWebSocketCallback: MyWebSocketCallback> MyWebSocketsMiddleware<TMyWebSocketCallback>
where
    TMyWebSocketCallback: MyWebSocketCallback + Send + Sync + 'static,
{
    pub fn new(path: &str, callback: Arc<TMyWebSocketCallback>) -> Self {
        Self {
            path: HttpPath::new(path),
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
impl<TMyWebSocketCallback: MyWebSocketCallback> HttpServerMiddleware
    for MyWebSocketsMiddleware<TMyWebSocketCallback>
where
    TMyWebSocketCallback: MyWebSocketCallback + Send + Sync + 'static,
{
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
            if ctx.request.http_path.is_the_same_to(&self.path) {
                if let RequestData::AsRaw(request) = &mut ctx.request.req {
                    let id = self.get_socket_id().await;
                    return my_http_server_web_sockets::handle_web_socket_upgrade(
                        request,
                        self.callback.clone(),
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
