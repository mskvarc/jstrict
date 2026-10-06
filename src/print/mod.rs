//! JSON printing.
//!
//! [`Print`] is the entry point. `pretty_print`, `inline_print` and
//! `compact_print` cover the common cases; `print_with` takes an [`Options`]
//! for everything else — indentation, padding, and the width or item count at
//! which an array or object is expanded onto several lines.
//!
//! Printing is a two-pass process: sizes are precomputed
//! ([`PrecomputeSize`]), then written out ([`PrintWithSize`]), so the layout
//! decision for a nested value is known before it is emitted.
//!
//! Strings are escaped per
//! [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785#name-serialization-of-strings).
use crate::byte_scan::find_escape;
use crate::native_stack::on_sufficient_stack;
use core::str;
use std::cell::RefCell;
use std::fmt;

#[cfg(feature = "contextual")]
mod contextual;

#[cfg(feature = "contextual")]
pub use self::contextual::*;

/// Indentation unit.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Indent {
	/// The given number of spaces.
	Spaces(u8),

	/// The given number of tabs.
	Tabs(u8),
}

impl Indent {
	/// Repeats this indentation unit `n` times.
	#[must_use]
	pub const fn by(self, n: usize) -> IndentBy {
		IndentBy(self, n)
	}
}

impl fmt::Display for Indent {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Spaces(n) => {
				for _ in 0..*n {
					f.write_str(" ")?;
				}
			}
			Self::Tabs(n) => {
				for _ in 0..*n {
					f.write_str("\t")?;
				}
			}
		}

		Ok(())
	}
}

/// An [`Indent`] repeated a given number of times, ready to be displayed.
pub struct IndentBy(Indent, usize);

impl fmt::Display for IndentBy {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		for _ in 0..self.1 {
			self.0.fmt(f)?;
		}

		Ok(())
	}
}

/// The given number of spaces, ready to be displayed.
pub struct Spaces(pub usize);

impl fmt::Display for Spaces {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		for _ in 0..self.0 {
			f.write_str(" ")?;
		}

		Ok(())
	}
}

/// Padding inserted between syntactic elements.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Padding {
	/// The given number of spaces.
	Spaces(u8),

	/// A line break.
	NewLine,
}

impl fmt::Display for Padding {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Spaces(n) => {
				for _ in 0..*n {
					f.write_str(" ")?;
				}
			}
			Self::NewLine => f.write_str("\n")?,
		}

		Ok(())
	}
}

/// Threshold past which an array or object is expanded onto several lines.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Limit {
	/// Always expanded, even if empty.
	Always,

	/// Expanded if the array/object has more than the given number of items.
	Item(usize),

	/// Expanded if the representation of the array/object is more than the
	/// given number of characters long.
	Width(usize),

	/// Expanded if the array/object has more than the given number of items
	/// (first argument), or if its the representation is more than the
	/// given number of characters long (second argument).
	ItemOrWidth(usize, usize),
}

/// Print options.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub struct Options {
	/// Indentation string.
	pub indent: Indent,

	/// String added after `[`.
	pub array_begin: usize,

	/// String added before `]`.
	pub array_end: usize,

	/// Number of spaces inside an inlined empty array.
	pub array_empty: usize,

	/// Number of spaces before a comma in an array.
	pub array_before_comma: usize,

	/// Number of spaces after a comma in an array.
	pub array_after_comma: usize,

	/// Limit after which an array is expanded.
	pub array_limit: Option<Limit>,

	/// String added after `{`.
	pub object_begin: usize,

	/// String added before `}`.
	pub object_end: usize,

	/// Number of spaces inside an inlined empty object.
	pub object_empty: usize,

	/// Number of spaces before a comma in an object.
	pub object_before_comma: usize,

	/// Number of spaces after a comma in an object.
	pub object_after_comma: usize,

	/// Number of spaces before a colon in an object.
	pub object_before_colon: usize,

	/// Number of spaces after a colon in an object.
	pub object_after_colon: usize,

	/// Limit after which an array is expanded.
	pub object_limit: Option<Limit>,
}

impl Options {
	/// Pretty print options.
	#[inline(always)]
	#[must_use]
	pub const fn pretty() -> Self {
		Self {
			indent: Indent::Spaces(2),
			array_begin: 1,
			array_end: 1,
			array_empty: 0,
			array_before_comma: 0,
			array_after_comma: 1,
			array_limit: Some(Limit::ItemOrWidth(1, 16)),
			object_begin: 1,
			object_end: 1,
			object_empty: 0,
			object_before_comma: 0,
			object_after_comma: 1,
			object_before_colon: 0,
			object_after_colon: 1,
			object_limit: Some(Limit::ItemOrWidth(1, 16)),
		}
	}

	/// Compact print options.
	///
	/// Values will be formatted on a single line without spaces.
	#[inline(always)]
	#[must_use]
	pub const fn compact() -> Self {
		Self {
			indent: Indent::Spaces(0),
			array_begin: 0,
			array_end: 0,
			array_empty: 0,
			array_before_comma: 0,
			array_after_comma: 0,
			array_limit: None,
			object_begin: 0,
			object_end: 0,
			object_empty: 0,
			object_before_comma: 0,
			object_after_comma: 0,
			object_before_colon: 0,
			object_after_colon: 0,
			object_limit: None,
		}
	}

	/// Inline print options.
	///
	/// Values will be formatted on a single line with some spaces.
	#[inline(always)]
	#[must_use]
	pub const fn inline() -> Self {
		Self {
			indent: Indent::Spaces(0),
			array_begin: 1,
			array_end: 1,
			array_empty: 0,
			array_before_comma: 0,
			array_after_comma: 1,
			array_limit: None,
			object_begin: 1,
			object_end: 1,
			object_empty: 0,
			object_before_comma: 0,
			object_after_comma: 1,
			object_before_colon: 0,
			object_after_colon: 1,
			object_limit: None,
		}
	}
}

/// The size of a value.
#[derive(Clone, Copy)]
pub enum Size {
	/// The value (array or object) is expanded on multiple lines.
	Expanded,

	/// The value is formatted in a single line with the given character width.
	Width(usize),
}

impl Size {
	/// Concatenates `other` to `self`.
	///
	/// Widths add up; anything combined with [`Size::Expanded`] stays
	/// expanded.
	pub const fn add(&mut self, other: Self) {
		*self = match (*self, other) {
			(Self::Width(a), Self::Width(b)) => Self::Width(a + b),
			_ => Self::Expanded,
		}
	}
}

/// Print methods.
///
/// Printing a [`Value`](crate::Value) recurses once per nesting level; each
/// level runs through [`on_sufficient_stack`], so printing a value of any
/// depth needs only [`RED_ZONE`](crate::native_stack::RED_ZONE) of the
/// caller's stack.
pub trait Print {
	/// Print the value with `Options::pretty` options.
	#[inline(always)]
	fn pretty_print(&self) -> Printed<'_, Self> {
		self.print_with(Options::pretty())
	}

	/// Print the value with `Options::compact` options.
	#[inline(always)]
	fn compact_print(&self) -> Printed<'_, Self> {
		self.print_with(Options::compact())
	}

	/// Print the value with `Options::inline` options.
	#[inline(always)]
	fn inline_print(&self) -> Printed<'_, Self> {
		self.print_with(Options::inline())
	}

	/// Print the value with the given options.
	#[inline(always)]
	fn print_with(&self, options: Options) -> Printed<'_, Self> {
		Printed(self, options, 0)
	}

	/// Writes the value to `f` with the given options, starting at the given
	/// indentation level.
	/// # Errors
	///
	/// Returns [`fmt::Error`] if writing to the formatter fails.
	fn fmt_with(&self, f: &mut fmt::Formatter, options: &Options, indent: usize) -> fmt::Result;
}

impl<T: Print + ?Sized> Print for &T {
	fn fmt_with(&self, f: &mut fmt::Formatter, options: &Options, indent: usize) -> fmt::Result {
		(**self).fmt_with(f, options, indent)
	}
}

/// Printing with precomputed sizes.
///
/// Implemented by the value types that can be expanded over several lines.
/// `sizes` holds the layout decisions produced by [`PrecomputeSize`], in
/// depth-first order, and `index` walks through them as printing proceeds.
///
/// Recurses once per nesting level; each level runs through
/// [`on_sufficient_stack`].
pub trait PrintWithSize {
	/// Writes the value to `f`, taking its layout from `sizes[*index]`.
	/// # Errors
	///
	/// Returns [`fmt::Error`] if writing to the formatter fails.
	fn fmt_with_size(
		&self,
		f: &mut fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> fmt::Result;
}

impl<T: PrintWithSize + ?Sized> PrintWithSize for &T {
	fn fmt_with_size(
		&self,
		f: &mut fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> fmt::Result {
		(**self).fmt_with_size(f, options, indent, sizes, index)
	}
}

/// Printed value.
pub struct Printed<'t, T: ?Sized>(&'t T, Options, usize);

impl<T: Print> fmt::Display for Printed<'_, T> {
	#[inline(always)]
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.0.fmt_with(f, &self.1, self.2)
	}
}

impl Print for bool {
	#[inline(always)]
	fn fmt_with(&self, f: &mut fmt::Formatter, _options: &Options, _indent: usize) -> fmt::Result {
		if *self {
			f.write_str("true")
		} else {
			f.write_str("false")
		}
	}
}

impl Print for crate::NumberBuf {
	#[inline(always)]
	fn fmt_with(&self, f: &mut fmt::Formatter, _options: &Options, _indent: usize) -> fmt::Result {
		fmt::Display::fmt(self, f)
	}
}

/// Lookup table mapping each ASCII control byte / `"` / `\\` to the
/// emitted byte length when escaped per RFC 8785. Non-escape bytes map
/// to 0 so the scanner can detect "needs escaping" with a single
/// table lookup. Bytes >= 0x20 (excluding `"` and `\\`) are 0.
static ESCAPE_LEN: [u8; 256] = {
	let mut t = [0u8; 256];
	let mut i = 0;
	while i < 0x20 {
		t[i] = 6;
		i += 1;
	}
	t[0x08] = 2; // \b
	t[0x09] = 2; // \t
	t[0x0a] = 2; // \n
	t[0x0c] = 2; // \f
	t[0x0d] = 2; // \r
	t[b'"' as usize] = 2;
	t[b'\\' as usize] = 2;
	t
};

/// Writes the escape sequence for a single byte that requires escaping.
fn write_escape(b: u8, f: &mut fmt::Formatter) -> fmt::Result {
	match b {
		b'\\' => f.write_str("\\\\"),
		b'"' => f.write_str("\\\""),
		0x08 => f.write_str("\\b"),
		0x09 => f.write_str("\\t"),
		0x0a => f.write_str("\\n"),
		0x0c => f.write_str("\\f"),
		0x0d => f.write_str("\\r"),
		c => {
			f.write_str("\\u00")?;
			let hi = (c >> 4) & 0x0f;
			let lo = c & 0x0f;
			f.write_str(&HEX[(hi as usize)..=(hi as usize)])?;
			f.write_str(&HEX[(lo as usize)..=(lo as usize)])
		}
	}
}

const HEX: &str = "0123456789abcdef";

/// Threshold (in bytes) below which `string_literal` walks the SWAR path
/// directly. Above it, the SIMD escape encoder is used. Tuned so the SIMD
/// startup cost (thread-local borrow + reserve) does not dominate.
const SIMD_ESCAPE_MIN: usize = 64;

thread_local! {
	static ESCAPE_BUFFER: RefCell<Vec<u8>> =
		RefCell::new(Vec::with_capacity(256));
}

/// Formats a string literal according to [RFC8785](https://www.rfc-editor.org/rfc/rfc8785#name-serialization-of-strings).
/// # Errors
///
/// Returns [`fmt::Error`] if writing to the formatter fails.
pub fn string_literal(s: &str, f: &mut fmt::Formatter) -> fmt::Result {
	let bytes = s.as_bytes();
	if bytes.len() < SIMD_ESCAPE_MIN {
		f.write_str("\"")?;
		let mut start = 0;
		while let Some(off) = find_escape(&bytes[start..]) {
			let i = start + off;
			if start < i {
				// SAFETY: bytes[start..i] is a prefix-aligned slice of the
				// original `&str`. Escape bytes are all ASCII, so any
				// boundary between them lies on a UTF-8 char boundary.
				let run = unsafe { str::from_utf8_unchecked(&bytes[start..i]) };
				f.write_str(run)?;
			}
			write_escape(bytes[i], f)?;
			start = i + 1;
		}
		if start < bytes.len() {
			let run = unsafe { str::from_utf8_unchecked(&bytes[start..]) };
			f.write_str(run)?;
		}
		f.write_str("\"")
	} else {
		ESCAPE_BUFFER.with_borrow_mut(|buf| {
			buf.clear();
			// `format_string` panics if the destination is undersized; the
			// worst case is 6 bytes per input byte plus the surrounding
			// quotes and a SIMD overshoot margin.
			buf.reserve(s.len() * 6 + 32 + 3);
			json_escape_simd::escape_into(s, buf);
			// SAFETY: `escape_into` writes only ASCII escape sequences and
			// the original UTF-8 source bytes. The two surrounding `"` and
			// any escape bytes are ASCII, and untouched runs of input are
			// already valid UTF-8.
			f.write_str(unsafe { str::from_utf8_unchecked(buf) })
		})
	}
}

/// Returns the byte length of string literal according to [RFC8785](https://www.rfc-editor.org/rfc/rfc8785#name-serialization-of-strings).
#[must_use]
pub fn printed_string_size(s: &str) -> usize {
	let mut width = 2usize;
	for &b in s.as_bytes() {
		let n = ESCAPE_LEN[b as usize] as usize;
		// Non-escape bytes count as 1 (covers all UTF-8 continuation and
		// leading bytes >= 0x20 except `"` and `\\`).
		width += if n == 0 { 1 } else { n };
	}
	width
}

impl Print for crate::String {
	#[inline(always)]
	fn fmt_with(&self, f: &mut fmt::Formatter, _options: &Options, _indent: usize) -> fmt::Result {
		string_literal(self, f)
	}
}

/// Writes `items` as a JSON array, using the layout at `sizes[*index]`.
/// # Errors
///
/// Returns [`fmt::Error`] if writing to the formatter fails.
pub fn print_array<I: IntoIterator>(
	items: I,
	f: &mut fmt::Formatter,
	options: &Options,
	indent: usize,
	sizes: &[Size],
	index: &mut usize,
) -> fmt::Result
where
	I::IntoIter: ExactSizeIterator,
	I::Item: PrintWithSize,
{
	use fmt::Display;
	let size = sizes[*index];
	*index += 1;

	f.write_str("[")?;

	let items = items.into_iter();
	if items.len() == 0 {
		match size {
			Size::Expanded => {
				f.write_str("\n")?;
				options.indent.by(indent).fmt(f)?;
			}
			Size::Width(_) => Spaces(options.array_empty).fmt(f)?,
		}
	} else {
		match size {
			Size::Expanded => {
				f.write_str("\n")?;

				for (i, item) in items.enumerate() {
					if i > 0 {
						Spaces(options.array_before_comma).fmt(f)?;
						f.write_str(",\n")?;
					}

					options.indent.by(indent + 1).fmt(f)?;
					item.fmt_with_size(f, options, indent + 1, sizes, index)?;
				}

				f.write_str("\n")?;
				options.indent.by(indent).fmt(f)?;
			}
			Size::Width(_) => {
				Spaces(options.array_begin).fmt(f)?;
				for (i, item) in items.enumerate() {
					if i > 0 {
						Spaces(options.array_before_comma).fmt(f)?;
						f.write_str(",")?;
						Spaces(options.array_after_comma).fmt(f)?;
					}

					item.fmt_with_size(f, options, indent + 1, sizes, index)?;
				}
				Spaces(options.array_end).fmt(f)?;
			}
		}
	}

	f.write_str("]")
}

impl<T: PrintWithSize> PrintWithSize for Vec<T> {
	#[inline(always)]
	fn fmt_with_size(
		&self,
		f: &mut fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> fmt::Result {
		print_array(self, f, options, indent, sizes, index)
	}
}

/// Writes `entries` as a JSON object, using the layout at `sizes[*index]`.
/// # Errors
///
/// Returns [`fmt::Error`] if writing to the formatter fails.
pub fn print_object<'a, V, I: IntoIterator<Item = (&'a str, V)>>(
	entries: I,
	f: &mut fmt::Formatter,
	options: &Options,
	indent: usize,
	sizes: &[Size],
	index: &mut usize,
) -> fmt::Result
where
	I::IntoIter: ExactSizeIterator,
	V: PrintWithSize,
{
	use fmt::Display;
	let size = sizes[*index];
	*index += 1;

	f.write_str("{")?;

	let entries = entries.into_iter();
	if entries.len() == 0 {
		match size {
			Size::Expanded => {
				f.write_str("\n")?;
				options.indent.by(indent).fmt(f)?;
			}
			Size::Width(_) => Spaces(options.object_empty).fmt(f)?,
		}
	} else {
		match size {
			Size::Expanded => {
				f.write_str("\n")?;

				for (i, (key, value)) in entries.enumerate() {
					if i > 0 {
						Spaces(options.object_before_comma).fmt(f)?;
						f.write_str(",\n")?;
					}

					options.indent.by(indent + 1).fmt(f)?;

					string_literal(key, f)?;
					Spaces(options.object_before_colon).fmt(f)?;
					f.write_str(":")?;
					Spaces(options.object_after_colon).fmt(f)?;

					value.fmt_with_size(f, options, indent + 1, sizes, index)?;
				}

				f.write_str("\n")?;
				options.indent.by(indent).fmt(f)?;
			}
			Size::Width(_) => {
				Spaces(options.object_begin).fmt(f)?;
				for (i, (key, value)) in entries.enumerate() {
					if i > 0 {
						Spaces(options.object_before_comma).fmt(f)?;
						f.write_str(",")?;
						Spaces(options.object_after_comma).fmt(f)?;
					}

					string_literal(key, f)?;
					Spaces(options.object_before_colon).fmt(f)?;
					f.write_str(":")?;
					Spaces(options.object_after_colon).fmt(f)?;

					value.fmt_with_size(f, options, indent + 1, sizes, index)?;
				}
				Spaces(options.object_end).fmt(f)?;
			}
		}
	}

	f.write_str("}")
}

impl PrintWithSize for crate::Object {
	#[inline(always)]
	fn fmt_with_size(
		&self,
		f: &mut fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> fmt::Result {
		print_object(
			self.iter().map(|e| (e.key.as_str(), &e.value)),
			f,
			options,
			indent,
			sizes,
			index,
		)
	}
}

/// Computation of the printed layout, ahead of printing.
///
/// Implementations push one [`Size`] per array/object encountered, in
/// depth-first order, and return the size of the value itself.
///
/// Recurses once per nesting level; each level runs through
/// [`on_sufficient_stack`].
pub trait PrecomputeSize {
	/// Computes the printed size of this value, appending the size of each
	/// nested array and object to `sizes`.
	fn pre_compute_size(&self, options: &Options, sizes: &mut Vec<Size>) -> Size;
}

impl PrecomputeSize for bool {
	#[inline(always)]
	fn pre_compute_size(&self, _options: &Options, _sizes: &mut Vec<Size>) -> Size {
		if *self {
			Size::Width(4)
		} else {
			Size::Width(5)
		}
	}
}

impl PrecomputeSize for crate::Value {
	fn pre_compute_size(&self, options: &Options, sizes: &mut Vec<Size>) -> Size {
		match self {
			crate::Value::Null => Size::Width(4),
			crate::Value::Boolean(b) => b.pre_compute_size(options, sizes),
			crate::Value::Number(n) => Size::Width(n.as_str().len()),
			crate::Value::String(s) => Size::Width(printed_string_size(s)),
			crate::Value::Array(a) => {
				on_sufficient_stack(|| pre_compute_array_size(a, options, sizes))
			}
			crate::Value::Object(o) => on_sufficient_stack(|| {
				pre_compute_object_size(
					o.iter().map(|e| (e.key.as_str(), &e.value)),
					options,
					sizes,
				)
			}),
		}
	}
}

impl<T: PrecomputeSize + ?Sized> PrecomputeSize for &T {
	fn pre_compute_size(&self, options: &Options, sizes: &mut Vec<Size>) -> Size {
		(**self).pre_compute_size(options, sizes)
	}
}

/// Computes the printed size of an array made of `items`.
pub fn pre_compute_array_size<I: IntoIterator>(
	items: I,
	options: &Options,
	sizes: &mut Vec<Size>,
) -> Size
where
	I::Item: PrecomputeSize,
{
	let index = sizes.len();
	sizes.push(Size::Width(0));

	let mut size = Size::Width(2 + options.object_begin + options.object_end);

	let mut len = 0;
	for (i, item) in items.into_iter().enumerate() {
		if i > 0 {
			size.add(Size::Width(
				1 + options.array_before_comma + options.array_after_comma,
			));
		}

		size.add(item.pre_compute_size(options, sizes));
		len += 1;
	}

	let size = match size {
		Size::Expanded => Size::Expanded,
		Size::Width(width) => match options.array_limit {
			None => Size::Width(width),
			Some(Limit::Always) => Size::Expanded,
			Some(Limit::Item(i)) => {
				if len > i {
					Size::Expanded
				} else {
					Size::Width(width)
				}
			}
			Some(Limit::ItemOrWidth(i, w)) => {
				if len > i || width > w {
					Size::Expanded
				} else {
					Size::Width(width)
				}
			}
			Some(Limit::Width(w)) => {
				if width > w {
					Size::Expanded
				} else {
					Size::Width(width)
				}
			}
		},
	};

	sizes[index] = size;
	size
}

/// Computes the printed size of an object made of `entries`.
pub fn pre_compute_object_size<'a, V, I: IntoIterator<Item = (&'a str, V)>>(
	entries: I,
	options: &Options,
	sizes: &mut Vec<Size>,
) -> Size
where
	V: PrecomputeSize,
{
	let index = sizes.len();
	sizes.push(Size::Width(0));

	let mut size = Size::Width(2 + options.object_begin + options.object_end);

	let mut len = 0;
	for (i, (key, value)) in entries.into_iter().enumerate() {
		if i > 0 {
			size.add(Size::Width(
				1 + options.object_before_comma + options.object_after_comma,
			));
		}

		size.add(Size::Width(
			printed_string_size(key) + 1 + options.object_before_colon + options.object_after_colon,
		));
		size.add(value.pre_compute_size(options, sizes));
		len += 1;
	}

	let size = match size {
		Size::Expanded => Size::Expanded,
		Size::Width(width) => match options.object_limit {
			None => Size::Width(width),
			Some(Limit::Always) => Size::Expanded,
			Some(Limit::Item(i)) => {
				if len > i {
					Size::Expanded
				} else {
					Size::Width(width)
				}
			}
			Some(Limit::ItemOrWidth(i, w)) => {
				if len > i || width > w {
					Size::Expanded
				} else {
					Size::Width(width)
				}
			}
			Some(Limit::Width(w)) => {
				if width > w {
					Size::Expanded
				} else {
					Size::Width(width)
				}
			}
		},
	};

	sizes[index] = size;
	size
}

impl Print for crate::Value {
	fn fmt_with(&self, f: &mut fmt::Formatter, options: &Options, indent: usize) -> fmt::Result {
		match self {
			Self::Null => f.write_str("null"),
			Self::Boolean(b) => b.fmt_with(f, options, indent),
			Self::Number(n) => n.fmt_with(f, options, indent),
			Self::String(s) => s.fmt_with(f, options, indent),
			Self::Array(a) => {
				let mut sizes =
					Vec::with_capacity(self.count(|_, v| v.is_array() || v.is_object()));
				self.pre_compute_size(options, &mut sizes);
				let mut index = 0;
				a.fmt_with_size(f, options, indent, &sizes, &mut index)
			}
			Self::Object(o) => {
				let mut sizes =
					Vec::with_capacity(self.count(|_, v| v.is_array() || v.is_object()));
				self.pre_compute_size(options, &mut sizes);
				let mut index = 0;
				o.fmt_with_size(f, options, indent, &sizes, &mut index)
			}
		}
	}
}

impl PrintWithSize for crate::Value {
	fn fmt_with_size(
		&self,
		f: &mut fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> fmt::Result {
		match self {
			Self::Null => f.write_str("null"),
			Self::Boolean(b) => b.fmt_with(f, options, indent),
			Self::Number(n) => n.fmt_with(f, options, indent),
			Self::String(s) => s.fmt_with(f, options, indent),
			Self::Array(a) => {
				on_sufficient_stack(|| a.fmt_with_size(f, options, indent, sizes, index))
			}
			Self::Object(o) => {
				on_sufficient_stack(|| o.fmt_with_size(f, options, indent, sizes, index))
			}
		}
	}
}
