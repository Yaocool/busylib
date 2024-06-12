#[cfg(feature = "http-util")]
pub mod body;
#[cfg(feature = "http-client")]
pub mod client;
#[cfg(feature = "http-util")]
pub mod convert;
pub mod error;

#[cfg(feature = "http-util")]
pub type HttpRequest = http::Request<body::Body>;
#[cfg(feature = "http-util")]
pub type HttpResponse = http::Response<body::Body>;
