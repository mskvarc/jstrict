//! JSON parsing.
//!
//! [`Parse`] is the entry point: `Value::parse_str`, `Value::parse_slice` and
//! friends return the parsed value together with a
//! [`CodeMap`]. When the code map is not needed, the
//! [`parse_str_value`] / [`parse_slice_value`] functions take a faster path
//! that skips span tracking.
//!
//! Parsing is strict by default; [`Options`] relaxes surrogate and codepoint
//! handling.
use decoded_char::DecodedChar;
use locspan::{Meta, Span};
use std::fmt;

mod array;
mod boolean;
mod null;
mod number;
mod object;
pub(crate) mod slice;
mod string;
mod value;

use crate::{CodeMap, Value};

/// Parser options.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Options {
	/// Whether or not to accept a high surrogate without its low counterpart
	/// in strings.
	///
	/// In such instance, the high surrogate will be replaced with the Unicode
	/// REPLACEMENT CHARACTER, U+FFFD.
	pub accept_truncated_surrogate_pair: bool,

	/// Whether or not to accept invalid Unicode codepoints in strings.
	///
	/// Invalid codepoints will be replaced with the Unicode
	/// REPLACEMENT CHARACTER, U+FFFD.
	pub accept_invalid_codepoints: bool,
}

impl Options {
	/// Strict mode.
	///
	/// All options are set to `false`.
	pub const fn strict() -> Self {
		Self {
			accept_truncated_surrogate_pair: false,
			accept_invalid_codepoints: false,
		}
	}

	/// Flexible mode.
	///
	/// All options are set to `true`.
	pub const fn flexible() -> Self {
		Self {
			accept_truncated_surrogate_pair: true,
			accept_invalid_codepoints: true,
		}
	}
}

impl Default for Options {
	fn default() -> Self {
		Self::strict()
	}
}

/// Parses a [`Value`] from a `&str` without building a [`CodeMap`].
///
/// This is the fastest path: the slice parser is monomorphised against
/// a no-op fragment recorder, so each value is built with zero
/// `Vec::push` for span tracking.
pub fn parse_str_value(content: &str) -> Result<Value, Error> {
	parse_str_value_with(content, Options::default())
}

/// Like [`parse_str_value`] but with a custom [`Options`].
pub fn parse_str_value_with(content: &str, options: Options) -> Result<Value, Error> {
	let (value, _) = slice::parse_value_str_with(content, options, slice::NoOp)?;
	Ok(value)
}

/// Parses a [`Value`] from `&[u8]` without building a [`CodeMap`].
///
/// See [`parse_str_value`].
pub fn parse_slice_value(content: &[u8]) -> Result<Value, Error> {
	parse_slice_value_with(content, Options::default())
}

/// Like [`parse_slice_value`] but with a custom [`Options`].
pub fn parse_slice_value_with(content: &[u8], options: Options) -> Result<Value, Error> {
	let (value, _) = slice::parse_value_slice_with(content, options, slice::NoOp)?;
	Ok(value)
}

/// JSON parsing methods.
///
/// Every method returns the parsed value together with the [`CodeMap`]
/// describing where each of its fragments came from.
///
/// # Example
///
/// ```
/// use jstrict::{Parse, Value};
///
/// let (value, code_map) = Value::parse_str("[1, 2]")?;
/// assert_eq!(code_map.len(), 3); // the array and its two items.
/// # Ok::<_, jstrict::parse::Error>(())
/// ```
pub trait Parse: Sized {
	/// Parses a value from a UTF-8 byte slice.
	fn parse_slice(content: &[u8]) -> Result<(Self, CodeMap), Error> {
		Self::parse_utf8(utf8_decode::Decoder::new(content.iter().copied()))
			.map_err(Error::io_into_utf8)
	}

	/// Parses a value from a UTF-8 byte slice, with the given `options`.
	fn parse_slice_with(content: &[u8], options: Options) -> Result<(Self, CodeMap), Error> {
		Self::parse_utf8_with(utf8_decode::Decoder::new(content.iter().copied()), options)
			.map_err(Error::io_into_utf8)
	}

	/// Parses a value from a string.
	fn parse_str(content: &str) -> Result<(Self, CodeMap), Error> {
		Self::parse_utf8(content.chars().map(Ok))
	}

	/// Parses a value from a string, with the given `options`.
	fn parse_str_with(content: &str, options: Options) -> Result<(Self, CodeMap), Error> {
		Self::parse_utf8_with(content.chars().map(Ok), options)
	}

	/// Parses a value from an infallible iterator of UTF-8 characters.
	fn parse_infallible_utf8<C>(chars: C) -> Result<(Self, CodeMap), Error>
	where
		C: Iterator<Item = char>,
	{
		Self::parse_infallible(chars.map(DecodedChar::from_utf8))
	}

	/// Parses a value from an infallible iterator of UTF-8 characters, with
	/// the given `options`.
	fn parse_utf8_infallible_with<C>(chars: C, options: Options) -> Result<(Self, CodeMap), Error>
	where
		C: Iterator<Item = char>,
	{
		Self::parse_infallible_with(chars.map(DecodedChar::from_utf8), options)
	}

	/// Parses a value from a fallible iterator of UTF-8 characters.
	///
	/// Stream errors of type `E` are wrapped in [`Error::Stream`].
	fn parse_utf8<C, E>(chars: C) -> Result<(Self, CodeMap), Error<E>>
	where
		C: Iterator<Item = Result<char, E>>,
	{
		Self::parse(chars.map(|c| c.map(DecodedChar::from_utf8)))
	}

	/// Parses a value from a fallible iterator of UTF-8 characters, with the
	/// given `options`.
	fn parse_utf8_with<C, E>(chars: C, options: Options) -> Result<(Self, CodeMap), Error<E>>
	where
		C: Iterator<Item = Result<char, E>>,
	{
		Self::parse_with(chars.map(|c| c.map(DecodedChar::from_utf8)), options)
	}

	/// Parses a value from an infallible iterator of decoded characters.
	///
	/// Unlike the `utf8` variants, [`DecodedChar`] carries the byte length of
	/// each character in the *source* encoding, so spans stay accurate when
	/// the input is not UTF-8.
	fn parse_infallible<C>(chars: C) -> Result<(Self, CodeMap), Error>
	where
		C: Iterator<Item = DecodedChar>,
	{
		let mut parser = Parser::new(chars.map(Ok));
		let value = Self::parse_in(&mut parser, Context::None)?.into_value();
		Ok((value, parser.code_map))
	}

	/// Parses a value from an infallible iterator of decoded characters, with
	/// the given `options`.
	fn parse_infallible_with<C>(chars: C, options: Options) -> Result<(Self, CodeMap), Error>
	where
		C: Iterator<Item = DecodedChar>,
	{
		let mut parser = Parser::new_with(chars.map(Ok), options);
		let value = Self::parse_in(&mut parser, Context::None)?.into_value();
		Ok((value, parser.code_map))
	}

	/// Parses a value from a fallible iterator of decoded characters.
	fn parse<C, E>(chars: C) -> Result<(Self, CodeMap), Error<E>>
	where
		C: Iterator<Item = Result<DecodedChar, E>>,
	{
		let mut parser = Parser::new(chars);
		let value = Self::parse_in(&mut parser, Context::None)?.into_value();
		Ok((value, parser.code_map))
	}

	/// Parses a value from a fallible iterator of decoded characters, with the
	/// given `options`.
	fn parse_with<C, E>(chars: C, options: Options) -> Result<(Self, CodeMap), Error<E>>
	where
		C: Iterator<Item = Result<DecodedChar, E>>,
	{
		let mut parser = Parser::new_with(chars, options);
		let value = Self::parse_in(&mut parser, Context::None)?.into_value();
		Ok((value, parser.code_map))
	}

	/// Parses a value in the middle of an ongoing parse, using `parser` and
	/// the surrounding [`Context`].
	///
	/// Returns the value along with the index of its code map fragment. This
	/// is the single method implementors must provide; everything else is a
	/// wrapper around it.
	fn parse_in<C, E>(
		parser: &mut Parser<C, E>,
		context: Context,
	) -> Result<Meta<Self, usize>, Error<E>>
	where
		C: Iterator<Item = Result<DecodedChar, E>>;
}

/// JSON parser.
pub struct Parser<C: Iterator<Item = Result<DecodedChar, E>>, E> {
	/// Character stream.
	chars: C,

	/// Pending next char.
	pending: Option<DecodedChar>,

	/// Position in the stream.
	position: usize,

	/// Parser options.
	options: Options,

	/// Code-map.
	code_map: CodeMap,
}

/// Checks if the given char `c` is a JSON whitespace.
#[inline(always)]
pub const fn is_whitespace(c: char) -> bool {
	matches!(c, ' ' | '\t' | '\r' | '\n')
}

impl<C: Iterator<Item = Result<DecodedChar, E>>, E> Parser<C, E> {
	/// Creates a new parser over `chars`, in strict mode.
	pub fn new(chars: C) -> Self {
		Self {
			chars,
			pending: None,
			position: 0,
			options: Options::default(),
			code_map: CodeMap::default(),
		}
	}

	/// Creates a new parser over `chars`, with the given `options`.
	pub fn new_with(chars: C, options: Options) -> Self {
		Self {
			chars,
			pending: None,
			position: 0,
			options,
			code_map: CodeMap::default(),
		}
	}

	fn begin_fragment(&mut self) -> usize {
		self.code_map.reserve(self.position)
	}

	fn end_fragment(&mut self, i: usize) {
		let entry_count = self.code_map.len();
		let entry = self.code_map.get_mut(i).unwrap();
		entry.span.set_end(self.position);
		entry.volume = entry_count - i;
	}

	fn peek_char(&mut self) -> Result<Option<char>, Error<E>> {
		match self.pending {
			Some(c) => Ok(Some(c.chr())),
			None => match self.chars.next() {
				Some(Ok(c)) => {
					self.pending = Some(c);
					Ok(Some(c.chr()))
				}
				Some(Err(e)) => Err(Error::Stream(self.position, e)),
				None => Ok(None),
			},
		}
	}

	fn next_char(&mut self) -> Result<(usize, Option<char>), Error<E>> {
		let c = match self.pending.take() {
			Some(c) => Some(c),
			None => self
				.chars
				.next()
				.transpose()
				.map_err(|e| Error::Stream(self.position, e))?,
		};

		let p = self.position;
		let c = c.map(|c| {
			self.position += c.len();
			c.chr()
		});

		Ok((p, c))
	}

	fn skip_whitespaces(&mut self) -> Result<(), Error<E>> {
		while let Some(c) = self.peek_char()? {
			if is_whitespace(c) {
				self.next_char()?;
			} else {
				break;
			}
		}

		Ok(())
	}
}

/// Parse error.
#[derive(Debug)]
pub enum Error<E = core::convert::Infallible> {
	/// Stream error.
	///
	/// The first parameter is the byte index at which the error occurred.
	Stream(usize, E),

	/// Unexpected character or end of stream.
	///
	/// The first parameter is the byte index at which the error occurred.
	Unexpected(usize, Option<char>),

	/// Invalid unicode codepoint.
	///
	/// The first parameter is the span at which the error occurred.
	InvalidUnicodeCodePoint(Span, u32),

	/// Missing low surrogate in a string.
	///
	/// The first parameter is the byte index at which the error occurred.
	MissingLowSurrogate(Span, u16),

	/// Invalid low surrogate in a string.
	///
	/// The first parameter is the span at which the error occurred.
	InvalidLowSurrogate(Span, u16, u32),

	/// UTF-8 encoding error.
	InvalidUtf8(usize),
}

impl<E> Error<E> {
	/// Creates an `Unexpected` error.
	#[inline(always)]
	const fn unexpected(position: usize, c: Option<char>) -> Self {
		// panic!("unexpected {:?}", c);
		Self::Unexpected(position, c)
	}

	/// Returns the byte position at which the error occurred.
	pub fn position(&self) -> usize {
		match self {
			Self::Stream(p, _) => *p,
			Self::Unexpected(p, _) => *p,
			Self::InvalidUnicodeCodePoint(span, _) => span.start(),
			Self::MissingLowSurrogate(span, _) => span.start(),
			Self::InvalidLowSurrogate(span, _, _) => span.start(),
			Self::InvalidUtf8(p) => *p,
		}
	}

	/// Returns the byte span the error covers.
	///
	/// Errors that point at a single position return an empty span there.
	pub fn span(&self) -> Span {
		match self {
			Self::Stream(p, _) => Span::new(*p, *p),
			Self::Unexpected(p, _) => Span::new(*p, *p),
			Self::InvalidUnicodeCodePoint(span, _) => *span,
			Self::MissingLowSurrogate(span, _) => *span,
			Self::InvalidLowSurrogate(span, _, _) => *span,
			Self::InvalidUtf8(p) => Span::new(*p, *p),
		}
	}
}

impl Error<utf8_decode::Utf8Error> {
	const fn io_into_utf8(self) -> Error {
		match self {
			Self::Stream(p, _) => Error::InvalidUtf8(p),
			Self::Unexpected(p, e) => Error::Unexpected(p, e),
			Self::InvalidUnicodeCodePoint(s, e) => Error::InvalidUnicodeCodePoint(s, e),
			Self::MissingLowSurrogate(s, e) => Error::MissingLowSurrogate(s, e),
			Self::InvalidLowSurrogate(s, a, b) => Error::InvalidLowSurrogate(s, a, b),
			Self::InvalidUtf8(p) => Error::InvalidUtf8(p),
		}
	}
}

impl<E: fmt::Display> fmt::Display for Error<E> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Stream(_, e) => e.fmt(f),
			Self::Unexpected(_, Some(c)) => write!(f, "unexpected character `{}`", c),
			Self::Unexpected(_, None) => write!(f, "unexpected end of file"),
			Self::InvalidUnicodeCodePoint(_, c) => write!(f, "invalid Unicode code point {:x}", *c),
			Self::MissingLowSurrogate(_, _) => write!(f, "missing low surrogate"),
			Self::InvalidLowSurrogate(_, _, _) => write!(f, "invalid low surrogate"),
			Self::InvalidUtf8(_) => write!(f, "invalid UTF-8"),
		}
	}
}

impl<E: 'static + std::error::Error> std::error::Error for Error<E> {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::Stream(_, e) => Some(e),
			_ => None,
		}
	}
}

/// Parsing context.
///
/// Defines what characters are allowed after a value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Context {
	/// Top level: only whitespace may follow the value.
	None,

	/// Inside an array: `,` or `]` may follow the value.
	Array,

	/// An object key: `:` may follow the value.
	ObjectKey,

	/// An object entry value: `,` or `}` may follow the value.
	ObjectValue,
}

impl Context {
	/// Checks if the given character `c` can follow a value in this context.
	pub const fn follows(&self, c: char) -> bool {
		match self {
			Self::None => is_whitespace(c),
			Self::Array => is_whitespace(c) || matches!(c, ',' | ']'),
			Self::ObjectKey => is_whitespace(c) || matches!(c, ':'),
			Self::ObjectValue => is_whitespace(c) || matches!(c, ',' | '}'),
		}
	}
}
