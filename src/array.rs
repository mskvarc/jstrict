//! JSON arrays.
use crate::{CodeMap, Value, code_map::Mapped};
use core::slice;

/// JSON array.
///
/// A plain `Vec<Value>`: arrays carry no extra state beyond their items.
pub type Array = Vec<Value>;

/// Trait for JSON array types like `Vec<Value>` and `[Value]`.
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

impl JsonArray for Vec<Value> {
	fn iter_mapped<'m>(&self, code_map: &'m CodeMap, offset: usize) -> IterMapped<'_, 'm> {
		IterMapped {
			items: self.iter(),
			code_map,
			offset: offset + 1,
		}
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
