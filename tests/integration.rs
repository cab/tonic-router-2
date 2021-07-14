use tonic_router_2::Router;
use tonic_router_2_test_protos::{echo, MyEcho, MyEcho2, MyEcho3};

#[test]
fn test_2() {
    let router = Router::new()
        .add_service(echo::echo_server::EchoServer::new(MyEcho::default()))
        .add_service(echo::echo_server::EchoServer::new(MyEcho2::default()))
        .add_service(echo::echo_server::EchoServer::new(MyEcho3::default()));
}
