use super::{Entry, Key};
use core::hash::{BuildHasher, Hash};
use hashbrown::hash_table::Entry as TableEntry;
use hashbrown::{DefaultHashBuilder, HashTable};

pub trait Equivalent<K: ?Sized> {
	fn equivalent(&self, key: &K) -> bool;
}

impl<Q: ?Sized + Eq, K: ?Sized> Equivalent<K> for Q
where
	K: std::borrow::Borrow<Q>,
{
	fn equivalent(&self, key: &K) -> bool {
		self == key.borrow()
	}
}

fn equivalent_key<'a, Q>(entries: &'a [Entry], k: &'a Q) -> impl 'a + Fn(&Indexes) -> bool
where
	Q: ?Sized + Equivalent<Key>,
{
	move |indexes| k.equivalent(&entries[indexes.rep].key)
}

fn make_hasher() -> impl Fn(&Indexes) -> u64 {
	|indexes| indexes.hash
}

#[derive(Clone, Debug)]
pub struct Indexes {
	/// Index of the first entry with the considered key (the representative).
	rep: usize,

	/// Other indexes with this key.
	other: Vec<usize>,

	/// Cached hash of the key shared by every index in this slot. Used by
	/// `make_hasher` so table grow does not re-hash keys.
	hash: u64,
}

impl PartialEq for Indexes {
	fn eq(&self, other: &Self) -> bool {
		self.rep == other.rep && self.other == other.other
	}
}

impl Eq for Indexes {}

impl Indexes {
	fn new(rep: usize, hash: u64) -> Self {
		Self {
			rep,
			other: Vec::new(),
			hash,
		}
	}

	pub fn len(&self) -> usize {
		1 + self.other.len()
	}

	pub fn first(&self) -> usize {
		self.rep
	}

	pub fn is_redundant(&self) -> bool {
		!self.other.is_empty()
	}

	pub fn redundant(&self) -> Option<usize> {
		self.other.first().cloned()
	}

	pub fn redundants(&self) -> &[usize] {
		&self.other
	}

	fn insert(&mut self, mut index: usize) {
		if index != self.rep {
			if index < self.rep {
				core::mem::swap(&mut index, &mut self.rep);
			}

			if let Err(i) = self.other.binary_search(&index) {
				self.other.insert(i, index)
			}
		}
	}

	/// Removes the given index, unless it is the last remaining index.
	///
	/// Returns `true` if the index has been removed or not in the list,
	/// and `false` if it was the last index (and hence not removed).
	fn remove(&mut self, index: usize) -> bool {
		if self.rep == index {
			if self.other.is_empty() {
				false
			} else {
				self.rep = self.other.remove(0);
				true
			}
		} else {
			if let Ok(i) = self.other.binary_search(&index) {
				self.other.remove(i);
			}

			true
		}
	}

	/// Decreases all index greater than `index` by one.
	pub fn shift_down(&mut self, index: usize) {
		if self.rep > index {
			self.rep -= 1
		}

		for i in &mut self.other {
			if *i > index {
				*i -= 1
			}
		}
	}

	/// Increases all index greater than or equal to `index` by one.
	pub fn shift_up(&mut self, index: usize) {
		if self.rep >= index {
			self.rep += 1
		}

		for i in &mut self.other {
			if *i >= index {
				*i += 1
			}
		}
	}

	pub fn iter(&self) -> super::Indexes<'_> {
		super::Indexes::Some {
			first: Some(self.rep),
			other: self.other.iter(),
		}
	}
}

impl<'a> IntoIterator for &'a Indexes {
	type Item = usize;
	type IntoIter = super::Indexes<'a>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

#[derive(Clone)]
pub struct IndexMap<S = DefaultHashBuilder> {
	hash_builder: S,
	table: HashTable<Indexes>,
}

impl<S: Default> Default for IndexMap<S> {
	fn default() -> Self {
		Self {
			hash_builder: S::default(),
			table: HashTable::new(),
		}
	}
}

impl<S> IndexMap<S> {
	pub fn new() -> Self
	where
		S: Default,
	{
		Self::default()
	}

	pub fn with_capacity(capacity: usize) -> Self
	where
		S: Default,
	{
		Self {
			hash_builder: S::default(),
			table: HashTable::with_capacity(capacity),
		}
	}

	pub fn contains_duplicate_keys(&self) -> bool {
		self.table.iter().any(Indexes::is_redundant)
	}
}

impl<S: BuildHasher> IndexMap<S> {
	pub fn get<Q>(&self, entries: &[Entry], key: &Q) -> Option<&Indexes>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let hash = self.hash_builder.hash_one(key);
		self.table.find(hash, equivalent_key(entries, key))
	}

	/// Associates the given `key` to `index`.
	///
	/// Returns `true` if no index was already associated to the key.
	///
	/// Performs a single hash and a single probe regardless of whether
	/// the key already exists.
	pub fn insert(&mut self, entries: &[Entry], index: usize) -> bool {
		let key = &entries[index].key;
		let hash = self.hash_builder.hash_one(key);
		match self
			.table
			.entry(hash, equivalent_key(entries, key), make_hasher())
		{
			TableEntry::Occupied(mut occupied) => {
				occupied.get_mut().insert(index);
				false
			}
			TableEntry::Vacant(vacant) => {
				vacant.insert(Indexes::new(index, hash));
				true
			}
		}
	}

	/// Single-hash key probe. If the key already maps to an index, returns
	/// `Some(rep)` where `rep` is the first index for this key. Otherwise
	/// reserves a slot for `new_index` and returns `None`. When `None` is
	/// returned, the caller MUST push the corresponding entry to `entries`
	/// at position `new_index` before any other `IndexMap` operation.
	pub fn lookup_or_insert<Q>(
		&mut self,
		entries: &[Entry],
		new_index: usize,
		key: &Q,
	) -> Option<usize>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let hash = self.hash_builder.hash_one(key);
		match self
			.table
			.entry(hash, equivalent_key(entries, key), make_hasher())
		{
			TableEntry::Occupied(occupied) => Some(occupied.get().first()),
			TableEntry::Vacant(vacant) => {
				vacant.insert(Indexes::new(new_index, hash));
				None
			}
		}
	}

	/// Removes the association between the given key and index.
	pub fn remove(&mut self, entries: &[Entry], index: usize) {
		let key = &entries[index].key;
		let hash = self.hash_builder.hash_one(key);
		if let Ok(mut occupied) = self.table.find_entry(hash, equivalent_key(entries, key)) {
			if !occupied.get_mut().remove(index) {
				occupied.remove();
			}
		}
	}

	/// Decreases all index greater than `index` by one everywhere in the table.
	pub fn shift_down(&mut self, index: usize) {
		for indexes in self.table.iter_mut() {
			indexes.shift_down(index);
		}
	}

	/// Increases all index greater than or equal to `index` by one everywhere in the table.
	pub fn shift_up(&mut self, index: usize) {
		for indexes in self.table.iter_mut() {
			indexes.shift_up(index);
		}
	}

	pub fn clear(&mut self) {
		self.table.clear()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Value;

	#[test]
	fn insert() {
		let entries = [
			Entry::new("a".into(), Value::Null),
			Entry::new("b".into(), Value::Null),
			Entry::new("a".into(), Value::Null),
		];

		let mut indexes: IndexMap = IndexMap::default();
		indexes.insert(&entries, 2);
		indexes.insert(&entries, 1);
		indexes.insert(&entries, 0);

		let mut a = indexes.get(&entries, "a").unwrap().iter();
		let mut b = indexes.get(&entries, "b").unwrap().iter();

		assert_eq!(a.next(), Some(0));
		assert_eq!(a.next(), Some(2));
		assert_eq!(a.next(), None);
		assert_eq!(b.next(), Some(1));
		assert_eq!(b.next(), None);
		assert_eq!(indexes.get(&entries, "c"), None)
	}

	#[test]
	fn remove1() {
		let entries = [
			Entry::new("a".into(), Value::Null),
			Entry::new("b".into(), Value::Null),
			Entry::new("a".into(), Value::Null),
		];

		let mut indexes: IndexMap = IndexMap::default();
		indexes.insert(&entries, 2);
		indexes.insert(&entries, 1);
		indexes.insert(&entries, 0);

		indexes.remove(&entries, 1);
		indexes.remove(&entries, 0);

		let mut a = indexes.get(&entries, "a").unwrap().iter();

		assert_eq!(a.next(), Some(2));
		assert_eq!(a.next(), None);
		assert_eq!(indexes.get(&entries, "b"), None)
	}

	#[test]
	fn remove2() {
		let entries = [
			Entry::new("a".into(), Value::Null),
			Entry::new("b".into(), Value::Null),
			Entry::new("a".into(), Value::Null),
		];

		let mut indexes: IndexMap = IndexMap::default();
		indexes.insert(&entries, 2);
		indexes.insert(&entries, 1);
		indexes.insert(&entries, 0);

		indexes.remove(&entries, 0);
		indexes.remove(&entries, 1);
		indexes.remove(&entries, 2);

		assert_eq!(indexes.get(&entries, "a"), None);
		assert_eq!(indexes.get(&entries, "b"), None)
	}
}
