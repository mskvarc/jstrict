//! JSON arrays.
use crate::{CodeMap, Value, code_map::Mapped, dismantle};
use core::slice;
use std::{
	fmt, mem,
	ops::{Deref, DerefMut},
	vec,
};

/// JSON array.
///
/// Owns its items like the `Vec<Value>` it wraps, and dereferences to it, so
/// the whole `Vec` API is available. What it adds is its [`Drop`]: dropping an
/// array never recurses, whatever the nesting depth of its items, because
/// every nested container is dismantled through one heap worklist before it
/// is dropped.
///
/// Because of that `Drop`, an `Array` cannot be destructured; take its items
/// with [`Array::into_vec`] or the owning [`IntoIterator`].
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Array(Vec<Value>);

impl Array {
	/// Creates a new empty array.
	#[must_use]
	pub const fn new() -> Self {
		Self(Vec::new())
	}

	/// Creates a new empty array with room for `capacity` items.
	#[must_use]
	pub fn with_capacity(capacity: usize) -> Self {
		Self(Vec::with_capacity(capacity))
	}

	/// Consumes this array, returning its items.
	#[must_use]
	pub fn into_vec(mut self) -> Vec<Value> {
		mem::take(&mut self.0)
	}

	/// Returns the number of items in this array.
	#[must_use]
	pub const fn len(&self) -> usize {
		self.0.len()
	}

	/// Checks if this array has no items.
	#[must_use]
	pub const fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	/// Returns the items of this array as a slice.
	#[must_use]
	pub fn as_slice(&self) -> &[Value] {
		&self.0
	}
}

impl Drop for Array {
	/// Drops the items without recursing: every nested container is moved
	/// onto a heap worklist and emptied there before it drops, so each drop
	/// only ever sees empty children. An array holding no nested container
	/// allocates nothing.
	fn drop(&mut self) {
		let mut pending: Vec<Value> = self
			.0
			.drain(..)
			.filter(Value::is_nested_container)
			.collect();
		dismantle::dismantle(&mut pending);
	}
}

impl fmt::Debug for Array {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_list().entries(self.0.iter()).finish()
	}
}

impl Deref for Array {
	type Target = Vec<Value>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for Array {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl AsRef<[Value]> for Array {
	fn as_ref(&self) -> &[Value] {
		&self.0
	}
}

impl From<Vec<Value>> for Array {
	fn from(items: Vec<Value>) -> Self {
		Self(items)
	}
}

impl From<Array> for Vec<Value> {
	fn from(array: Array) -> Self {
		array.into_vec()
	}
}

impl FromIterator<Value> for Array {
	fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> Self {
		Self(iter.into_iter().collect())
	}
}

impl Extend<Value> for Array {
	fn extend<I: IntoIterator<Item = Value>>(&mut self, iter: I) {
		self.0.extend(iter);
	}
}

impl IntoIterator for Array {
	type Item = Value;
	type IntoIter = vec::IntoIter<Value>;

	fn into_iter(self) -> Self::IntoIter {
		self.into_vec().into_iter()
	}
}

impl<'a> IntoIterator for &'a Array {
	type Item = &'a Value;
	type IntoIter = slice::Iter<'a, Value>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.iter()
	}
}

impl<'a> IntoIterator for &'a mut Array {
	type Item = &'a mut Value;
	type IntoIter = slice::IterMut<'a, Value>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.iter_mut()
	}
}

/// Trait for JSON array types like [`Array`] and `[Value]`.
pub trait JsonArray {
	/// Iterates over the items of this array, pairing each with its offset in
	/// `code_map`.
	///
	/// `offset` is the code map offset of the array itself.
	fn iter_mapped<'m>(&self, code_map: &'m CodeMap, offset: usize) -> IterMapped<'_, 'm>;
}

impl JsonArray for [Value] {
	fn iter_mapped<'m>(&self, code_map: &'m CodeMap, offset: usize) -> IterMapped<'_, 'm> {
		IterMapped {
			items: self.iter(),
			code_map,
			offset: offset + 1,
		}
	}
}

impl JsonArray for Array {
	fn iter_mapped<'m>(&self, code_map: &'m CodeMap, offset: usize) -> IterMapped<'_, 'm> {
		self.as_slice().iter_mapped(code_map, offset)
	}
}

impl JsonArray for Vec<Value> {
	fn iter_mapped<'m>(&self, code_map: &'m CodeMap, offset: usize) -> IterMapped<'_, 'm> {
		self.as_slice().iter_mapped(code_map, offset)
	}
}

/// Iterator over the items of an array, with their code map offset.
///
/// Returned by [`JsonArray::iter_mapped`].
pub struct IterMapped<'a, 'm> {
	items: slice::Iter<'a, Value>,
	code_map: &'m CodeMap,
	offset: usize,
}

impl<'a> Iterator for IterMapped<'a, '_> {
	type Item = Mapped<&'a Value>;

	fn next(&mut self) -> Option<Self::Item> {
		self.items.next().map(|item| {
			let offset = self.offset;
			self.offset += self
				.code_map
				.get(self.offset)
				.map_or(0, |entry| entry.volume);
			Mapped::new(offset, item)
		})
	}
}
