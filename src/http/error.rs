use std::fmt;

#[derive(Debug)]
pub struct Error {
    #[allow(dead_code)]
    inner: BoxError,
}

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

impl Error {
    pub fn new(error: impl Into<BoxError>) -> Self {
        Self {
            inner: error.into(),
        }
    }

    pub fn into_inner(self) -> BoxError {
        self.inner
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.inner)
    }
}
