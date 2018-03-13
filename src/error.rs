//! Functionalities for implementing trackable errors and operating on those.
//!
//! You can easily define your own trackable error types as follows:
//!
//! ```
//! #[macro_use]
//! extern crate trackable;
//! use trackable::error::{TrackableError, ErrorKind, ErrorKindExt};
//!
//! #[derive(Debug)]
//! struct MyError(TrackableError<MyErrorKind>);
//! derive_traits_for_trackable_error_newtype!(MyError, MyErrorKind);
//! impl From<std::io::Error> for MyError {
//!     fn from(f: std::io::Error) -> Self {
//!         // Any I/O errors are considered critical
//!         MyErrorKind::Critical.cause(f).into()
//!     }
//! }
//!
//! # #[allow(dead_code)]
//! #[derive(Debug, PartialEq, Eq)]
//! enum MyErrorKind {
//!     Critical,
//!     NonCritical,
//! }
//! impl ErrorKind for MyErrorKind {}
//!
//! fn main() {
//!     // Tracks an error
//!     let error: MyError = MyErrorKind::Critical.cause("something wrong").into();
//!     let error = track!(error);
//!     let error = track!(error, "I passed here");
//!     assert_eq!(format!("\nError: {}", error).replace('\\', "/"), r#"
//! Error: Critical (cause; something wrong)
//! HISTORY:
//!   [0] at src/error.rs:27
//!   [1] at src/error.rs:28 -- I passed here
//! "#);
//!
//!     // Tries to execute I/O operation
//!     let result = (|| -> Result<_, MyError> {
//!         let f = track!(std::fs::File::open("/path/to/non_existent_file")
//!                        .map_err(MyError::from))?;
//!         Ok(f)
//!     })();
//!     let error = result.err().unwrap();
//!     let cause = error.concrete_cause::<std::io::Error>().unwrap();
//!     assert_eq!(cause.kind(), std::io::ErrorKind::NotFound);
//! }
//! ```
use std::fmt;
use std::error::Error;
use std::sync::Arc;

use super::{Location, Trackable};

/// Boxed `Error` object.
pub type BoxError = Box<Error + Send + Sync>;

/// Boxed `ErrorKind` object.
pub type BoxErrorKind = Box<ErrorKind + Send + Sync>;

/// `History` type specialized for `TrackableError`.
pub type History = ::History<Location>;

/// Built-in `ErrorKind` implementation which represents opaque errors.
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Failed;
impl ErrorKind for Failed {
    fn description(&self) -> &str {
        "Failed"
    }
}

/// `TrackableError` type specialized for `Failed`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Failure(TrackableError<Failed>);
derive_traits_for_trackable_error_newtype!(Failure, Failed);
impl Failure {
    /// Makes a new `Failure` instance which was caused by the `error`.
    pub fn from_error<E>(error: E) -> Self
    where
        E: Into<BoxError>,
    {
        Failed.cause(error).into()
    }
}

/// This trait represents a error kind which `TrackableError` can have.
pub trait ErrorKind: fmt::Debug {
    /// A short description of the error kind.
    ///
    /// This is used for the description of the error that contains it.
    ///
    /// The default implementation always returns `"An error"`.
    fn description(&self) -> &str {
        "An error"
    }

    /// Displays this kind.
    ///
    /// The default implementation uses the debugging form of this.
    fn display(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl ErrorKind for String {
    fn description(&self) -> &str {
        self
    }
}

/// An extention of `ErrorKind` trait.
///
/// This provides convenient functions to create a `TrackableError` instance of this kind.
pub trait ErrorKindExt: ErrorKind + Sized {
    /// Makes a `TrackableError` instance without cause.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::error::Error;
    /// use trackable::error::{Failed, ErrorKindExt};
    ///
    /// let e = Failed.error();
    /// assert!(e.cause().is_none());
    /// ```
    #[inline]
    fn error(self) -> TrackableError<Self> {
        self.into()
    }

    /// Makes a `TrackableError` instance with the specified `cause`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::error::Error;
    /// use trackable::error::{Failed, ErrorKindExt};
    ///
    /// let e = Failed.cause("something wrong");
    /// assert_eq!(e.cause().unwrap().to_string(), "something wrong");
    /// ```
    #[inline]
    fn cause<E>(self, cause: E) -> TrackableError<Self>
    where
        E: Into<BoxError>,
    {
        TrackableError::new(self, cause.into())
    }

    /// Takes over from other `TrackableError` instance.
    ///
    /// The history of `from` will be preserved.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use]
    /// # extern crate trackable;
    /// #
    /// use trackable::error::{ErrorKind, ErrorKindExt};
    ///
    /// #[derive(Debug)]
    /// struct Kind0;
    /// impl ErrorKind for Kind0 {}
    ///
    /// #[derive(Debug)]
    /// struct Kind1;
    /// impl ErrorKind for Kind1 {}
    ///
    /// fn main() {
    ///   let e = Kind0.error();
    ///   let e = track!(e);
    ///
    ///   let e = Kind1.takes_over(e);
    ///   let e = track!(e);
    ///
    ///   assert_eq!(format!("\nERROR: {}", e).replace('\\', "/"), r#"
    /// ERROR: Kind1
    /// HISTORY:
    ///   [0] at src/error.rs:17
    ///   [1] at src/error.rs:20
    /// "#);
    /// }
    /// ```
    fn takes_over<F, K>(self, from: F) -> TrackableError<Self>
    where
        F: Into<TrackableError<K>>,
        K: ErrorKind + Send + Sync + 'static,
    {
        let from = from.into();
        TrackableError {
            kind: self,
            cause: from.cause,
            history: from.history,
        }
    }
}
impl<T: ErrorKind> ErrorKindExt for T {}

/// Trackable error.
///
/// # Examples
///
/// Defines your own `Error` type.
///
/// ```
/// #[macro_use]
/// extern crate trackable;
/// use trackable::error::{TrackableError, ErrorKind, ErrorKindExt};
///
/// #[derive(Debug)]
/// struct MyError(TrackableError<MyErrorKind>);
/// derive_traits_for_trackable_error_newtype!(MyError, MyErrorKind);
/// impl From<std::io::Error> for MyError {
///     fn from(f: std::io::Error) -> Self {
///         // Any I/O errors are considered critical
///         MyErrorKind::Critical.cause(f).into()
///     }
/// }
///
/// # #[allow(dead_code)]
/// #[derive(Debug, PartialEq, Eq)]
/// enum MyErrorKind {
///     Critical,
///     NonCritical,
/// }
/// impl ErrorKind for MyErrorKind {}
///
/// fn main() {
///     // Tracks an error
///     let error: MyError = MyErrorKind::Critical.cause("something wrong").into();
///     let error = track!(error);
///     let error = track!(error, "I passed here");
///     assert_eq!(format!("\nError: {}", error).replace('\\', "/"), r#"
/// Error: Critical (cause; something wrong)
/// HISTORY:
///   [0] at src/error.rs:27
///   [1] at src/error.rs:28 -- I passed here
/// "#);
///
///     // Tries to execute I/O operation
///     let result = (|| -> Result<_, MyError> {
///         let f = track!(std::fs::File::open("/path/to/non_existent_file")
///                        .map_err(MyError::from))?;
///         Ok(f)
///     })();
///     let error = result.err().unwrap();
///     let cause = error.concrete_cause::<std::io::Error>().unwrap();
///     assert_eq!(cause.kind(), std::io::ErrorKind::NotFound);
/// }
/// ```
///
/// `TrackableError` is cloneable if `K` is so.
///
/// ```no_run
/// #[macro_use]
/// extern crate trackable;
///
/// use trackable::Trackable;
/// use trackable::error::{Failed, ErrorKindExt};
///
/// fn main() {
///     let mut original = Failed.error();
///
///     let original = track!(original, "Hello `original`!");
///     let forked = original.clone();
///     let forked = track!(forked, "Hello `forked`!");
///
///     assert_eq!(format!("\n{}", original).replace('\\', "/"), r#"
/// Failed
/// HISTORY:
///   [0] at src/error.rs:11 -- Hello `original`!
/// "#);
///
///     assert_eq!(format!("\n{}", forked).replace('\\', "/"), r#"
/// Failed
/// HISTORY:
///   [0] at src/error.rs:11 -- Hello `original`!
///   [1] at src/error.rs:13 -- Hello `forked`!
/// "#);
/// }
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct TrackableError<K> {
    kind: K,
    cause: Option<Cause>,
    history: History,
}
impl<K: ErrorKind> TrackableError<K> {
    /// Makes a new `TrackableError` instance.
    pub fn new<E>(kind: K, cause: E) -> Self
    where
        E: Into<BoxError>,
    {
        TrackableError {
            kind,
            cause: Some(Cause(Arc::new(cause.into()))),
            history: History::new(),
        }
    }

    /// Makes a new `TrackableError` instance from `kind`.
    ///
    /// Note that the returning error has no cause.
    fn from_kind(kind: K) -> Self {
        TrackableError {
            kind,
            cause: None,
            history: History::new(),
        }
    }

    /// Returns the kind of this error.
    #[inline]
    pub fn kind(&self) -> &K {
        &self.kind
    }

    /// Tries to return the cause of this error as a value of `T` type.
    ///
    /// If neither this error has a cause nor it is an `T` value,
    /// this method will return `None`.
    #[inline]
    pub fn concrete_cause<T>(&self) -> Option<&T>
    where
        T: Error + 'static,
    {
        self.cause.as_ref().and_then(|c| c.0.downcast_ref())
    }
}
impl<K: ErrorKind> From<K> for TrackableError<K> {
    #[inline]
    fn from(kind: K) -> Self {
        Self::from_kind(kind)
    }
}
impl<K: ErrorKind + Default> Default for TrackableError<K> {
    #[inline]
    fn default() -> Self {
        Self::from_kind(K::default())
    }
}
impl<K: ErrorKind> fmt::Display for TrackableError<K> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.kind.display(f)?;
        if let Some(ref e) = self.cause {
            write!(f, " (cause; {})", e.0)?;
        }
        write!(f, "\n{}", self.history)?;
        Ok(())
    }
}
impl<K: ErrorKind> Error for TrackableError<K> {
    fn description(&self) -> &str {
        self.kind.description()
    }
    fn cause(&self) -> Option<&Error> {
        if let Some(ref e) = self.cause {
            Some(&**e.0)
        } else {
            None
        }
    }
}
impl<K> Trackable for TrackableError<K> {
    type Event = Location;

    #[inline]
    fn history(&self) -> Option<&History> {
        Some(&self.history)
    }

    #[inline]
    fn history_mut(&mut self) -> Option<&mut History> {
        Some(&mut self.history)
    }
}

#[derive(Debug, Clone)]
struct Cause(Arc<BoxError>);

#[cfg(feature = "serialize")]
mod impl_serde {
    use std::sync::Arc;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::Cause;

    impl Serialize for Cause {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.0.to_string())
        }
    }
    impl<'de> Deserialize<'de> for Cause {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            Ok(Cause(Arc::new(s.into())))
        }
    }
}

#[cfg(test)]
mod test {
    use std;
    use super::*;

    #[test]
    fn it_works() {
        #[derive(Debug)]
        struct MyError(TrackableError<MyErrorKind>);
        derive_traits_for_trackable_error_newtype!(MyError, MyErrorKind);
        impl From<std::io::Error> for MyError {
            fn from(f: std::io::Error) -> Self {
                // Any I/O errors are considered critical
                MyErrorKind::Critical.cause(f).into()
            }
        }

        #[derive(Debug, PartialEq, Eq)]
        enum MyErrorKind {
            Critical,
            NonCritical,
        }
        impl ErrorKind for MyErrorKind {}

        // Tracks an error
        let error: MyError = MyErrorKind::Critical.cause("something wrong").into();
        let error = track!(error);
        let error = track!(error, "I passed here");
        assert_eq!(
            format!("\nError: {}", error).replace('\\', "/"),
            r#"
Error: Critical (cause; something wrong)
HISTORY:
  [0] at src/error.rs:439
  [1] at src/error.rs:440 -- I passed here
"#
        );

        // Tries to execute I/O operation
        let result = (|| -> Result<_, MyError> {
            let f =
                track!(std::fs::File::open("/path/to/non_existent_file").map_err(MyError::from,))?;
            Ok(f)
        })();
        let error = result.err().unwrap();
        let cause = error.concrete_cause::<std::io::Error>().unwrap();
        assert_eq!(cause.kind(), std::io::ErrorKind::NotFound);
    }
}
