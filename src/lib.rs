mod tonic_vendor;

use bytes::Bytes;
use dyn_clone::DynClone;
use futures_util::{
    future::{self, Either, MapErr},
    ready, FutureExt, TryFutureExt,
};
use hyper::{Body, Request, Response};
use std::{
    fmt::Debug,
    net::SocketAddr,
    sync::Arc,
    task::{Context, Poll},
};
use tonic::transport::{Error, NamedService};
use tonic::{
    body::BoxBody,
    codegen::{BoxFuture, Never},
};
use tonic_vendor::transport::{server::Unimplemented, Server};
use tower::{layer::util::Identity, Service};

use crate::tonic_vendor::transport::server::TcpIncoming;

type StdError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
pub struct Router {
    server: Server<Identity>,
    routes: Routes,
}

impl Router {
    pub fn new() -> Self {
        let pred = Box::new(|_: &Request<Body>| false);
        Self {
            routes: Routes::new(pred, Unimplemented::default(), Unimplemented::default()),
            server: Server::builder(),
        }
    }

    pub fn add_service<S>(&mut self, service: S) -> Self
    where
        S: Service<
                Request<Body>,
                Response = Response<BoxBody>,
                Error = Never,
                Future = BoxFuture<Response<BoxBody>, tonic::codegen::Never>,
            > + NamedService
            + Clone
            + Send
            + Sync
            + 'static,
    {
        let service = BoxedService::from_never_error(service);
        println!("adding {:?}", service.service_name.as_ref());
        let pred: Box<dyn Fn(&Request<Body>) -> bool + Send + Sync> =
            if let Some(name) = service.service_name.as_ref() {
                let service_route = format!("/{}", name);
                Box::new(move |req: &Request<Body>| {
                    let path = req.uri().path();
                    println!("starts with {}?", path);
                    path.starts_with(&service_route)
                })
            } else {
                Box::new(|req: &Request<Body>| false)
            };
        Self {
            routes: Routes::new(pred, service, self.routes.clone()),
            server: self.server.clone(),
        }
    }

    pub async fn serve(self, addr: SocketAddr) -> Result<(), StdError> {
        let incoming = TcpIncoming::new(addr, self.server.tcp_nodelay, self.server.tcp_keepalive)
            .map_err(|e| todo!())
            .unwrap();
        self.server
            .serve_with_shutdown::<_, _, future::Ready<()>, _, _, _>(self.routes, incoming, None)
            .await
            .unwrap();
        todo!();
    }
}

#[derive(Debug)]
pub struct Routes {
    routes: Or,
}

impl Clone for Routes {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
        }
    }
}

impl Routes {
    pub(crate) fn new(
        predicate: impl Fn(&Request<Body>) -> bool + Send + Sync + 'static,
        a: impl Into<BoxedService>,
        b: impl Into<BoxedService>,
    ) -> Self {
        let routes = Or::new(predicate, a, b);
        Self { routes }
    }
}

impl Service<Request<Body>> for Routes {
    type Response = Response<BoxBody>;
    type Error = StdError;
    type Future = <Or as Service<Request<Body>>>::Future;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.routes.call(req)
    }
}

pub struct Or {
    predicate: Arc<dyn Fn(&Request<Body>) -> bool + Send + Sync + 'static>,
    a: BoxedService,
    b: BoxedService,
}

impl Clone for Or {
    fn clone(&self) -> Self {
        Self {
            predicate: self.predicate.clone(),
            a: self.a.clone(),
            b: self.b.clone(),
        }
    }
}

impl Or {
    pub(crate) fn new<F>(
        predicate: F,
        a: impl Into<BoxedService>,
        b: impl Into<BoxedService>,
    ) -> Self
    where
        F: Fn(&Request<Body>) -> bool + Send + Sync + 'static,
    {
        let predicate = Arc::new(predicate);
        Self {
            predicate,
            a: a.into(),
            b: b.into(),
        }
    }
}

impl std::fmt::Debug for Or {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Or").finish()
    }
}

impl Service<Request<Body>> for Or {
    type Response = Response<BoxBody>;
    type Error = StdError;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        if (self.predicate)(&req) {
            Box::pin(self.a.call(req).map_err(|e| e.into()))
        } else {
            Box::pin(self.b.call(req).map_err(|e| e.into()))
        }
    }
}

trait StoredService<Request>: Service<Request> + DynClone {}
impl<T, Request> StoredService<Request> for T where T: Service<Request> + Clone {}

#[derive(Clone)]
struct BoxedService {
    service_name: Option<String>,
    service: InnerBoxedService,
}

enum InnerBoxedService {
    Unimplemented(Unimplemented),
    Routes(Box<Routes>),
    NeverError(
        Box<
            dyn StoredService<
                    Request<Body>,
                    Response = Response<BoxBody>,
                    Error = tonic::codegen::Never,
                    Future = BoxFuture<Response<BoxBody>, tonic::codegen::Never>,
                > + Send
                + Sync,
        >,
    ),
}

impl BoxedService {
    fn from_never_error<T>(t: T) -> Self
    where
        T: Service<
                hyper::Request<hyper::Body>,
                Error = tonic::codegen::Never,
                Response = hyper::Response<BoxBody>,
                Future = BoxFuture<Response<BoxBody>, tonic::codegen::Never>,
            > + NamedService
            + Clone
            + Send
            + Sync
            + 'static,
    {
        Self {
            service_name: Some(T::NAME.to_string()),
            service: InnerBoxedService::NeverError(Box::new(t)),
        }
    }
}

impl From<Routes> for BoxedService {
    fn from(routes: Routes) -> Self {
        Self {
            service_name: None,
            service: InnerBoxedService::Routes(Box::new(routes)),
        }
    }
}

impl From<Unimplemented> for BoxedService {
    fn from(unimplemented: Unimplemented) -> Self {
        Self {
            service_name: None,
            service: InnerBoxedService::Unimplemented(unimplemented),
        }
    }
}

impl Debug for BoxedService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxedService")
            .field("service_name", &self.service_name)
            .finish()
    }
}

impl Clone for InnerBoxedService {
    fn clone(&self) -> Self {
        match self {
            Self::Unimplemented(unimplemented) => Self::Unimplemented(unimplemented.clone()),
            Self::Routes(routes) => Self::Routes(routes.clone()),
            Self::NeverError(inner) => Self::NeverError(dyn_clone::clone_box(inner.as_ref())),
        }
    }
}

impl Service<Request<Body>> for BoxedService {
    type Response = Response<BoxBody>;
    type Error = StdError;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.service.call(req)
    }

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }
}

impl Service<Request<Body>> for InnerBoxedService {
    type Response = Response<BoxBody>;
    type Error = StdError;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        match self {
            Self::NeverError(inner) => Box::pin(inner.call(req).err_into()),
            Self::Routes(routes) => routes.call(req),
            Self::Unimplemented(unimplemented) => unimplemented.call(req).boxed(),
        }
    }

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match self {
            Self::NeverError(inner) => inner.poll_ready(cx).map_err(|e| e.into()),
            Self::Routes(routes) => routes.poll_ready(cx),
            Self::Unimplemented(unimplemented) => unimplemented.poll_ready(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_into_boxed_service() {}
}
