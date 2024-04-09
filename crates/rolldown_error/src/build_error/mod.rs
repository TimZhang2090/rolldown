pub mod error_constructors;
pub mod severity;
use std::fmt::Display;

use crate::{diagnostic::Diagnostic, events::BuildEvent};

use self::severity::Severity;

#[derive(Debug)]
pub struct BuildError {
  inner: Box<dyn BuildEvent>,
  source: Option<Box<dyn std::error::Error + 'static + Send + Sync>>,
  severity: Severity,
}

fn _assert_build_error_send_sync() {
  fn _assert_send_sync<T: Send + Sync>() {}
  _assert_send_sync::<BuildError>();
}

impl Display for BuildError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.inner.message().fmt(f)
  }
}

impl std::error::Error for BuildError {
  // clippy::option_map_or_none: Cool. Finally, catch a error of clippy. Clippy suggest using `self.source.as_ref().map(|source| source.as_ref())`
  // which will cause type mismatch error.
  #[allow(clippy::option_map_or_none)]
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    self.source.as_ref().map_or(None, |source| Some(source.as_ref()))
  }
}

impl BuildError {
  pub fn kind(&self) -> crate::event_kind::EventKind {
    self.inner.kind()
  }

  pub fn code(&self) -> &'static str {
    self.inner.code()
  }

  #[must_use]
  pub fn with_source(
    mut self,
    source: impl Into<Box<dyn std::error::Error + 'static + Send + Sync>>,
  ) -> Self {
    self.source = Some(source.into());
    self
  }

  #[must_use]
  pub fn with_severity_warning(mut self) -> Self {
    self.severity = Severity::Warning;
    self
  }

  pub fn into_diagnostic(self) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(self.code(), self.inner.message(), self.severity);
    self.inner.on_diagnostic(&mut diagnostic);
    diagnostic
  }

  // --- private

  fn new_inner(inner: impl Into<Box<dyn BuildEvent>>) -> Self {
    Self { inner: inner.into(), source: None, severity: Severity::Error }
  }
}

impl From<std::io::Error> for BuildError {
  fn from(e: std::io::Error) -> Self {
    Self::new_inner(e)
  }
}

#[cfg(feature = "napi")]
impl From<napi::Error> for BuildError {
  fn from(e: napi::Error) -> Self {
    BuildError::napi_error(e.status.to_string(), e.reason)
  }
}
