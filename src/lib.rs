// This file contains parts that are Copyright 2015 The Rust Project Developers, copied from:
// https://raw.githubusercontent.com/rust-lang/rust/cb2a656cdfb6400ac0200c661267f91fabf237e2/src/libstd/path.rs

//! A platform-neutral relative path.
//!
//! This library is analogous to `std::path::Path`, and `std::path::PathBuf`, with the exception of
//! the following characteristics:
//!
//! * The path separator is set to a fixed character (`/`), regardless of platform.
//! * Relative paths cannot represent an absolute path. Any slash (`/`) prefixes provided will only
//!   apply to operations involving relative paths, they will be considered as relative to the
//!   path provided during conversion.

use std::hash::{Hash, Hasher};
use std::mem;
use std::borrow;
use std::borrow::Cow;
use std::path;
use std::fmt;
use std::ops;
use std::cmp;
#[cfg(feature = "serde")]
use std::result;

#[cfg(feature = "serde")]
extern crate serde;

#[cfg(feature = "serde")]
use serde::ser::{Serialize, Serializer};
#[cfg(feature = "serde")]
use serde::de::{self, Deserialize, Deserializer, Visitor};

const SEP: char = '/';
const SEP_BYTE: u8 = SEP as u8;

/// Scan backwards until the given separator has been encountered using the provided `cmp`.
macro_rules! scan_back {
    ($source:expr, $init:expr, $cmp:tt, $sep:expr) => {{
            let mut n = $init;

            while n > 0 && $source[n - 1] $cmp $sep {
                n -= 1;
            }

            n
    }}
}

/// Scan forward until the given separator has been encountered using the provided `cmp`.
macro_rules! scan_forward {
    ($source:expr, $init:expr, $cmp:tt, $sep:expr) => {{
        let mut n = $init;

        while n < $source.len() && $source[n] $cmp $sep {
            n += 1;
        }

        n
    }}
}

/// Iterator over all the components in a relative path.
#[derive(Clone)]
pub struct Components<'a> {
    source: &'a [u8],
}

impl<'a> Iterator for Components<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.source.is_empty() {
            return None;
        }

        // strip prefixing separators
        let start = scan_forward!(self.source, 0usize, ==, SEP_BYTE);
        // collect component
        let end = scan_forward!(self.source, start, !=, SEP_BYTE);

        let slice = &self.source[start..end];
        self.source = &self.source[end..];

        if slice.is_empty() {
            return None;
        }

        Some(unsafe { ::std::str::from_utf8_unchecked(slice) })
    }
}

impl<'a> DoubleEndedIterator for Components<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.next_back_component().map(|(s, _)| s)
    }
}

impl<'a> Components<'a> {
    pub fn new(input: &str) -> Components {
        Components { source: input.as_bytes() }
    }

    /// Extracts the next back component and its length including separators.
    fn next_back_component(&mut self) -> Option<(&'a str, usize)> {
        if self.source.is_empty() {
            return None;
        }

        let slice_end = self.source.len();

        // strip suffixing separators
        let end = scan_back!(self.source, slice_end, ==, SEP_BYTE);
        // find component
        let start = scan_back!(self.source, end, !=, SEP_BYTE);
        // strip prefixing separator
        let slice_start = scan_back!(self.source, start, ==, SEP_BYTE);

        let slice = &self.source[start..end];
        self.source = &self.source[..slice_start];

        if slice.is_empty() {
            return None;
        }

        let slice = unsafe { ::std::str::from_utf8_unchecked(slice) };
        Some((slice, slice_end - slice_start))
    }
}

impl<'a> cmp::PartialEq for Components<'a> {
    fn eq(&self, other: &Components<'a>) -> bool {
        Iterator::eq(self.clone(), other.clone())
    }
}

/// An owned, mutable relative path.
///
/// This type provides methods to manipulate relative path objects.
#[derive(Clone)]
pub struct RelativePathBuf {
    inner: String,
}

impl RelativePathBuf {
    /// Create a new relative path buffer.
    pub fn new() -> RelativePathBuf {
        RelativePathBuf { inner: String::new() }
    }

    /// Extends `self` with `path`.
    ///
    /// If `path` is absolute, it replaces the current path.
    pub fn push<P: AsRef<RelativePath>>(&mut self, path: P) {
        let other = path.as_ref();

        if other.is_absolute() {
            self.inner.clear();
            self.inner.push_str(&other.inner);
            return;
        }

        if self.inner.len() > 0 {
            self.inner.push(SEP);
        }

        self.inner.push_str(&other.inner)
    }

    /// Coerce to a [`RelativePath`] slice.
    ///
    /// [`RelativePath`]: struct.RelativePath.html
    pub fn as_relative_path(&self) -> &RelativePath {
        self
    }
}

impl fmt::Debug for RelativePathBuf {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", &self.inner)
    }
}

impl AsRef<RelativePath> for RelativePathBuf {
    fn as_ref(&self) -> &RelativePath {
        RelativePath::new(&self.inner)
    }
}

impl borrow::Borrow<RelativePath> for RelativePathBuf {
    fn borrow(&self) -> &RelativePath {
        RelativePath::new(&self.inner)
    }
}

impl From<String> for RelativePathBuf {
    fn from(value: String) -> RelativePathBuf {
        RelativePathBuf { inner: value }
    }
}

impl ops::Deref for RelativePathBuf {
    type Target = RelativePath;

    fn deref(&self) -> &RelativePath {
        RelativePath::new(&self.inner)
    }
}

impl cmp::PartialEq for RelativePathBuf {
    fn eq(&self, other: &RelativePathBuf) -> bool {
        self.components() == other.components()
    }
}

impl cmp::Eq for RelativePathBuf {}

impl cmp::PartialOrd for RelativePathBuf {
    fn partial_cmp(&self, other: &RelativePathBuf) -> Option<cmp::Ordering> {
        self.components().partial_cmp(other.components())
    }
}

impl cmp::Ord for RelativePathBuf {
    fn cmp(&self, other: &RelativePathBuf) -> cmp::Ordering {
        self.components().cmp(other.components())
    }
}

impl Hash for RelativePathBuf {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_relative_path().hash(h)
    }
}

#[cfg(feature = "serde")]
impl Serialize for RelativePathBuf {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.inner)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RelativePathBuf {
    fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RelativePathBufVisitor;

        impl<'de> Visitor<'de> for RelativePathBufVisitor {
            type Value = RelativePathBuf;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a relative path")
            }

            fn visit_string<E>(self, input: String) -> result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(RelativePathBuf::from(input))
            }

            fn visit_str<E>(self, input: &str) -> result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(RelativePathBuf::from(input.to_string()))
            }
        }

        deserializer.deserialize_any(RelativePathBufVisitor)
    }
}

/// A borrowed, immutable relative path.
pub struct RelativePath {
    inner: str,
}

impl RelativePath {
    /// Directly wraps a string slice as a `RelativePath` slice.
    pub fn new<S: AsRef<str> + ?Sized>(s: &S) -> &RelativePath {
        unsafe { mem::transmute(s.as_ref()) }
    }

    /// Creates an owned [`RelativePathBuf`] with path adjoined to self.
    ///
    /// [`RelativePathBuf`]: struct.RelativePathBuf.html
    pub fn join<P: AsRef<RelativePath>>(&self, path: P) -> RelativePathBuf {
        let p = path.as_ref();

        if p.is_absolute() {
            return p.to_relative_path_buf();
        }

        let mut out = self.to_relative_path_buf();
        out.push(p);
        out
    }

    /// Iterate over all components in this relative path.
    pub fn components(&self) -> Components {
        Components::new(&self.inner)
    }

    /// Convert to an owned [`RelativePathBuf`].
    ///
    /// [`RelativePathBuf`]: struct.RelativePathBuf.html
    pub fn to_relative_path_buf(&self) -> RelativePathBuf {
        RelativePathBuf::from(self.inner.to_string())
    }

    /// Build an owned `PathBuf` relative to `path` for the current relative path.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use relative_path::RelativePath;
    /// use std::path::Path;
    ///
    /// let _ = RelativePath::new("foo/bar").to_relative_of(Path::new("."));
    /// ```
    pub fn to_relative_of<P: AsRef<path::Path>>(&self, relative_to: P) -> path::PathBuf {
        let mut p = relative_to.as_ref().to_path_buf();
        p.extend(self.components());
        p
    }

    /// Check if path starts with a path separator.
    pub fn is_absolute(&self) -> bool {
        self.inner.chars().next() == Some(SEP)
    }

    /// Returns a relative path, without its final component if there is one.
    pub fn parent(&self) -> Option<&RelativePath> {
        self.components().next_back_component().and_then(
            |(_, size)| {
                let slice = &self.inner[..self.inner.len() - size];

                if slice.is_empty() {
                    None
                } else {
                    Some(RelativePath::new(slice))
                }
            },
        )
    }
}

impl fmt::Debug for RelativePath {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", &self.inner)
    }
}

impl ToOwned for RelativePath {
    type Owned = RelativePathBuf;

    fn to_owned(&self) -> RelativePathBuf {
        self.to_relative_path_buf()
    }
}

impl AsRef<RelativePath> for String {
    fn as_ref(&self) -> &RelativePath {
        RelativePath::new(self)
    }
}

impl AsRef<RelativePath> for str {
    fn as_ref(&self) -> &RelativePath {
        RelativePath::new(self)
    }
}

impl AsRef<RelativePath> for RelativePath {
    fn as_ref(&self) -> &RelativePath {
        self
    }
}

impl cmp::PartialEq for RelativePath {
    fn eq(&self, other: &RelativePath) -> bool {
        self.components() == other.components()
    }
}

impl cmp::Eq for RelativePath {}

impl cmp::PartialOrd for RelativePath {
    fn partial_cmp(&self, other: &RelativePath) -> Option<cmp::Ordering> {
        self.components().partial_cmp(other.components())
    }
}

impl cmp::Ord for RelativePath {
    fn cmp(&self, other: &RelativePath) -> cmp::Ordering {
        self.components().cmp(other.components())
    }
}

impl Hash for RelativePath {
    fn hash<H: Hasher>(&self, h: &mut H) {
        for c in self.components() {
            c.hash(h);
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for RelativePath {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.inner)
    }
}

macro_rules! impl_cmp {
    ($lhs:ty, $rhs: ty) => {
        impl<'a, 'b> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                <RelativePath as PartialEq>::eq(self, other)
            }
        }

        impl<'a, 'b> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                <RelativePath as PartialEq>::eq(self, other)
            }
        }

        impl<'a, 'b> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<cmp::Ordering> {
                <RelativePath as PartialOrd>::partial_cmp(self, other)
            }
        }

        impl<'a, 'b> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<cmp::Ordering> {
                <RelativePath as PartialOrd>::partial_cmp(self, other)
            }
        }
    }
}

impl_cmp!(RelativePathBuf, RelativePath);
impl_cmp!(RelativePathBuf, &'a RelativePath);
impl_cmp!(Cow<'a, RelativePath>, RelativePath);
impl_cmp!(Cow<'a, RelativePath>, &'b RelativePath);
impl_cmp!(Cow<'a, RelativePath>, RelativePathBuf);

macro_rules! impl_cmp_str {
    ($lhs:ty, $rhs: ty) => {
        impl<'a, 'b> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                <RelativePath as PartialEq>::eq(self, other.as_ref())
            }
        }

        impl<'a, 'b> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                <RelativePath as PartialEq>::eq(self.as_ref(), other)
            }
        }

        impl<'a, 'b> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<cmp::Ordering> {
                <RelativePath as PartialOrd>::partial_cmp(self, other.as_ref())
            }
        }

        impl<'a, 'b> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<cmp::Ordering> {
                <RelativePath as PartialOrd>::partial_cmp(self.as_ref(), other)
            }
        }
    }
}

impl_cmp_str!(RelativePathBuf, str);
impl_cmp_str!(RelativePathBuf, &'a str);
impl_cmp_str!(RelativePathBuf, String);
impl_cmp_str!(RelativePath, str);
impl_cmp_str!(RelativePath, &'a str);
impl_cmp_str!(RelativePath, String);
impl_cmp_str!(&'a RelativePath, str);
impl_cmp_str!(&'a RelativePath, String);

#[cfg(test)]
mod tests {
    use std::path::Path;
    use super::*;

    fn assert_components(components: &[&str], path: &RelativePath) {
        let result: Vec<_> = path.components().collect();
        assert_eq!(components, &result[..]);
    }

    fn rp(input: &str) -> &RelativePath {
        RelativePath::new(input)
    }

    #[test]
    fn test_join() {
        assert_components(&["foo", "bar", "baz"], &rp("foo/bar").join("baz///"));
        assert_components(
            &["foo", "bar", "baz"],
            &rp("hello/world").join("///foo/bar/baz"),
        );
        assert_components(&["foo", "bar", "baz"], &rp("").join("///foo/bar/baz"));
    }

    #[test]
    fn test_components_iterator() {
        assert_eq!(
            vec!["hello", "world"],
            rp("/hello///world//").components().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_to_path_buf() {
        let path = rp("/hello///world//");
        let path_buf = path.to_relative_of(Path::new("."));
        let expected = Path::new(".").join("hello").join("world");
        assert_eq!(expected, path_buf);
    }

    #[test]
    fn test_eq() {
        assert_eq!(rp("//foo///bar"), rp("/foo/bar"));
        assert_eq!(rp("foo///bar"), rp("foo/bar"));
        assert_eq!(rp("foo"), rp("foo"));
        assert_eq!(rp("foo"), rp("foo").to_relative_path_buf());
    }

    #[test]
    fn test_next_back() {
        let mut it = rp("baz/bar///foo").components();
        assert_eq!(Some("foo"), it.next_back());
        assert_eq!(Some("bar"), it.next_back());
        assert_eq!(Some("baz"), it.next_back());
        assert_eq!(None, it.next_back());
    }

    #[test]
    fn test_parent() {
        let path = rp("/baz//bar/foo//");
        assert_eq!(Some(rp("/baz/bar")), path.parent());
        assert_eq!(
            Some(rp("/baz")),
            path.parent().and_then(RelativePath::parent)
        );
        assert_eq!(
            None,
            path.parent().and_then(RelativePath::parent).and_then(
                RelativePath::parent,
            )
        );
    }

    #[test]
    fn test_relative_path_buf() {
        assert_eq!(
            rp("hello/world/."),
            rp("/hello///world//").to_owned().join(".")
        );
    }
}
