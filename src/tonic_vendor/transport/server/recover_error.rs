use super::super::super::option_pin::{OptionPin, OptionPinProj};
use futures_util::ready;
use http::{HeaderMap, HeaderValue, Response};
use pin_project::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tonic::Status;
use tower::Service;

/// Middleware that attempts to recover from service errors by turning them into a response built
/// from the `Status`.
#[derive(Debug, Clone)]
pub(crate) struct RecoverError<S> {
    inner: S,
}

impl<S> RecoverError<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, R, ResBody> Service<R> for RecoverError<S>
where
    S: Service<R, Response = Response<ResBody>>,
    S::Error: Into<crate::tonic_vendor::Error>,
{
    type Response = Response<MaybeEmptyBody<ResBody>>;
    type Error = crate::tonic_vendor::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: R) -> Self::Future {
        ResponseFuture {
            inner: self.inner.call(req),
        }
    }
}

#[pin_project]
pub(crate) struct ResponseFuture<F> {
    #[pin]
    inner: F,
}

impl<F, E, ResBody> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<ResBody>, E>>,
    E: Into<crate::tonic_vendor::Error>,
{
    type Output = Result<Response<MaybeEmptyBody<ResBody>>, crate::tonic_vendor::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result: Result<Response<_>, crate::tonic_vendor::Error> =
            ready!(self.project().inner.poll(cx)).map_err(Into::into);

        match result {
            Ok(response) => {
                let response = response.map(MaybeEmptyBody::full);
                Poll::Ready(Ok(response))
            }
            Err(err) => match try_status_from_error(err) {
                Ok(status) => {
                    let mut res = Response::new(MaybeEmptyBody::empty());
                    // TODO
                    // add_header(&mut status, res.headers_mut()).unwrap();
                    Poll::Ready(Ok(res))
                }
                Err(err) => Poll::Ready(Err(err)),
            },
        }
    }
}

fn try_status_from_error(
    err: Box<dyn std::error::Error + Send + Sync + 'static>,
) -> Result<Status, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let err = match err.downcast::<Status>() {
        Ok(status) => {
            return Ok(*status);
        }
        Err(err) => err,
    };

    #[cfg(feature = "transport")]
    let err = match err.downcast::<h2::Error>() {
        Ok(h2) => {
            return Ok(Status::from_h2_error(&*h2));
        }
        Err(err) => err,
    };

    if let Some(status) = find_status_in_source_chain(&*err) {
        return Ok(status);
    }

    Err(err)
}

fn find_status_in_source_chain(err: &(dyn std::error::Error + 'static)) -> Option<Status> {
    let mut source = Some(err);

    while let Some(err) = source {
        if let Some(status) = err.downcast_ref::<Status>() {
            let status = Status::new(status.code().clone(), status.message().to_string());
            return Some(status);
        }

        #[cfg(feature = "transport")]
        if let Some(timeout) = err.downcast_ref::<crate::transport::TimeoutExpired>() {
            return Some(Status::cancelled(timeout.to_string()));
        }

        source = err.source();
    }

    None
}

#[pin_project]
pub(crate) struct MaybeEmptyBody<B> {
    #[pin]
    inner: OptionPin<B>,
}

impl<B> MaybeEmptyBody<B> {
    fn full(inner: B) -> Self {
        Self {
            inner: OptionPin::Some(inner),
        }
    }

    fn empty() -> Self {
        Self {
            inner: OptionPin::None,
        }
    }
}

impl<B> http_body::Body for MaybeEmptyBody<B>
where
    B: http_body::Body + Send,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        match self.project().inner.project() {
            OptionPinProj::Some(b) => b.poll_data(cx),
            OptionPinProj::None => Poll::Ready(None),
        }
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        match self.project().inner.project() {
            OptionPinProj::Some(b) => b.poll_trailers(cx),
            OptionPinProj::None => Poll::Ready(Ok(None)),
        }
    }

    fn is_end_stream(&self) -> bool {
        match &self.inner {
            OptionPin::Some(b) => b.is_end_stream(),
            OptionPin::None => true,
        }
    }
}
