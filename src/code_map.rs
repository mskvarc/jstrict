//! Mapping between JSON value fragments and the source text.
//!
//! Parsing a value with the [`Parse`](crate::Parse) trait returns a
//! [`CodeMap`] alongside it. The code map is a flat list of [`Entry`], one per
//! fragment, in the same depth-first order as [`Value::traverse`]: entry `i`
//! describes the `i`-th fragment. This lets a caller report an error against
//! the exact byte range of the offending value without keeping the source
//! layout inside the value tree.
//!
//! [`Value::traverse`]: crate::Value::traverse
use core::fmt;
use std::error::Error as StdError;
use std::iter::Enumerate;
use std::slice;
use std::vec;
use std::{borrow::Borrow, ops::Deref};

use locspan::Span;

/// Code-map.
///
/// Maps each fragment of a parsed JSON value to its span in the source text.
/// See the [module documentation](self).
#[derive(Debug, Default, Clone)]
pub struct CodeMap(Vec<Entry>);

impl CodeMap {
	/// Returns the entries of this code map as a slice, in fragment order.
	#[must_use]
	pub fn as_slice(&self) -> &[Entry] {
		&self.0
	}

	pub(crate) fn reserve(&mut self, position: usize) -> usize {
		let i = self.0.len();
		self.0.push(Entry {
			span: Span::new(position, position),
			volume: 0,
		});
		i
	}

	pub(crate) fn get_mut(&mut self, i: usize) -> Option<&mut Entry> {
		self.0.get_mut(i)
	}

	/// Iterates over the entries of this code map, paired with their fragment
	/// index.
	pub fn iter(&self) -> Iter<'_> {
		self.0.iter().enumerate()
	}
}

impl Deref for CodeMap {
	type Target = [Entry];

	fn deref(&self) -> &Self::Target {
		self.as_slice()
	}
}

impl AsRef<[Entry]> for CodeMap {
	fn as_ref(&self) -> &[Entry] {
		self.as_slice()
	}
}

impl Borrow<[Entry]> for CodeMap {
	fn borrow(&self) -> &[Entry] {
		self.as_slice()
	}
}

/// Borrowing iterator over the entries of a [`CodeMap`], with their index.
pub type Iter<'a> = Enumerate<slice::Iter<'a, Entry>>;

/// Consuming iterator over the entries of a [`CodeMap`], with their index.
pub type IntoIter = Enumerate<vec::IntoIter<Entry>>;

impl<'a> IntoIterator for &'a CodeMap {
	type IntoIter = Iter<'a>;
	type Item = (usize, &'a Entry);

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

impl IntoIterator for CodeMap {
	type IntoIter = IntoIter;
	type Item = (usize, Entry);

	fn into_iter(self) -> Self::IntoIter {
		self.0.into_iter().enumerate()
	}
}

/// Code-map entry.
///
/// Provides code-mapping metadata about a fragment of JSON value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entry {
	/// Byte span of the fragment in the original source code.
	pub span: Span,

	/// Number of sub-fragment (including the fragment itself).
	pub volume: usize,
}

impl Entry {
	/// Creates a new code map entry from a source `span` and a fragment
	/// `volume`.
	#[must_use]
	pub const fn new(span: Span, volume: usize) -> Self {
		Self { span, volume }
	}
}

/// A value paired with the code map offset of the fragment it came from.
///
/// Used by the mapped accessors of [`Object`](crate::Object) and
/// [`JsonArray`](crate::array::JsonArray), and as the error type of the
/// [`TryFromJson`](crate::TryFromJson) implementations, so a failure can be
/// traced back to a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mapped<T> {
	/// Offset of the fragment in the code map.
	pub offset: usize,

	/// The mapped value.
	pub value: T,
}

impl<T> Mapped<T> {
	/// Pairs `value` with the code map `offset` of its fragment.
	pub const fn new(offset: usize, value: T) -> Self {
		Self { offset, value }
	}
}

impl<T: fmt::Display> fmt::Display for Mapped<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.value.fmt(f)
	}
}

impl<T: 'static + StdError> StdError for Mapped<T> {
	fn source(&self) -> Option<&(dyn StdError + 'static)> {
		Some(&self.value)
	}
}

#[cfg(test)]
mod tests {
	use super::Entry;
	use crate::{Parse, Value};
	use locspan::Span;

	#[test]
	fn code_map_t1() {
		let (value, code_map) = Value::parse_str(r#"{ "a": 0, "b": [1, 2] }"#).unwrap();
		let expected = [
			Entry::new(Span::new(0, 23), 9),  // { "a": 0, "b": [1, 2] }
			Entry::new(Span::new(2, 8), 3),   // "a": 0
			Entry::new(Span::new(2, 5), 1),   // "a"
			Entry::new(Span::new(7, 8), 1),   // 0
			Entry::new(Span::new(10, 21), 5), // "b": [1, 2]
			Entry::new(Span::new(10, 13), 1), // "b"
			Entry::new(Span::new(15, 21), 3), // [1, 2]
			Entry::new(Span::new(16, 17), 1), // 1
			Entry::new(Span::new(19, 20), 1), // 2
		];

		assert_eq!(code_map.len(), expected.len());
		assert_eq!(value.traverse().count(), expected.len());
		for (i, entry) in code_map {
			assert_eq!(entry, expected[i]);
		}
	}

	#[test]
	fn code_map_t2() {
		let (value, code_map) =
			Value::parse_str(r#"{ "a": 0, "b": { "c": 1, "d": [2, 3] }, "e": [4, [5, 6]] }"#)
				.unwrap();
		let expected = [
			Entry::new(Span::new(0, 58), 22), // { "a": 0, "b": { "c": 1, "d": [2, 3] }, "e": [4, [5, 6]] }
			Entry::new(Span::new(2, 8), 3),   // "a": 0
			Entry::new(Span::new(2, 5), 1),   // "a"
			Entry::new(Span::new(7, 8), 1),   // 0
			Entry::new(Span::new(10, 38), 11), // "b": { "c": 1, "d": [2, 3] }
			Entry::new(Span::new(10, 13), 1), // "b"
			Entry::new(Span::new(15, 38), 9), // { "c": 1, "d": [2, 3] }
			Entry::new(Span::new(17, 23), 3), // "c": 1
			Entry::new(Span::new(17, 20), 1), // "c"
			Entry::new(Span::new(22, 23), 1), // 1
			Entry::new(Span::new(25, 36), 5), // "d": [2, 3]
			Entry::new(Span::new(25, 28), 1), // "d"
			Entry::new(Span::new(30, 36), 3), // [2, 3]
			Entry::new(Span::new(31, 32), 1), // 2
			Entry::new(Span::new(34, 35), 1), // 3
			Entry::new(Span::new(40, 56), 7), // "e": [4, [5, 6]]
			Entry::new(Span::new(40, 43), 1), // "e"
			Entry::new(Span::new(45, 56), 5), // [4, [5, 6]]
			Entry::new(Span::new(46, 47), 1), // 4
			Entry::new(Span::new(49, 55), 3), // [5, 6]
			Entry::new(Span::new(50, 51), 1), // 5
			Entry::new(Span::new(53, 54), 1), // 6
		];

		assert_eq!(code_map.len(), expected.len());
		assert_eq!(value.traverse().count(), expected.len());
		for (i, entry) in code_map {
			assert_eq!(entry, expected[i]);
		}
	}
}
