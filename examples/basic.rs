use tonic_router_2::Router;
use tonic_router_2_test_protos::{echo, MyEcho, MyEcho2, MyEcho3};

#[tokio::main]
async fn main() {
    let router = Router::new()
        .add_service(echo::echo_server::EchoServer::new(MyEcho::default()))
        .add_service(echo::echo2_server::Echo2Server::new(MyEcho2::default()))
        .add_service(echo::echo3_server::Echo3Server::new(MyEcho3::default()));

    let addr = "[::1]:50051".parse().unwrap();

    router.serve(addr).await;
}
