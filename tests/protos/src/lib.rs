pub mod echo {
    tonic::include_proto!("echo");
}

#[derive(Default, Debug)]
pub struct MyEcho {}

#[tonic::async_trait]
impl echo::echo_server::Echo for MyEcho {
    async fn echo(
        &self,
        request: tonic::Request<echo::EchoRequest>,
    ) -> Result<tonic::Response<echo::EchoResponse>, tonic::Status> {
        let reply = echo::EchoResponse {
            message: Some(format!("echo 1: {}", request.into_inner().message.unwrap())),
        };
        Ok(tonic::Response::new(reply))
    }
}

#[derive(Default, Debug)]
pub struct MyEcho2 {}

#[tonic::async_trait]
impl echo::echo2_server::Echo2 for MyEcho2 {
    async fn echo(
        &self,
        request: tonic::Request<echo::EchoRequest2>,
    ) -> Result<tonic::Response<echo::EchoResponse2>, tonic::Status> {
        let reply = echo::EchoResponse2 {
            message: Some(format!("echo 2: {}", request.into_inner().message.unwrap())),
        };
        Ok(tonic::Response::new(reply))
    }
}

#[derive(Default, Debug)]
pub struct MyEcho3 {}

#[tonic::async_trait]
impl echo::echo3_server::Echo3 for MyEcho3 {
    async fn echo(
        &self,
        request: tonic::Request<echo::EchoRequest3>,
    ) -> Result<tonic::Response<echo::EchoResponse3>, tonic::Status> {
        let reply = echo::EchoResponse3 {
            message: Some(format!("echo 3: {}", request.into_inner().message.unwrap())),
        };
        Ok(tonic::Response::new(reply))
    }
}
