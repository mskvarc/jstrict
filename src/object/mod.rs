//! JSON objects.
//!
//! A JSON [`Object`] is a *list* of key-value [`Entry`] in definition order,
//! not a map: duplicate keys are preserved exactly as they appear in the
//! source. An internal index still gives `O(1)` average lookup by key, and
//! the `*_unique` accessors report duplicates as an error instead of silently
//! picking one.
use crate::code_map::Mapped;
use crate::{CodeMap, FragmentRef, UnorderedEq, UnorderedPartialEq, Value, dismantle::dismantle};
use compact_str::CompactString;
use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use core::{fmt, mem, slice};
use std::error::Error as StdError;
use std::vec::{Drain, IntoIter};

mod index_map;

pub use index_map::Equivalent;
use index_map::IndexMap;

/// Object key.
///
/// Keys short enough to fit inline are stored on the stack; longer ones spill
/// to the heap. Same representation as [`crate::String`].
pub type Key = CompactString;

/// Object entry.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Entry<K = Key, V = Value> {
	/// Entry key.
	pub key: K,

	/// Entry value.
	pub value: V,
}

impl<K, V> Entry<K, V> {
	/// Creates a new entry from a key and a value.
	pub const fn new(key: K, value: V) -> Self {
		Self { key, value }
	}

	/// Returns a reference to the key.
	pub const fn as_key(&self) -> &K {
		&self.key
	}

	/// Returns a reference to the value.
	pub const fn as_value(&self) -> &V {
		&self.value
	}

	/// Consumes this entry, returning its key.
	pub fn into_key(self) -> K {
		self.key
	}

	/// Consumes this entry, returning its value.
	pub fn into_value(self) -> V {
		self.value
	}

	/// Returns references to the key and the value.
	pub const fn as_pair(&self) -> (&K, &V) {
		(&self.key, &self.value)
	}

	/// Consumes this entry, returning its key and value.
	pub fn into_pair(self) -> (K, V) {
		(self.key, self.value)
	}

	/// Borrows this entry as an `Entry<&K, &V>`.
	pub const fn as_ref(&self) -> Entry<&K, &V> {
		Entry::new(&self.key, &self.value)
	}

	/// Attaches code map offsets to the key and the value of this entry.
	pub fn into_mapped(
		self,
		key_offset: usize,
		value_offset: usize,
	) -> Entry<Mapped<K>, Mapped<V>> {
		Entry::new(
			Mapped::new(key_offset, self.key),
			Mapped::new(value_offset, self.value),
		)
	}
}

impl Entry {
	/// Returns the fragment at the given `index` in traversal order, where
	/// index `0` is the entry itself and index `1` its key.
	///
	/// On failure, returns the number of fragments left to skip.
	/// # Errors
	///
	/// Returns the number of fragments left to skip when `index` does not resolve
	/// to a fragment within this object.
	pub fn get_fragment(&self, index: usize) -> Result<FragmentRef<'_>, usize> {
		match index {
			0 => Ok(FragmentRef::Entry(self)),
			1 => Ok(FragmentRef::Key(&self.key)),
			_ => self.value.get_fragment(index - 2),
		}
	}
}

/// Borrowed object entry, with the code map offset of the entry, its key and
/// its value.
pub type MappedEntry<'a> = Mapped<Entry<Mapped<&'a Key>, Mapped<&'a Value>>>;

/// A [`MappedEntry`] together with its position in the object.
pub type IndexedMappedEntry<'a> = (usize, MappedEntry<'a>);

/// A mapped entry value together with the entry's position in the object.
pub type IndexedMappedValue<'a> = (usize, Mapped<&'a Value>);

/// Object.
#[derive(Clone)]
pub struct Object {
	/// The entries of the object, in order.
	entries: Vec<Entry>,

	/// Maps each key to an entry index.
	indexes: IndexMap,
}

impl Default for Object {
	fn default() -> Self {
		Self {
			entries: Vec::new(),
			indexes: IndexMap::new(),
		}
	}
}

impl Object {
	/// Creates a new empty object.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a new empty object with room for `capacity` entries.
	#[must_use]
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			entries: Vec::with_capacity(capacity),
			indexes: IndexMap::with_capacity(capacity),
		}
	}

	/// Builds an object from a list of entries, keeping their order and any
	/// duplicate keys.
	#[must_use]
	pub fn from_vec(entries: Vec<Entry>) -> Self {
		let mut indexes = IndexMap::with_capacity(entries.len());
		for i in 0..entries.len() {
			indexes.insert(&entries, i);
		}

		Self { entries, indexes }
	}

	/// Returns the number of entries this object can hold without
	/// reallocating.
	#[must_use]
	pub const fn capacity(&self) -> usize {
		self.entries.capacity()
	}

	/// Returns the number of entries in this object, duplicates included.
	#[must_use]
	pub const fn len(&self) -> usize {
		self.entries.len()
	}

	/// Checks if this object has no entries.
	#[must_use]
	pub const fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	/// Whether two entries of this object share a key.
	///
	/// Runs in `O(n)` over the key index; the object's descendants are not
	/// inspected.
	#[must_use]
	pub fn contains_duplicate_keys(&self) -> bool {
		self.indexes.contains_duplicate_keys()
	}

	/// Removes every entry, leaving the object empty, and yields them in
	/// order.
	pub(crate) fn drain_entries(&mut self) -> Drain<'_, Entry> {
		self.indexes.clear();
		self.entries.drain(..)
	}

	/// Returns the fragment at the given `index`, relative to the *contents*
	/// of this object: index `0` is the first entry.
	///
	/// On failure, returns the number of fragments left to skip.
	/// # Errors
	///
	/// Returns the number of fragments left to skip when `index` does not resolve
	/// to a fragment within this object.
	pub fn get_fragment(&self, mut index: usize) -> Result<FragmentRef<'_>, usize> {
		for e in &self.entries {
			match e.get_fragment(index) {
				Ok(value) => return Ok(value),
				Err(i) => index = i,
			}
		}

		Err(index)
	}

	/// Returns the entries of this object as a slice, in definition order.
	#[must_use]
	pub fn entries(&self) -> &[Entry] {
		&self.entries
	}

	/// Iterates over the entries of this object, in definition order.
	pub fn iter(&self) -> Iter<'_> {
		self.entries.iter()
	}

	/// Iterates over the entries of this object, yielding mutable values.
	///
	/// Keys are not mutable: changing one would invalidate the key index.
	pub fn iter_mut(&mut self) -> IterMut<'_> {
		IterMut(self.entries.iter_mut())
	}

	/// Iterates over the entries of this object, pairing each entry, key and
	/// value with its offset in `code_map`.
	///
	/// `offset` is the code map offset of the object itself.
	#[must_use]
	pub fn iter_mapped<'m>(&self, code_map: &'m CodeMap, offset: usize) -> IterMapped<'_, 'm> {
		IterMapped {
			entries: self.entries.iter(),
			code_map,
			offset: offset + 1,
		}
	}

	/// Checks if this object contains the given key.
	///
	/// Runs in `O(1)` (average).
	pub fn contains_key<Q>(&self, key: &Q) -> bool
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		self.indexes.get(&self.entries, key).is_some()
	}

	/// Returns an iterator over the values matching the given key.
	///
	/// Runs in `O(1)` (average).
	pub fn get<Q>(&self, key: &Q) -> Values<'_>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let indexes = self
			.indexes
			.get(&self.entries, key)
			.map(IntoIterator::into_iter)
			.unwrap_or_default();
		Values {
			indexes,
			object: self,
		}
	}

	/// Returns an iterator over the values matching the given key.
	///
	/// Runs in `O(1)` (average).
	pub fn get_mut<Q>(&mut self, key: &Q) -> ValuesMut<'_>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let indexes = self
			.indexes
			.get(&self.entries, key)
			.map(IntoIterator::into_iter)
			.unwrap_or_default();
		ValuesMut {
			indexes,
			entries: &mut self.entries,
		}
	}

	/// Returns the unique entry value matching the given key.
	///
	/// Returns an error if multiple entries match the key.
	///
	/// Runs in `O(1)` (average).
	/// # Errors
	///
	/// Returns [`Duplicate`] if more than one entry matches `key`.
	pub fn get_unique<Q>(&self, key: &Q) -> Result<Option<&Value>, Duplicate<&Entry>>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let mut entries = self.get_entries(key);

		match entries.next() {
			Some(entry) => match entries.next() {
				Some(duplicate) => Err(Duplicate(entry, duplicate)),
				None => Ok(Some(&entry.value)),
			},
			None => Ok(None),
		}
	}

	/// Returns the unique entry value matching the given key.
	///
	/// Returns an error if multiple entries match the key.
	///
	/// Runs in `O(1)` (average).
	/// # Errors
	///
	/// Returns [`Duplicate`] if more than one entry matches `key`.
	pub fn get_unique_mut<Q>(&mut self, key: &Q) -> Result<Option<&mut Value>, Duplicate<&Entry>>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let index = {
			let mut entries = self.get_entries_with_index(key);
			match entries.next() {
				Some((i, _)) => match entries.next() {
					Some((j, _)) => Err(Duplicate(i, j)),
					None => Ok(Some(i)),
				},
				None => Ok(None),
			}
		};

		match index {
			Ok(Some(i)) => Ok(Some(&mut self.entries[i].value)),
			Ok(None) => Ok(None),
			Err(Duplicate(i, j)) => Err(Duplicate(&self.entries[i], &self.entries[j])),
		}
	}

	/// Returns an iterator over the entries matching the given key.
	///
	/// Runs in `O(1)` (average).
	pub fn get_entries<Q>(&self, key: &Q) -> Entries<'_>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let indexes = self
			.indexes
			.get(&self.entries, key)
			.map(IntoIterator::into_iter)
			.unwrap_or_default();
		Entries {
			indexes,
			object: self,
		}
	}

	/// Returns the unique entry matching the given key.
	///
	/// Returns an error if multiple entries match the key.
	///
	/// Runs in `O(1)` (average).
	/// # Errors
	///
	/// Returns [`Duplicate`] if more than one entry matches `key`.
	pub fn get_unique_entry<Q>(&self, key: &Q) -> Result<Option<&Entry>, Duplicate<&Entry>>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let mut entries = self.get_entries(key);

		match entries.next() {
			Some(entry) => match entries.next() {
				Some(duplicate) => Err(Duplicate(entry, duplicate)),
				None => Ok(Some(entry)),
			},
			None => Ok(None),
		}
	}

	/// Returns an iterator over the values matching the given key.
	///
	/// Runs in `O(1)` (average).
	pub fn get_with_index<Q>(&self, key: &Q) -> ValuesWithIndex<'_>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let indexes = self
			.indexes
			.get(&self.entries, key)
			.map(IntoIterator::into_iter)
			.unwrap_or_default();
		ValuesWithIndex {
			indexes,
			object: self,
		}
	}

	/// Returns an iterator over the entries matching the given key.
	///
	/// Runs in `O(1)` (average).
	pub fn get_entries_with_index<Q>(&self, key: &Q) -> EntriesWithIndex<'_>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let indexes = self
			.indexes
			.get(&self.entries, key)
			.map(IntoIterator::into_iter)
			.unwrap_or_default();
		EntriesWithIndex {
			indexes,
			object: self,
		}
	}

	/// Returns the (first) value associated to `key`, or insert a `key`-`value`
	/// entry where `value` is returned by the given function `f`.
	pub fn get_or_insert_with<Q>(&mut self, key: &Q, f: impl FnOnce() -> Value) -> &Value
	where
		Q: ?Sized + Hash + Equivalent<Key> + ToOwned,
		Q::Owned: Into<Key>,
	{
		let index = if let Some(index) = self.index_of(key) {
			index
		} else {
			let index = self.entries.len();
			self.push(key.to_owned().into(), f());
			index
		};

		&self.entries[index].value
	}

	/// Returns a mutable reference to the (first) value associated to `key`, or
	/// insert a `key`-`value` entry where `value` is returned by the given
	/// function `f`.
	pub fn get_mut_or_insert_with<Q>(&mut self, key: &Q, f: impl FnOnce() -> Value) -> &mut Value
	where
		Q: ?Sized + Hash + Equivalent<Key> + ToOwned,
		Q::Owned: Into<Key>,
	{
		let index = if let Some(index) = self.index_of(key) {
			index
		} else {
			let index = self.entries.len();
			self.push(key.to_owned().into(), f());
			index
		};

		&mut self.entries[index].value
	}

	/// Returns the position of the first entry matching `key`.
	///
	/// Runs in `O(1)` (average).
	pub fn index_of<Q>(&self, key: &Q) -> Option<usize>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		self.indexes
			.get(&self.entries, key)
			.map(index_map::Indexes::first)
	}

	/// Returns the position of the *second* entry matching `key`, if the key
	/// is duplicated.
	///
	/// Runs in `O(1)` (average).
	pub fn redundant_index_of<Q>(&self, key: &Q) -> Option<usize>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		self.indexes
			.get(&self.entries, key)
			.and_then(index_map::Indexes::redundant)
	}

	/// Iterates over the positions of every entry matching `key`.
	///
	/// Runs in `O(1)` (average).
	pub fn indexes_of<Q>(&self, key: &Q) -> Indexes<'_>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		self.indexes
			.get(&self.entries, key)
			.map(IntoIterator::into_iter)
			.unwrap_or_default()
	}

	/// Returns an iterator over the mapped entries matching the given key.
	///
	/// Runs in `O(n)` (average). `O(1)` to find the entry, `O(n)` to compute
	/// the entry fragment offset.
	pub fn get_mapped_entries<'m, Q>(
		&self,
		code_map: &'m CodeMap,
		offset: usize,
		key: &Q,
	) -> MappedEntries<'_, 'm>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let indexes = self
			.indexes
			.get(&self.entries, key)
			.map(IntoIterator::into_iter)
			.unwrap_or_default();
		MappedEntries {
			indexes,
			object: self,
			code_map,
			offset: offset + 1,
			last_index: 0,
		}
	}

	/// Returns the unique mapped entry matching the given key.
	///
	/// Runs in `O(n)` (average). `O(1)` to find the entry, `O(n)` to compute
	/// the entry fragment offset.
	/// # Errors
	///
	/// Returns [`Duplicate`] if more than one entry matches `key`.
	pub fn get_unique_mapped_entry<Q>(
		&self,
		code_map: &CodeMap,
		offset: usize,
		key: &Q,
	) -> Result<Option<MappedEntry<'_>>, Duplicate<MappedEntry<'_>>>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let mut entries = self.get_mapped_entries(code_map, offset, key);

		match entries.next() {
			Some(entry) => match entries.next() {
				Some(duplicate) => Err(Duplicate(entry, duplicate)),
				None => Ok(Some(entry)),
			},
			None => Ok(None),
		}
	}

	/// Returns an iterator over the mapped entries matching the given key, with
	/// their index.
	///
	/// Runs in `O(n)` (average). `O(1)` to find the entry, `O(n)` to compute
	/// the entry fragment offset.
	pub fn get_mapped_entries_with_index<'m, Q>(
		&self,
		code_map: &'m CodeMap,
		offset: usize,
		key: &Q,
	) -> MappedEntriesWithIndex<'_, 'm>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let indexes = self
			.indexes
			.get(&self.entries, key)
			.map(IntoIterator::into_iter)
			.unwrap_or_default();
		MappedEntriesWithIndex {
			indexes,
			object: self,
			code_map,
			offset: offset + 1,
			last_index: 0,
		}
	}

	/// Returns the unique mapped entry matching the given key, with its index.
	///
	/// Runs in `O(n)` (average). `O(1)` to find the entry, `O(n)` to compute
	/// the entry fragment offset.
	/// # Errors
	///
	/// Returns [`Duplicate`] if more than one entry matches `key`.
	pub fn get_unique_mapped_entry_with_index<Q>(
		&self,
		code_map: &CodeMap,
		offset: usize,
		key: &Q,
	) -> Result<Option<IndexedMappedEntry<'_>>, Duplicate<IndexedMappedEntry<'_>>>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let mut entries = self.get_mapped_entries_with_index(code_map, offset, key);

		match entries.next() {
			Some(entry) => match entries.next() {
				Some(duplicate) => Err(Duplicate(entry, duplicate)),
				None => Ok(Some(entry)),
			},
			None => Ok(None),
		}
	}

	/// Returns an iterator over the mapped values matching the given key.
	///
	/// Runs in `O(n)` (average). `O(1)` to find the entry, `O(n)` to compute
	/// the entry fragment offset.
	pub fn get_mapped<'m, Q>(
		&self,
		code_map: &'m CodeMap,
		offset: usize,
		key: &Q,
	) -> MappedValues<'_, 'm>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let indexes = self
			.indexes
			.get(&self.entries, key)
			.map(IntoIterator::into_iter)
			.unwrap_or_default();
		MappedValues {
			indexes,
			object: self,
			code_map,
			offset: offset + 1,
			last_index: 0,
		}
	}

	/// Returns the unique mapped values matching the given key.
	///
	/// Runs in `O(n)` (average). `O(1)` to find the entry, `O(n)` to compute
	/// the entry fragment offset.
	/// # Errors
	///
	/// Returns [`Duplicate`] if more than one entry matches `key`.
	pub fn get_unique_mapped<Q>(
		&self,
		code_map: &CodeMap,
		offset: usize,
		key: &Q,
	) -> Result<Option<Mapped<&Value>>, Duplicate<Mapped<&Value>>>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let mut entries = self.get_mapped(code_map, offset, key);

		match entries.next() {
			Some(entry) => match entries.next() {
				Some(duplicate) => Err(Duplicate(entry, duplicate)),
				None => Ok(Some(entry)),
			},
			None => Ok(None),
		}
	}

	/// Returns an iterator over the mapped values matching the given key, with
	/// their index.
	///
	/// Runs in `O(n)` (average). `O(1)` to find the entry, `O(n)` to compute
	/// the entry fragment offset.
	pub fn get_mapped_with_index<'m, Q>(
		&self,
		code_map: &'m CodeMap,
		offset: usize,
		key: &Q,
	) -> MappedValuesWithIndex<'_, 'm>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let indexes = self
			.indexes
			.get(&self.entries, key)
			.map(IntoIterator::into_iter)
			.unwrap_or_default();
		MappedValuesWithIndex {
			indexes,
			object: self,
			code_map,
			offset: offset + 1,
			last_index: 0,
		}
	}

	/// Returns the unique mapped values matching the given key, with its index.
	///
	/// Runs in `O(n)` (average). `O(1)` to find the entry, `O(n)` to compute
	/// the entry fragment offset.
	/// # Errors
	///
	/// Returns [`Duplicate`] if more than one entry matches `key`.
	pub fn get_unique_mapped_with_index<Q>(
		&self,
		code_map: &CodeMap,
		offset: usize,
		key: &Q,
	) -> Result<Option<IndexedMappedValue<'_>>, Duplicate<IndexedMappedValue<'_>>>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let mut entries = self.get_mapped_with_index(code_map, offset, key);

		match entries.next() {
			Some(entry) => match entries.next() {
				Some(duplicate) => Err(Duplicate(entry, duplicate)),
				None => Ok(Some(entry)),
			},
			None => Ok(None),
		}
	}

	/// Returns the first entry of this object.
	#[must_use]
	pub fn first(&self) -> Option<&Entry> {
		self.entries.first()
	}

	/// Returns the last entry of this object.
	#[must_use]
	pub fn last(&self) -> Option<&Entry> {
		self.entries.last()
	}

	/// Push the given key-value pair to the end of the object.
	///
	/// Returns `true` if the key was not already present in the object,
	/// and `false` otherwise.
	/// Any previous entry matching the key is **not** overridden: duplicates
	/// are preserved, in order.
	///
	/// Runs in `O(1)`.
	pub fn push(&mut self, key: Key, value: Value) -> bool {
		self.push_entry(Entry::new(key, value))
	}

	/// Push the given entry to the end of the object.
	///
	/// Returns `true` if its key was not already present in the object.
	///
	/// Runs in `O(1)`.
	pub fn push_entry(&mut self, entry: Entry) -> bool {
		let index = self.entries.len();
		self.entries.push(entry);
		self.indexes.insert(&self.entries, index)
	}

	/// Push the given key-value pair to the top of the object.
	///
	/// Returns `true` if the key was not already present in the object,
	/// and `false` otherwise.
	/// Any previous entry matching the key is **not** overridden: duplicates
	/// are preserved, in order.
	///
	/// Runs in `O(n)`.
	pub fn push_front(&mut self, key: Key, value: Value) -> bool {
		self.push_entry_front(Entry::new(key, value))
	}

	/// Push the given entry to the top of the object.
	///
	/// Returns `true` if its key was not already present in the object.
	///
	/// Runs in `O(n)`.
	pub fn push_entry_front(&mut self, entry: Entry) -> bool {
		self.entries.insert(0, entry);
		self.indexes.shift_up(0);
		self.indexes.insert(&self.entries, 0)
	}

	/// Removes the entry at the given index.
	pub fn remove_at(&mut self, index: usize) -> Option<Entry> {
		if index < self.entries.len() {
			self.indexes.remove(&self.entries, index);
			self.indexes.shift_down(index);
			Some(self.entries.remove(index))
		} else {
			None
		}
	}

	/// Inserts the given key-value pair.
	///
	/// If one or more entries are already matching the given key,
	/// all of them are removed and returned in the resulting iterator.
	/// Otherwise, `None` is returned.
	pub fn insert(&mut self, key: Key, value: Value) -> Option<RemovedByInsertion<'_>> {
		let new_index = self.entries.len();
		if let Some(index) = self
			.indexes
			.lookup_or_insert(&self.entries, new_index, &key)
		{
			let mut entry = Entry::new(key, value);
			mem::swap(&mut entry, &mut self.entries[index]);
			Some(RemovedByInsertion {
				index,
				first: Some(entry),
				object: self,
			})
		} else {
			self.entries.push(Entry::new(key, value));
			None
		}
	}

	/// Inserts the given key-value pair on top of the object.
	///
	/// If one or more entries are already matching the given key,
	/// all of them are removed and returned in the resulting iterator.
	pub fn insert_front(&mut self, key: Key, value: Value) -> RemovedByInsertFront<'_> {
		if let Some(first) = self.entries.first_mut()
			&& first.key == key
		{
			let first = mem::replace(first, Entry::new(key, value));
			return RemovedByInsertFront {
				first: Some(first),
				object: self,
			};
		}

		self.push_front(key, value);
		RemovedByInsertFront {
			first: None,
			object: self,
		}
	}

	/// Remove all entries associated to the given key.
	///
	/// Runs in `O(n)` time (average).
	pub const fn remove<'q, Q>(&mut self, key: &'q Q) -> RemovedEntries<'_, 'q, Q>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		RemovedEntries { key, object: self }
	}

	/// Remove the unique entry associated to the given key.
	///
	/// Returns an error if multiple entries match the key.
	///
	/// Runs in `O(n)` time (average).
	#[allow(
		clippy::result_large_err,
		reason = "the duplicate-key error carries the conflicting Entry by value; boxing it would pessimise the common Ok path"
	)]
	/// # Errors
	///
	/// Returns [`Duplicate`] if more than one entry matches `key`.
	pub fn remove_unique<Q>(&mut self, key: &Q) -> Result<Option<Entry>, Duplicate<Entry>>
	where
		Q: ?Sized + Hash + Equivalent<Key>,
	{
		let mut entries = self.remove(key);

		match entries.next() {
			Some(entry) => match entries.next() {
				Some(duplicate) => Err(Duplicate(entry, duplicate)),
				None => Ok(Some(entry)),
			},
			None => Ok(None),
		}
	}

	/// Sort the entries by key name.
	///
	/// Entries with the same key are sorted by value.
	pub fn sort(&mut self) {
		// `Entry: Ord` compares key then value, so entries comparing equal are
		// wholly equal and their relative order is unobservable. That makes an
		// unstable sort sound here, and it avoids the auxiliary allocation a
		// stable sort needs.
		self.entries.sort_unstable();
		self.indexes.rebuild_sorted(&self.entries);
	}

	/// Puts this JSON object in canonical form according to
	/// [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785#name-generation-of-canonical-jso).
	///
	/// This will canonicalize the entries and sort them by the UTF-16 code
	/// units of their keys (RFC 8785 section 3.2.3). Entries with the same key
	/// are sorted by value.
	#[cfg(feature = "canonicalize")]
	pub fn canonicalize_with(&mut self, buffer: &mut ryu_js::Buffer) {
		for (_, item) in self.iter_mut() {
			item.canonicalize_with(buffer);
		}

		// UTF-16 order differs from the code point order of `Key: Ord` once a
		// key holds a character above U+FFFF and another key one in
		// U+E000..=U+FFFF. Equal keys still compare equal, so each run of
		// duplicates stays contiguous for `rebuild_sorted`.
		self.entries.sort_unstable_by(|a, b| {
			a.key
				.encode_utf16()
				.cmp(b.key.encode_utf16())
				.then_with(|| a.value.cmp(&b.value))
		});
		self.indexes.rebuild_sorted(&self.entries);
	}

	/// Puts this JSON object in canonical form according to
	/// [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785#name-generation-of-canonical-jso).
	#[cfg(feature = "canonicalize")]
	pub fn canonicalize(&mut self) {
		crate::with_canonicalize_buffer(|buf| self.canonicalize_with(buf));
	}
}

/// Iterator over the entries of an [`Object`], in definition order.
pub type Iter<'a> = slice::Iter<'a, Entry>;

/// Iterator over the entries of an [`Object`], yielding mutable values.
///
/// Returned by [`Object::iter_mut`].
pub struct IterMut<'a>(slice::IterMut<'a, Entry>);

impl<'a> Iterator for IterMut<'a> {
	type Item = (&'a Key, &'a mut Value);

	fn next(&mut self) -> Option<Self::Item> {
		self.0.next().map(|entry| (&entry.key, &mut entry.value))
	}
}

/// Iterator over the entries of an [`Object`], with their code map offsets.
///
/// Returned by [`Object::iter_mapped`].
pub struct IterMapped<'a, 'm> {
	entries: slice::Iter<'a, Entry>,
	code_map: &'m CodeMap,
	offset: usize,
}

impl<'a> Iterator for IterMapped<'a, '_> {
	type Item = MappedEntry<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		self.entries.next().map(|e| {
			let offset = self.offset;
			self.offset += 2 + self
				.code_map
				.get(self.offset + 2)
				.map_or(0, |entry| entry.volume);
			Mapped::new(offset, e.as_ref().into_mapped(offset + 1, offset + 2))
		})
	}
}

impl PartialEq for Object {
	fn eq(&self, other: &Self) -> bool {
		self.entries == other.entries
	}
}

impl Eq for Object {}

impl UnorderedPartialEq for Object {
	fn unordered_eq(&self, other: &Self) -> bool {
		if self.entries.len() != other.entries.len() {
			return false;
		}

		if !self.iter().all(|Entry { key, value: a }| {
			other
				.get_entries(key)
				.any(|Entry { value: b, .. }| a.unordered_eq(b))
		}) {
			return false;
		}

		if self.indexes.contains_duplicate_keys()
			&& !other.iter().all(
				|Entry {
				     key: other_key,
				     value: b,
				 }| {
					self.get_entries(other_key)
						.any(|Entry { value: a, .. }| a.unordered_eq(b))
				},
			) {
			return false;
		}

		true
	}
}

impl UnorderedEq for Object {}

impl PartialOrd for Object {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for Object {
	fn cmp(&self, other: &Self) -> Ordering {
		self.entries.cmp(&other.entries)
	}
}

impl Hash for Object {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.entries.hash(state);
	}
}

impl fmt::Debug for Object {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.debug_map()
			.entries(self.entries.iter().map(Entry::as_pair))
			.finish()
	}
}

impl From<Vec<Entry>> for Object {
	fn from(entries: Vec<Entry>) -> Self {
		Self::from_vec(entries)
	}
}

impl<'a> IntoIterator for &'a Object {
	type Item = &'a Entry;
	type IntoIter = slice::Iter<'a, Entry>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

impl<'a> IntoIterator for &'a mut Object {
	type Item = (&'a Key, &'a mut Value);
	type IntoIter = IterMut<'a>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter_mut()
	}
}

impl IntoIterator for Object {
	type Item = Entry;
	type IntoIter = IntoIter<Entry>;

	fn into_iter(mut self) -> Self::IntoIter {
		self.indexes.clear();
		mem::take(&mut self.entries).into_iter()
	}
}

impl Drop for Object {
	/// Drops the entries without recursing: every nested container is moved
	/// onto a heap worklist and emptied there before it drops, so each drop
	/// only ever sees empty children. An object holding no nested container
	/// allocates nothing.
	fn drop(&mut self) {
		let mut pending: Vec<Value> = self
			.drain_entries()
			.map(Entry::into_value)
			.filter(Value::is_nested_container)
			.collect();
		dismantle(&mut pending);
	}
}

impl Extend<Entry> for Object {
	fn extend<I: IntoIterator<Item = Entry>>(&mut self, iter: I) {
		for entry in iter {
			self.push_entry(entry);
		}
	}
}

impl FromIterator<Entry> for Object {
	fn from_iter<I: IntoIterator<Item = Entry>>(iter: I) -> Self {
		let mut object = Object::default();
		object.extend(iter);
		object
	}
}

impl Extend<(Key, Value)> for Object {
	fn extend<I: IntoIterator<Item = (Key, Value)>>(&mut self, iter: I) {
		for (key, value) in iter {
			self.push(key, value);
		}
	}
}

impl FromIterator<(Key, Value)> for Object {
	fn from_iter<I: IntoIterator<Item = (Key, Value)>>(iter: I) -> Self {
		let mut object = Object::default();
		object.extend(iter);
		object
	}
}

/// Iterator over the positions of the entries matching a key.
///
/// Returned by [`Object::indexes_of`].
#[derive(Default)]
pub enum Indexes<'a> {
	/// At least one entry matches the key.
	Some {
		/// Position of the first matching entry, until it is yielded.
		first: Option<usize>,

		/// Positions of the remaining matching entries.
		other: slice::Iter<'a, usize>,
	},

	/// No entry matches the key.
	#[default]
	None,
}

impl Iterator for Indexes<'_> {
	type Item = usize;

	fn next(&mut self) -> Option<Self::Item> {
		match self {
			Self::Some { first, other } => match first.take() {
				Some(index) => Some(index),
				None => other.next().copied(),
			},
			Self::None => None,
		}
	}
}

macro_rules! entries_iter {
	($($id:ident <$lft:lifetime> {
		type Item = $item:ty ;

		fn next(&mut $self:ident, $index:ident) { $e:expr }
	})*) => {
		$(
			#[doc = concat!(
				"Iterator over the entries of an [`Object`] matching a key, yielding `",
				stringify!($item),
				"`."
			)]
			pub struct $id<$lft> {
				indexes: Indexes<$lft>,
				object: &$lft Object
			}

			impl<$lft> Iterator for $id<$lft> {
				type Item = $item;

				fn next(&mut $self) -> Option<Self::Item> {
					$self.indexes.next().map(|$index| $e)
				}
			}
		)*
	};
}

entries_iter! {
	Values<'a> {
		type Item = &'a Value;

		fn next(&mut self, index) { &self.object.entries[index].value }
	}

	ValuesWithIndex<'a> {
		type Item = (usize, &'a Value);

		fn next(&mut self, index) { (index, &self.object.entries[index].value) }
	}

	Entries<'a> {
		type Item = &'a Entry;

		fn next(&mut self, index) { &self.object.entries[index] }
	}

	EntriesWithIndex<'a> {
		type Item = (usize, &'a Entry);

		fn next(&mut self, index) { (index, &self.object.entries[index]) }
	}
}

macro_rules! entries_iter_mut {
	($($id:ident <$lft:lifetime> {
		type Item = $item:ty ;

		fn next(&mut $self:ident, $index:ident) { $e:expr }
	})*) => {
		$(
			#[doc = concat!(
				"Mutable iterator over the entries of an [`Object`] matching a key, yielding `",
				stringify!($item),
				"`."
			)]
			pub struct $id<$lft> {
				indexes: Indexes<$lft>,
				entries: &$lft mut [Entry]
			}

			impl<$lft> Iterator for $id<$lft> {
				type Item = $item;

				fn next(&mut $self) -> Option<Self::Item> {
					$self.indexes.next().map(|$index| $e)
				}
			}
		)*
	};
}

entries_iter_mut! {
	ValuesMut<'a> {
		type Item = &'a mut Value;

		fn next(&mut self, index) {
			// This is safe because there is no aliasing between the values.
			unsafe { mem::transmute(&mut self.entries[index].value) }
		}
	}

	ValuesMutWithIndex<'a> {
		type Item = (usize, &'a mut Value);

		fn next(&mut self, index) {
			// This is safe because there is no aliasing between the values.
			unsafe { (index, mem::transmute::<&mut Value, &'a mut Value>(&mut self.entries[index].value)) }
		}
	}
}

macro_rules! mapped_entries_iter {
	($($id:ident <$lft:lifetime> {
		type Item = $item:ty ;

		fn next(&mut $self:ident, $index:ident) { $e:expr }
	})*) => {
		$(
			#[doc = concat!(
				"Iterator over the entries of an [`Object`] matching a key, yielding `",
				stringify!($item),
				"` with code map offsets."
			)]
			pub struct $id<$lft, 'm> {
				indexes: Indexes<$lft>,
				object: &$lft Object,
				code_map: &'m CodeMap,
				offset: usize,
				last_index: usize
			}

			impl<$lft, 'm> Iterator for $id<$lft, 'm> {
				type Item = $item;

				fn next(&mut $self) -> Option<Self::Item> {
					$self.indexes.next().map(|$index| {
						while $self.last_index < $index {
							$self.last_index += 1;
							$self.offset += 2 + $self.code_map.get($self.offset+2).unwrap().volume;
						}

						$e
					})
				}
			}
		)*
	};
}

mapped_entries_iter! {
	MappedEntries<'a> {
		type Item = MappedEntry<'a>;

		fn next(&mut self, index) {
			Mapped::new(
				self.offset,
				self.object.entries[index].as_ref().into_mapped(
					self.offset+1,
					self.offset+2
				)
			)
		}
	}

	MappedEntriesWithIndex<'a> {
		type Item = (usize, MappedEntry<'a>);

		fn next(&mut self, index) {
			(
				index,
				Mapped::new(
					self.offset,
					self.object.entries[index].as_ref().into_mapped(
						self.offset+1,
						self.offset+2
					)
				)
			)
		}
	}

	MappedValues<'a> {
		type Item = Mapped<&'a Value>;

		fn next(&mut self, index) {
			Mapped::new(
				self.offset+2,
				&self.object.entries[index].value
			)
		}
	}

	MappedValuesWithIndex<'a> {
		type Item = (usize, Mapped<&'a Value>);

		fn next(&mut self, index) {
			(
				index,
				Mapped::new(
					self.offset+2,
					&self.object.entries[index].value
				)
			)
		}
	}
}

/// Iterator over the entries removed by [`Object::insert`].
///
/// Dropping it removes the remaining duplicates.
pub struct RemovedByInsertion<'a> {
	index: usize,
	first: Option<Entry>,
	object: &'a mut Object,
}

impl Iterator for RemovedByInsertion<'_> {
	type Item = Entry;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some(entry) = self.first.take() {
			Some(entry)
		} else {
			let key = &self.object.entries[self.index].key;
			self.object
				.redundant_index_of(key)
				.and_then(|index| self.object.remove_at(index))
		}
	}
}

impl Drop for RemovedByInsertion<'_> {
	fn drop(&mut self) {
		self.last();
	}
}

/// Iterator over the entries removed by [`Object::insert_front`].
///
/// Dropping it removes the remaining duplicates.
pub struct RemovedByInsertFront<'a> {
	first: Option<Entry>,
	object: &'a mut Object,
}

impl Iterator for RemovedByInsertFront<'_> {
	type Item = Entry;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some(entry) = self.first.take() {
			Some(entry)
		} else {
			let key = &self.object.entries[0].key;
			self.object
				.redundant_index_of(key)
				.and_then(|index| self.object.remove_at(index))
		}
	}
}

impl Drop for RemovedByInsertFront<'_> {
	fn drop(&mut self) {
		self.last();
	}
}

/// Iterator over the entries removed by [`Object::remove`].
///
/// Dropping it removes the remaining matching entries.
pub struct RemovedEntries<'a, 'q, Q: ?Sized>
where
	Q: Hash + Equivalent<Key>,
{
	key: &'q Q,
	object: &'a mut Object,
}

impl<Q: ?Sized> Iterator for RemovedEntries<'_, '_, Q>
where
	Q: Hash + Equivalent<Key>,
{
	type Item = Entry;

	fn next(&mut self) -> Option<Self::Item> {
		self.object
			.index_of(self.key)
			.and_then(|index| self.object.remove_at(index))
	}
}

impl<Q: ?Sized> Drop for RemovedEntries<'_, '_, Q>
where
	Q: Hash + Equivalent<Key>,
{
	fn drop(&mut self) {
		self.last();
	}
}

/// Two items sharing the same object key.
///
/// Returned by the `*_unique` accessors of [`Object`] when the requested key
/// matches more than one entry.
#[derive(Debug)]
pub struct Duplicate<T>(pub T, pub T);

/// Duplicate entry error.
pub type DuplicateEntry = Duplicate<Entry>;

impl fmt::Display for DuplicateEntry {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "duplicate entry `{}`", self.0.key)
	}
}

impl StdError for DuplicateEntry {}

#[cfg(test)]
mod tests {
	use crate::BorrowUnordered;

	use super::*;

	#[test]
	fn object_contains_duplicate_keys_is_true_for_a_repeated_member() {
		let object = Object::from_vec(vec![
			Entry::new("a".into(), Value::Null),
			Entry::new("a".into(), Value::Boolean(true)),
		]);
		assert!(object.contains_duplicate_keys());
	}

	#[test]
	fn object_contains_duplicate_keys_is_false_for_distinct_members() {
		let object = Object::from_vec(vec![
			Entry::new("a".into(), Value::Null),
			Entry::new("b".into(), Value::Null),
		]);
		assert!(!object.contains_duplicate_keys());
	}

	#[test]
	fn object_contains_duplicate_keys_is_false_after_the_duplicate_is_removed() {
		let mut object = Object::from_vec(vec![
			Entry::new("a".into(), Value::Null),
			Entry::new("b".into(), Value::Null),
			Entry::new("a".into(), Value::Boolean(true)),
		]);
		let removed = object.remove_at(2);
		assert!(removed.is_some());
		assert!(!object.contains_duplicate_keys());
	}

	#[test]
	fn remove() {
		let mut object = Object::new();
		object.insert("a".into(), Value::Null);

		object.remove("a");
		object.remove("a");
	}

	#[test]
	fn unordered_eq1() {
		let mut a = Object::new();
		a.push("a".into(), Value::Null);
		a.push("b".into(), Value::Null);

		let mut b = Object::new();
		b.push("b".into(), Value::Null);
		b.push("a".into(), Value::Null);

		assert_ne!(a, b);
		assert_eq!(a.as_unordered(), b.as_unordered());
	}

	#[test]
	fn unordered_eq2() {
		let mut a = Object::new();
		a.push("a".into(), Value::Null);
		a.push("a".into(), Value::Null);

		let mut b = Object::new();
		b.push("a".into(), Value::Null);
		b.push("a".into(), Value::Null);

		assert_eq!(a, b);
		assert_eq!(a.as_unordered(), b.as_unordered());
	}

	#[test]
	fn insert_front1() {
		let mut a = Object::new();
		a.push("a".into(), Value::Null);
		a.push("b".into(), Value::Null);
		a.push("c".into(), Value::Null);
		a.insert_front("b".into(), Value::Null);

		let mut b = Object::new();
		b.push("b".into(), Value::Null);
		b.push("a".into(), Value::Null);
		b.push("c".into(), Value::Null);

		assert_eq!(a, b);
	}

	#[test]
	fn insert_front2() {
		let mut a = Object::new();
		a.push("a".into(), Value::Null);
		a.push("a".into(), Value::Null);
		a.push("c".into(), Value::Null);
		a.insert_front("a".into(), Value::Null);

		let mut b = Object::new();
		b.push("a".into(), Value::Null);
		b.push("c".into(), Value::Null);

		assert_eq!(a, b);
	}

	#[test]
	fn mapped_entries() {
		use crate::Parse;
		let (json, code_map) = crate::Value::parse_str(
			r#"{ "0": [null, null], "1": { "foo": 0, "bar": 1 }, "0": null }"#,
		)
		.unwrap();
		let object = json.into_object().unwrap();

		let offsets: Vec<_> = object
			.get_mapped_entries(&code_map, 0, "0")
			.map(|e| (e.offset, e.value.key.offset, e.value.value.offset))
			.collect();

		assert_eq!(offsets, [(1, 2, 3), (15, 16, 17)]);

		let offsets: Vec<_> = object
			.get_mapped_entries(&code_map, 0, "1")
			.map(|e| (e.offset, e.value.key.offset, e.value.value.offset))
			.collect();

		assert_eq!(offsets, [(6, 7, 8)]);

		let offsets: Vec<_> = object
			.iter_mapped(&code_map, 0)
			.map(|e| (e.offset, e.value.key.offset, e.value.value.offset))
			.collect();

		assert_eq!(offsets, [(1, 2, 3), (6, 7, 8), (15, 16, 17)]);
	}
}
