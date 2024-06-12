use crate::http::body::Body;
use crate::http::error::Error;
use crate::http::{HttpRequest, HttpResponse};
use bytes::Bytes;

pub trait FromBytes {
    type BytesWrapper;
    fn from_bytes(bytes_wrapper: Self::BytesWrapper) -> Self;
}

impl FromBytes for HttpRequest {
    type BytesWrapper = http::Request<Bytes>;
    fn from_bytes(bytes_wrapper: Self::BytesWrapper) -> Self {
        let (p, b) = bytes_wrapper.into_parts();
        Self::from_parts(p, Body::from(b))
    }
}

impl FromBytes for HttpResponse {
    type BytesWrapper = http::Response<Bytes>;
    fn from_bytes(bytes_wrapper: Self::BytesWrapper) -> Self {
        let (p, b) = bytes_wrapper.into_parts();
        Self::from_parts(p, Body::from(b))
    }
}

pub trait ToBytes {
    type BytesWrapper;
    fn to_bytes(
        self,
    ) -> impl std::future::Future<Output = Result<Self::BytesWrapper, Error>> + Send;
}

impl ToBytes for HttpRequest {
    type BytesWrapper = http::Request<Bytes>;
    async fn to_bytes(self) -> Result<Self::BytesWrapper, Error> {
        let (p, b) = self.into_parts();
        Ok(http::Request::from_parts(p, b.to_bytes().await?))
    }
}

impl ToBytes for HttpResponse {
    type BytesWrapper = http::Response<Bytes>;
    async fn to_bytes(self) -> Result<Self::BytesWrapper, Error> {
        let (p, b) = self.into_parts();
        Ok(http::Response::from_parts(p, b.to_bytes().await?))
    }
}
