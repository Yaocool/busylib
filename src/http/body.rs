use std::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::http::error::{BoxError, Error};
use bytes::Bytes;
use futures_core::TryStream;
use http_body::Frame;
use http_body_util::{combinators::UnsyncBoxBody, BodyExt};
use pin_project_lite::pin_project;
use sync_wrapper::SyncWrapper;

#[derive(Debug)]
pub struct Body(pub BoxBody);

type BoxBody = UnsyncBoxBody<Bytes, Error>;

impl Body {
    pub fn new<B>(body: B) -> Self
    where
        B: http_body::Body<Data = Bytes> + Send + 'static,
        B::Error: Into<BoxError>,
    {
        Self::try_downcast(body).unwrap_or_else(|body| Self(Self::boxed(body)))
    }

    pub fn empty() -> Self {
        Self::new(http_body_util::Empty::new())
    }

    fn boxed<B>(body: B) -> BoxBody
    where
        B: http_body::Body<Data = Bytes> + Send + 'static,
        B::Error: Into<BoxError>,
    {
        Self::try_downcast(body).unwrap_or_else(|body| body.map_err(Error::new).boxed_unsync())
    }

    pub async fn to_bytes(self) -> Result<Bytes, Error> {
        Ok(self.0.collect().await?.to_bytes())
    }

    pub fn from_stream<S>(stream: S) -> Self
    where
        S: TryStream + Send + 'static,
        S::Ok: Into<Bytes>,
        S::Error: Into<BoxError>,
    {
        Self::new(StreamBody {
            stream: SyncWrapper::new(stream),
        })
    }

    pub(crate) fn try_downcast<T, K>(k: K) -> Result<T, K>
    where
        T: 'static,
        K: Send + 'static,
    {
        let mut k = Some(k);
        if let Some(k) = <dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k) {
            Ok(k.take().unwrap())
        } else {
            Err(k.unwrap())
        }
    }
}

impl From<BoxBody> for Body {
    fn from(body: BoxBody) -> Self {
        Body(body)
    }
}

impl From<()> for Body {
    fn from(_: ()) -> Self {
        Self::empty()
    }
}

macro_rules! body_from_impl {
    ($ty:ty) => {
        impl From<$ty> for Body {
            fn from(buf: $ty) -> Self {
                Self::new(http_body_util::Full::from(buf))
            }
        }
    };
}

body_from_impl!(&'static [u8]);
body_from_impl!(std::borrow::Cow<'static, [u8]>);
body_from_impl!(Vec<u8>);

body_from_impl!(&'static str);
body_from_impl!(std::borrow::Cow<'static, str>);
body_from_impl!(String);

body_from_impl!(Bytes);

pin_project! {
    struct StreamBody<S> {
        #[pin]
        stream: SyncWrapper<S>,
    }
}

impl<S> http_body::Body for StreamBody<S>
where
    S: TryStream,
    S::Ok: Into<Bytes>,
    S::Error: Into<BoxError>,
{
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let stream = self.project().stream.get_pin_mut();
        match futures_util::ready!(stream.try_poll_next(cx)) {
            Some(Ok(chunk)) => Poll::Ready(Some(Ok(Frame::data(chunk.into())))),
            Some(Err(err)) => Poll::Ready(Some(Err(Error::new(err)))),
            None => Poll::Ready(None),
        }
    }
}
