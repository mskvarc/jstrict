//! Strict JSON parsing and mapping.
//!
//! `jstrict` implements JSON exactly as specified by
//! [RFC 8259](https://datatracker.ietf.org/doc/html/rfc8259) and
//! [ECMA-404](https://www.ecma-international.org/publications-and-standards/standards/ecma-404/),
//! and preserves everything the standard leaves in the document: entry order,
//! duplicate keys, and the lexical form of numbers. Parsing also produces a
//! [`CodeMap`] recording the byte span of every value fragment, so errors and
//! diagnostics can point back at the source text.
//!
//! # Features
//!
//! - Strict by default; the [`parse::Options`] type relaxes surrogate and
//!   codepoint handling for documents that do not adhere to the standard.
//! - No stack overflow: nesting depth is bounded by available memory, not by
//!   the call stack.
//! - Numbers are stored in lexical form, so precision is never lost. See
//!   [`Number`].
//! - Duplicate entries are preserved: an [`Object`] is a list of entries in
//!   definition order, with an index for `O(1)` lookup.
//! - Short strings and keys live inline on the stack, and so do numbers up
//!   to [`NUMBER_CAPACITY`] bytes.
//! - Highly configurable printing, see [`Print`] and [`print::Options`].
//! - [`json!`] builds any value from a JSON literal.
//!
//! # Usage
//!
//! ```
//! use jstrict::{Parse, Print, Value};
//!
//! let (value, code_map) = Value::parse_str(r#"{ "a": 0, "b": [1, 2] }"#)?;
//!
//! let object = value.as_object().unwrap();
//! assert_eq!(object.get_unique("a").unwrap().unwrap().to_string(), "0");
//!
//! // Fragment 0 is the whole object; fragment 3 is the `0` bound to `"a"`.
//! assert_eq!(code_map[3].span.start, 7);
//!
//! println!("{}", value.pretty_print());
//! # Ok::<_, jstrict::parse::Error>(())
//! ```
//!
//! When the code map is not needed, [`parse::parse_str_value`] takes the
//! faster path that skips span tracking entirely:
//!
//! ```
//! let value = jstrict::parse::parse_str_value("[1, 2, 3]")?;
//! assert_eq!(value.as_array().unwrap().len(), 3);
//! # Ok::<_, jstrict::parse::Error>(())
//! ```
//!
//! # Feature flags
//!
//! | Flag           | Default | Enables                                                                          |
//! | -------------- | :-----: | -------------------------------------------------------------------------------- |
//! | `simdutf8`     |   yes   | SIMD-accelerated UTF-8 validation in the slice parser                            |
//! | `canonicalize` |         | JSON Canonicalization Scheme, [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) |
//! | `serde`        |         | `Serialize` / `Deserialize`, plus `to_value` / `from_value`                      |
//! | `serde_json`   |         | Conversion from/to `serde_json::Value`                                           |
//! | `sonic-rs`     |         | Conversion from/to `sonic_rs::Value`                                             |
//! | `contextual`   |         | `contextual::WithContext` printing adapters                                      |
#![cfg_attr(docsrs, feature(doc_cfg), doc(auto_cfg))]
#![warn(missing_docs)]

pub mod number;
pub use crate::number::{InvalidNumber, Number, Sign, TryFromFloatError};
use compact_str::CompactString;
use smallvec::SmallVec;
use std::{fmt, str::FromStr};

pub mod array;
pub(crate) mod byte_scan;
pub mod code_map;
pub mod object;
pub mod parse;
mod unordered;
pub use code_map::CodeMap;
// `CodeMap` entries expose a `locspan::Span` field, so the type is part of this
// crate's public API and is re-exported here: consumers must not have to add
// their own `locspan` dependency, nor match its major version, to read a span.
pub use locspan::Span;
pub use parse::Parse;
pub mod print;
pub use print::Print;
pub mod kind;
pub use kind::{Kind, KindSet};
mod convert;
mod macros;
mod try_from;
pub use try_from::*;

#[cfg(feature = "serde")]
mod serde;

#[cfg(feature = "serde")]
pub use self::serde::*;

pub use unordered::*;

/// String.
///
/// Strings short enough to fit inline are stored on the stack; longer ones
/// spill to the heap. The inline capacity is `compact_str`'s own, one word
/// short of three pointers, and is not configurable.
pub type String = CompactString;

pub use array::Array;

pub use object::Object;

/// Number buffer stack capacity.
///
/// If the number is longer than this value,
/// it will be stored on the heap.
pub const NUMBER_CAPACITY: usize = 16;

/// Number buffer.
pub type NumberBuf = crate::number::SmallNumberBuf<NUMBER_CAPACITY>;

/// JSON Value.
///
/// # Parsing
///
/// You can parse a `Value` by importing the [`Parse`] trait providing a
/// collection of parsing functions.
///
/// ## Example
///
/// ```
/// use jstrict::{Value, Parse, CodeMap};
/// let (value, code_map) = Value::parse_str("{ \"key\": \"value\" }").unwrap();
/// ```
///
/// The `code_map` value of type [`CodeMap`] contains code-mapping information
/// about all the fragments of the JSON value (their location in the source
/// text).
///
/// # Comparison
///
/// This type implements the usual comparison traits `PartialEq`, `Eq`,
/// `PartialOrd` and `Ord`. However by default JSON object entries ordering
/// matters, meaning that `{ "a": 0, "b": 1 }` is **not** equal to
/// `{ "b": 1, "a": 0 }`.
/// If you want to do comparisons while ignoring entries ordering, you can use
/// the [`Unordered`] type (combined with the [`UnorderedPartialEq`] trait).
/// Any `T` reference can be turned into an [`Unordered<T>`] reference
/// at will using the [`BorrowUnordered::as_unordered`] method.
///
/// ## Example
///
/// ```
/// use jstrict::{json, Unordered, BorrowUnordered};
///
/// let a = json!({ "a": 0, "b": 1 });
/// let b = json!({ "b": 1, "a": 0 });
///
/// assert_ne!(a, b); // not equals entries are in a different order.
/// assert_eq!(a.as_unordered(), b.as_unordered()); // equals modulo entry order.
/// assert_eq!(Unordered(a), Unordered(b)); // equals modulo entry order.
/// ```
///
/// # Printing
///
/// The [`Print`] trait provide a highly configurable printing method.
///
/// ## Example
///
/// ```
/// use jstrict::{Value, Parse, Print};
///
/// let value = Value::parse_str("[ 0, 1, { \"key\": \"value\" }, null ]").unwrap().0;
///
/// println!("{}", value.pretty_print()); // multi line, indent with 2 spaces
/// println!("{}", value.inline_print()); // single line, spaces
/// println!("{}", value.compact_print()); // single line, no spaces
///
/// let mut options = jstrict::print::Options::pretty();
/// options.indent = jstrict::print::Indent::Tabs(1);
/// println!("{}", value.print_with(options)); // multi line, indent with tabs
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Value {
	/// `null`.
	Null,

	/// Boolean `true` or `false`.
	Boolean(bool),

	/// Number.
	Number(NumberBuf),

	/// String.
	String(String),

	/// Array.
	Array(Array),

	/// Object.
	Object(Object),
}

/// Returns the fragment of `array` at the given `index`, in traversal order.
///
/// The index is relative to the *contents* of the array: index `0` is the
/// first fragment of the first item. On failure, returns the number of
/// fragments left to skip past the end of `array`, so callers can chain
/// lookups across sibling values.
pub fn get_array_fragment(array: &[Value], mut index: usize) -> Result<FragmentRef<'_>, usize> {
	for v in array {
		match v.get_fragment(index) {
			Ok(value) => return Ok(value),
			Err(i) => index = i,
		}
	}

	Err(index)
}

impl Value {
	/// Returns the fragment at the given `index` in traversal order, where
	/// index `0` is `self`.
	///
	/// Fragment indices match [`CodeMap`] offsets: the fragment returned for
	/// index `i` is described by `code_map[offset + i]`. On failure, returns
	/// the number of fragments left to skip.
	pub fn get_fragment(&self, index: usize) -> Result<FragmentRef<'_>, usize> {
		if index == 0 {
			Ok(FragmentRef::Value(self))
		} else {
			match self {
				Self::Array(a) => get_array_fragment(a, index - 1),
				Self::Object(o) => o.get_fragment(index - 1),
				_ => Err(index - 1),
			}
		}
	}

	/// Returns the [`Kind`] of this value.
	#[inline]
	pub const fn kind(&self) -> Kind {
		match self {
			Self::Null => Kind::Null,
			Self::Boolean(_) => Kind::Boolean,
			Self::Number(_) => Kind::Number,
			Self::String(_) => Kind::String,
			Self::Array(_) => Kind::Array,
			Self::Object(_) => Kind::Object,
		}
	}

	/// Checks if this value is of the given `kind`.
	#[inline]
	pub fn is_kind(&self, kind: Kind) -> bool {
		self.kind() == kind
	}

	/// Checks if this value is `null`.
	#[inline]
	pub const fn is_null(&self) -> bool {
		matches!(self, Self::Null)
	}

	/// Checks if this value is a boolean.
	#[inline]
	pub const fn is_boolean(&self) -> bool {
		matches!(self, Self::Boolean(_))
	}

	/// Checks if this value is a number.
	#[inline]
	pub const fn is_number(&self) -> bool {
		matches!(self, Self::Number(_))
	}

	/// Checks if this value is a string.
	#[inline]
	pub const fn is_string(&self) -> bool {
		matches!(self, Self::String(_))
	}

	/// Checks if this value is an array.
	#[inline]
	pub const fn is_array(&self) -> bool {
		matches!(self, Self::Array(_))
	}

	/// Checks if this value is an object.
	#[inline]
	pub const fn is_object(&self) -> bool {
		matches!(self, Self::Object(_))
	}

	/// Checks if the value is either an empty array or an empty object.
	#[inline]
	pub fn is_empty_array_or_object(&self) -> bool {
		match self {
			Self::Array(a) => a.is_empty(),
			Self::Object(o) => o.is_empty(),
			_ => false,
		}
	}

	/// Returns this value as a `bool`, if it is a boolean.
	#[inline]
	pub const fn as_boolean(&self) -> Option<bool> {
		match self {
			Self::Boolean(b) => Some(*b),
			_ => None,
		}
	}

	/// Returns a mutable reference to the inner `bool`, if this is a boolean.
	#[inline]
	pub const fn as_boolean_mut(&mut self) -> Option<&mut bool> {
		match self {
			Self::Boolean(b) => Some(b),
			_ => None,
		}
	}

	/// Returns this value as a [`Number`], if it is a number.
	#[inline]
	pub fn as_number(&self) -> Option<&Number> {
		match self {
			Self::Number(n) => Some(n),
			_ => None,
		}
	}

	/// Returns a mutable reference to the inner number buffer, if this is a
	/// number.
	#[inline]
	pub const fn as_number_mut(&mut self) -> Option<&mut NumberBuf> {
		match self {
			Self::Number(n) => Some(n),
			_ => None,
		}
	}

	/// Returns this value as a `&str`, if it is a string.
	#[inline]
	pub fn as_string(&self) -> Option<&str> {
		match self {
			Self::String(s) => Some(s),
			_ => None,
		}
	}

	/// Alias for [`as_string`](Self::as_string).
	#[inline]
	pub fn as_str(&self) -> Option<&str> {
		self.as_string()
	}

	/// Returns a mutable reference to the inner string, if this is a string.
	#[inline]
	pub const fn as_string_mut(&mut self) -> Option<&mut String> {
		match self {
			Self::String(s) => Some(s),
			_ => None,
		}
	}

	/// Returns this value as a slice of values, if it is an array.
	#[inline]
	pub fn as_array(&self) -> Option<&[Self]> {
		match self {
			Self::Array(a) => Some(a),
			_ => None,
		}
	}

	/// Returns a mutable reference to the inner array, if this is an array.
	#[inline]
	pub const fn as_array_mut(&mut self) -> Option<&mut Array> {
		match self {
			Self::Array(a) => Some(a),
			_ => None,
		}
	}

	/// Return the given value as an array, even if it is not an array.
	///
	/// Returns the input value as is if it is already an array,
	/// or puts it in a slice with a single element if it is not.
	#[inline]
	pub fn force_as_array(&self) -> &[Self] {
		match self {
			Self::Array(a) => a,
			other => core::slice::from_ref(other),
		}
	}

	/// Returns this value as an [`Object`], if it is an object.
	#[inline]
	pub const fn as_object(&self) -> Option<&Object> {
		match self {
			Self::Object(o) => Some(o),
			_ => None,
		}
	}

	/// Returns a mutable reference to the inner object, if this is an object.
	#[inline]
	pub const fn as_object_mut(&mut self) -> Option<&mut Object> {
		match self {
			Self::Object(o) => Some(o),
			_ => None,
		}
	}

	/// Consumes this value, returning the inner `bool` if it is a boolean.
	#[inline]
	pub fn into_boolean(self) -> Option<bool> {
		match self {
			Self::Boolean(b) => Some(b),
			_ => None,
		}
	}

	/// Consumes this value, returning the inner number if it is a number.
	#[inline]
	pub fn into_number(self) -> Option<NumberBuf> {
		match self {
			Self::Number(n) => Some(n),
			_ => None,
		}
	}

	/// Consumes this value, returning the inner string if it is a string.
	#[inline]
	pub fn into_string(self) -> Option<String> {
		match self {
			Self::String(s) => Some(s),
			_ => None,
		}
	}

	/// Consumes this value, returning the inner array if it is an array.
	#[inline]
	pub fn into_array(self) -> Option<Array> {
		match self {
			Self::Array(a) => Some(a),
			_ => None,
		}
	}

	/// Consumes this value, returning the inner object if it is an object.
	#[inline]
	pub fn into_object(self) -> Option<Object> {
		match self {
			Self::Object(o) => Some(o),
			_ => None,
		}
	}

	/// Returns an iterator over every fragment of this value, in depth-first
	/// order, paired with its index.
	///
	/// The index of each fragment is its offset in the [`CodeMap`] produced
	/// when the value was parsed.
	pub fn traverse(&self) -> Traverse<'_> {
		let mut stack = SmallVec::new();
		stack.push(FragmentRef::Value(self));
		Traverse { offset: 0, stack }
	}

	/// Recursively count the number of values for which `f` returns `true`.
	pub fn count(&self, mut f: impl FnMut(usize, FragmentRef) -> bool) -> usize {
		self.traverse().filter(|(i, q)| f(*i, *q)).count()
	}

	/// Returns the volume of the value.
	///
	/// The volume is the sum of all values and recursively nested values
	/// included in `self`, including `self` (the volume is at least `1`).
	///
	/// This is equivalent to `value.traverse().filter(|(_, f)| f.is_value()).count()`.
	pub fn volume(&self) -> usize {
		self.traverse().filter(|(_, f)| f.is_value()).count()
	}

	/// Move and return the value, leaves `null` in its place.
	#[inline(always)]
	pub const fn take(&mut self) -> Self {
		let mut result = Self::Null;
		std::mem::swap(&mut result, self);
		result
	}

	/// Puts this JSON value in canonical form according to
	/// [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785).
	///
	/// The given `buffer` is used to canonicalize the number values.
	#[cfg(feature = "canonicalize")]
	pub fn canonicalize_with(&mut self, buffer: &mut ryu_js::Buffer) {
		match self {
			Self::Number(n) => *n = NumberBuf::from_number(n.canonical_with(buffer)),
			Self::Array(a) => {
				for item in a {
					item.canonicalize_with(buffer)
				}
			}
			Self::Object(o) => o.canonicalize_with(buffer),
			_ => (),
		}
	}

	/// Puts this JSON value in canonical form according to
	/// [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785).
	///
	/// For predictable performance in tight loops, use
	/// [`Self::canonicalize_with`] with a caller-owned buffer.
	#[cfg(feature = "canonicalize")]
	pub fn canonicalize(&mut self) {
		crate::with_canonicalize_buffer(|buf| self.canonicalize_with(buf))
	}
}

#[cfg(feature = "canonicalize")]
thread_local! {
	static CANONICALIZE_BUFFER: std::cell::RefCell<ryu_js::Buffer> =
		std::cell::RefCell::new(ryu_js::Buffer::new());
}

#[cfg(feature = "canonicalize")]
fn with_canonicalize_buffer<R>(f: impl FnOnce(&mut ryu_js::Buffer) -> R) -> R {
	CANONICALIZE_BUFFER.with_borrow_mut(f)
}

impl UnorderedPartialEq for Value {
	fn unordered_eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::Null, Self::Null) => true,
			(Self::Boolean(a), Self::Boolean(b)) => a == b,
			(Self::Number(a), Self::Number(b)) => a == b,
			(Self::String(a), Self::String(b)) => a == b,
			(Self::Array(a), Self::Array(b)) => a.unordered_eq(b),
			(Self::Object(a), Self::Object(b)) => a.unordered_eq(b),
			_ => false,
		}
	}
}

impl UnorderedEq for Value {}

impl fmt::Display for Value {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.compact_print().fmt(f)
	}
}

impl From<Value> for ::std::string::String {
	fn from(value: Value) -> Self {
		value.to_string()
	}
}

impl From<bool> for Value {
	fn from(b: bool) -> Self {
		Self::Boolean(b)
	}
}

impl From<NumberBuf> for Value {
	fn from(n: NumberBuf) -> Self {
		Self::Number(n)
	}
}

impl<'n> From<&'n Number> for Value {
	fn from(n: &'n Number) -> Self {
		Self::Number(unsafe { NumberBuf::new_unchecked(n.as_bytes().into()) })
	}
}

impl From<String> for Value {
	fn from(s: String) -> Self {
		Self::String(s)
	}
}

impl From<::std::string::String> for Value {
	fn from(s: ::std::string::String) -> Self {
		Self::String(s.into())
	}
}

impl<'s> From<&'s str> for Value {
	fn from(s: &'s str) -> Self {
		Self::String(s.into())
	}
}

impl From<Array> for Value {
	fn from(a: Array) -> Self {
		Self::Array(a)
	}
}

impl From<Object> for Value {
	fn from(o: Object) -> Self {
		Self::Object(o)
	}
}

impl FromStr for Value {
	type Err = parse::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Self::parse_str(s)?.0)
	}
}

macro_rules! from_integer {
	($($ty:ident),*) => {
		$(
			impl From<$ty> for Value {
				fn from(n: $ty) -> Self {
					Value::Number(n.into())
				}
			}
		)*
	};
}

from_integer! {
	u8,
	u16,
	u32,
	u64,
	i8,
	i16,
	i32,
	i64
}

macro_rules! try_from_float {
	($($ty:ident),*) => {
		$(
			impl TryFrom<$ty> for Value {
				type Error = crate::number::TryFromFloatError;

				fn try_from(n: $ty) -> Result<Self, Self::Error> {
					Ok(Value::Number(n.try_into()?))
				}
			}
		)*
	};
}

try_from_float! {
	f32,
	f64
}

/// Reference to a fragment of a JSON value.
///
/// A *fragment* is anything the parser assigns a source span to: a value, an
/// object entry, or an object key. Fragments are what [`Value::traverse`]
/// yields and what a [`CodeMap`] describes.
pub enum FragmentRef<'a> {
	/// A JSON value.
	Value(&'a Value),

	/// An object entry, spanning both its key and its value.
	Entry(&'a object::Entry),

	/// An object key.
	Key(&'a object::Key),
}

impl<'a> FragmentRef<'a> {
	/// Checks if this fragment is an object entry.
	pub const fn is_entry(&self) -> bool {
		matches!(self, Self::Entry(_))
	}

	/// Checks if this fragment is an object key.
	pub const fn is_key(&self) -> bool {
		matches!(self, Self::Key(_))
	}

	/// Checks if this fragment is a value.
	pub const fn is_value(&self) -> bool {
		matches!(self, Self::Value(_))
	}

	/// Checks if this fragment is the `null` value.
	pub const fn is_null(&self) -> bool {
		matches!(self, Self::Value(Value::Null))
	}

	/// Checks if this fragment is a number value.
	pub const fn is_number(&self) -> bool {
		matches!(self, Self::Value(Value::Number(_)))
	}

	/// Checks if this fragment is a string value.
	pub const fn is_string(&self) -> bool {
		matches!(self, Self::Value(Value::String(_)))
	}

	/// Checks if this fragment is an array value.
	pub const fn is_array(&self) -> bool {
		matches!(self, Self::Value(Value::Array(_)))
	}

	/// Checks if this fragment is an object value.
	pub const fn is_object(&self) -> bool {
		matches!(self, Self::Value(Value::Object(_)))
	}

	/// Returns this fragment reference unchanged.
	pub const fn strip(self) -> FragmentRef<'a> {
		match self {
			Self::Value(v) => FragmentRef::Value(v),
			Self::Entry(e) => FragmentRef::Entry(e),
			Self::Key(k) => FragmentRef::Key(k),
		}
	}
}

impl<'a> Clone for FragmentRef<'a> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<'a> Copy for FragmentRef<'a> {}

impl<'a> FragmentRef<'a> {
	/// Returns an iterator over the direct sub-fragments of this fragment.
	pub fn sub_fragments(&self) -> SubFragments<'a> {
		match self {
			Self::Value(Value::Array(a)) => SubFragments::Array(a.iter()),
			Self::Value(Value::Object(o)) => SubFragments::Object(o.iter()),
			Self::Entry(e) => SubFragments::Entry(Some(&e.key), Some(&e.value)),
			_ => SubFragments::None,
		}
	}
}

/// Iterator over the direct sub-fragments of a [`FragmentRef`].
pub enum SubFragments<'a> {
	/// The fragment is a leaf: it has no sub-fragments.
	None,

	/// Sub-fragments of an array: its items.
	Array(core::slice::Iter<'a, Value>),

	/// Sub-fragments of an object: its entries.
	Object(core::slice::Iter<'a, object::Entry>),

	/// Sub-fragments of an object entry: its key, then its value.
	Entry(Option<&'a object::Key>, Option<&'a Value>),
}

impl<'a> Iterator for SubFragments<'a> {
	type Item = FragmentRef<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		match self {
			Self::None => None,
			Self::Array(a) => a.next().map(FragmentRef::Value),
			Self::Object(e) => e.next().map(FragmentRef::Entry),
			Self::Entry(k, v) => k
				.take()
				.map(FragmentRef::Key)
				.or_else(|| v.take().map(FragmentRef::Value)),
		}
	}
}

impl<'a> DoubleEndedIterator for SubFragments<'a> {
	fn next_back(&mut self) -> Option<Self::Item> {
		match self {
			Self::None => None,
			Self::Array(a) => a.next_back().map(FragmentRef::Value),
			Self::Object(e) => e.next_back().map(FragmentRef::Entry),
			Self::Entry(k, v) => v
				.take()
				.map(FragmentRef::Value)
				.or_else(|| k.take().map(FragmentRef::Key)),
		}
	}
}

/// Depth-first iterator over every fragment of a [`Value`], with its index.
///
/// Returned by [`Value::traverse`].
pub struct Traverse<'a> {
	offset: usize,
	stack: SmallVec<[FragmentRef<'a>; 8]>,
}

impl<'a> Iterator for Traverse<'a> {
	type Item = (usize, FragmentRef<'a>);

	fn next(&mut self) -> Option<Self::Item> {
		match self.stack.pop() {
			Some(v) => {
				self.stack.extend(v.sub_fragments().rev());
				let i = self.offset;
				self.offset += 1;
				Some((i, v))
			}
			None => None,
		}
	}
}

#[cfg(test)]
mod tests {
	#[cfg(feature = "canonicalize")]
	#[test]
	fn canonicalize_01() {
		use super::*;
		let mut value: Value = json!({
			"b": 0.00000000001,
			"c": {
				"foo": true,
				"bar": false
			},
			"a": [ "foo", "bar" ]
		});

		value.canonicalize();

		assert_eq!(
			value.compact_print().to_string(),
			"{\"a\":[\"foo\",\"bar\"],\"b\":1e-11,\"c\":{\"bar\":false,\"foo\":true}}"
		)
	}

	#[cfg(feature = "canonicalize")]
	#[test]
	fn canonicalize_02() {
		use super::*;
		let (mut value, _) = Value::parse_str(
			"{
			\"numbers\": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],
			\"string\": \"\\u20ac$\\u000F\\u000aA'\\u0042\\u0022\\u005c\\\\\\\"\\/\",
			\"literals\": [null, true, false]
		}",
		)
		.unwrap();

		value.canonicalize();

		assert_eq!(
			value.compact_print().to_string(),
			"{\"literals\":[null,true,false],\"numbers\":[333333333.3333333,1e+30,4.5,0.002,1e-27],\"string\":\"€$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\"}"
		)
	}
}
