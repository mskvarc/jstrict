//! Byte-slice JSON parser.
//!
//! Replaces the char-iterator [`Parser`](super::Parser) on the `parse_str` /
//! `parse_slice` entry points. Because the source is already in memory as
//! `&[u8]`, we can:
//!
//! * scan strings and numbers byte-wise (no per-char `Result` dispatch),
//! * copy un-escaped string runs as slices (no per-char `push`),
//! * decode keywords with a single 4/5-byte compare,
//! * leave UTF-8 validation lazy / SIMD-accelerated.
//!
//! The slice parser is generic over a [`Mapper`] so callers that want a
//! [`CodeMap`] pay for it, and callers that don't get a no-op trait method
//! that monomorphises away.

use crate::{
	Array, CodeMap, NumberBuf, Object, String as JsonString, Value, object::Key, parse::Options,
};
use locspan::Span;

use super::Error;

/// Hook for fragment span tracking. The slice parser is generic over `Mapper`
/// so [`Recording`] (populates a [`CodeMap`]) and [`NoOp`] both produce
/// dedicated, fully-monomorphized parsers.
pub(crate) trait Mapper {
	fn begin_fragment(&mut self, pos: usize) -> usize;
	fn end_fragment(&mut self, idx: usize, end_pos: usize);
}

/// `Mapper` impl that builds a [`CodeMap`] (the public mapped path).
pub(crate) struct Recording {
	pub code_map: CodeMap,
}

impl Recording {
	pub fn new() -> Self {
		Self {
			code_map: CodeMap::default(),
		}
	}
}

impl Mapper for Recording {
	#[inline]
	fn begin_fragment(&mut self, pos: usize) -> usize {
		self.code_map.reserve(pos)
	}

	#[inline]
	fn end_fragment(&mut self, idx: usize, end_pos: usize) {
		let entry_count = self.code_map.len();
		let entry = self.code_map.get_mut(idx).unwrap();
		entry.span.set_end(end_pos);
		entry.volume = entry_count - idx;
	}
}

/// `Mapper` impl that records nothing (used by the `_value` entry points).
pub(crate) struct NoOp;

impl Mapper for NoOp {
	#[inline]
	fn begin_fragment(&mut self, _pos: usize) -> usize {
		0
	}
	#[inline]
	fn end_fragment(&mut self, _idx: usize, _end_pos: usize) {}
}

enum StackItem {
	Array {
		array: Array,
		frag: usize,
	},
	ArrayItem {
		array: Array,
		frag: usize,
	},
	Object {
		object: Object,
		frag: usize,
	},
	ObjectEntry {
		object: Object,
		frag: usize,
		key: Key,
		entry_frag: usize,
	},
}

enum Begin {
	Value(Value),
	ArrayItem {
		frag: usize,
	},
	ObjectEntry {
		frag: usize,
		key: Key,
		entry_frag: usize,
	},
}

pub(crate) struct SliceParser<'a, M: Mapper> {
	input: &'a [u8],
	pos: usize,
	options: Options,
	mapper: M,
}

pub(crate) fn parse_value_str_with<M: Mapper>(
	input: &str,
	options: Options,
	mapper: M,
) -> Result<(Value, M), Error> {
	let mut parser = SliceParser {
		input: input.as_bytes(),
		pos: 0,
		options,
		mapper,
	};
	let value = parser.parse_root()?;
	Ok((value, parser.mapper))
}

pub(crate) fn parse_value_slice_with<M: Mapper>(
	input: &[u8],
	options: Options,
	mapper: M,
) -> Result<(Value, M), Error> {
	#[cfg(feature = "simdutf8")]
	let s = simdutf8::basic::from_utf8(input).map_err(|_| Error::InvalidUtf8(0))?;
	#[cfg(not(feature = "simdutf8"))]
	let s = core::str::from_utf8(input).map_err(|e| Error::InvalidUtf8(e.valid_up_to()))?;
	parse_value_str_with(s, options, mapper)
}

impl<'a, M: Mapper> SliceParser<'a, M> {
	#[inline]
	fn peek(&self) -> Option<u8> {
		self.input.get(self.pos).copied()
	}

	#[inline]
	fn skip_whitespace(&mut self) {
		let bytes = self.input;
		let mut i = self.pos;
		while i < bytes.len() {
			match bytes[i] {
				b' ' | b'\t' | b'\r' | b'\n' => i += 1,
				_ => break,
			}
		}
		self.pos = i;
	}

	#[inline]
	fn next_byte(&mut self) -> Result<(usize, u8), Error> {
		match self.input.get(self.pos) {
			Some(&b) => {
				let p = self.pos;
				self.pos += 1;
				Ok((p, b))
			}
			None => Err(Error::Unexpected(self.pos, None)),
		}
	}

	/// Returns the `char` at `byte_pos`, decoding UTF-8 if needed. Used only
	/// for error reporting.
	fn char_at(&self, byte_pos: usize) -> Option<char> {
		if byte_pos >= self.input.len() {
			return None;
		}
		match core::str::from_utf8(&self.input[byte_pos..]) {
			Ok(s) => s.chars().next(),
			Err(_) => Some(self.input[byte_pos] as char),
		}
	}

	#[inline]
	fn unexpected_at(&self, p: usize) -> Error {
		Error::Unexpected(p, self.char_at(p))
	}

	fn expect_keyword(&mut self, kw: &[u8]) -> Result<(), Error> {
		let end = self.pos + kw.len();
		if end <= self.input.len() && &self.input[self.pos..end] == kw {
			self.pos = end;
			Ok(())
		} else {
			let mut p = self.pos;
			let max = end.min(self.input.len());
			while p < max && self.input[p] == kw[p - self.pos] {
				p += 1;
			}
			Err(self.unexpected_at(p))
		}
	}

	#[inline]
	fn validate_str_slice(&self, start: usize, end: usize) -> Result<&'a str, Error> {
		// SAFETY: input is validated as UTF-8 upfront for slice entry points,
		// or comes from a `&str` directly.
		Ok(unsafe { core::str::from_utf8_unchecked(&self.input[start..end]) })
	}

	fn parse_string(&mut self) -> Result<JsonString, Error> {
		debug_assert_eq!(self.input.get(self.pos), Some(&b'"'));
		self.pos += 1;
		let start = self.pos;
		let bytes = self.input;
		match memchr::memchr2(b'"', b'\\', &bytes[start..]) {
			Some(off) => {
				let end = start + off;
				if let Some(bad) = crate::byte_scan::find_lt_0x20(&bytes[start..end]) {
					return Err(self.unexpected_at(start + bad));
				}
				if bytes[end] == b'"' {
					let s = self.validate_str_slice(start, end)?;
					let out = JsonString::from_str(s);
					self.pos = end + 1;
					Ok(out)
				} else {
					self.parse_string_with_escapes(start, end)
				}
			}
			None => Err(Error::Unexpected(bytes.len(), None)),
		}
	}

	fn parse_string_with_escapes(
		&mut self,
		start: usize,
		first_escape: usize,
	) -> Result<JsonString, Error> {
		let mut out = JsonString::new();
		if first_escape > start {
			let prefix = self.validate_str_slice(start, first_escape)?;
			out.push_str(prefix);
		}
		self.pos = first_escape;

		let mut high_surrogate: Option<(usize, u32)> = None;
		loop {
			let run_start = self.pos;
			let bytes = self.input;
			let (j, hit): (usize, Option<u8>) =
				match crate::byte_scan::find_escape(&bytes[run_start..]) {
					Some(off) => (run_start + off, Some(bytes[run_start + off])),
					None => (bytes.len(), None),
				};
			if j > run_start {
				if let Some((p_high, high)) = high_surrogate.take() {
					if self.options.accept_truncated_surrogate_pair {
						out.push('\u{fffd}');
					} else {
						return Err(Error::MissingLowSurrogate(
							Span::new(p_high, j),
							high as u16,
						));
					}
				}
				let run = self.validate_str_slice(run_start, j)?;
				out.push_str(run);
				self.pos = j;
			}
			match hit {
				Some(b'"') => {
					if let Some((p_high, high)) = high_surrogate {
						if self.options.accept_truncated_surrogate_pair {
							out.push('\u{fffd}');
						} else {
							return Err(Error::MissingLowSurrogate(
								Span::new(p_high, self.pos),
								high as u16,
							));
						}
					}
					self.pos += 1;
					return Ok(out);
				}
				Some(b'\\') => {
					self.pos += 1;
					let (esc_pos, esc) = self.next_byte()?;
					match esc {
						b'"' => out.push('"'),
						b'\\' => out.push('\\'),
						b'/' => out.push('/'),
						b'b' => out.push('\u{0008}'),
						b't' => out.push('\u{0009}'),
						b'n' => out.push('\u{000a}'),
						b'f' => out.push('\u{000c}'),
						b'r' => out.push('\u{000d}'),
						b'u' => {
							let codepoint = self.parse_hex4()?;
							let p_u = esc_pos;
							let decoded = match high_surrogate.take() {
								Some((p_high, high)) => {
									if (0xdc00..=0xdfff).contains(&codepoint) {
										let low = codepoint;
										let cp =
											((high - 0xd800) << 10 | (low - 0xdc00)) + 0x010000;
										match char::from_u32(cp) {
											Some(c) => Some(c),
											None => {
												if self.options.accept_invalid_codepoints {
													Some('\u{fffd}')
												} else {
													return Err(Error::InvalidUnicodeCodePoint(
														Span::new(p_high, self.pos),
														cp,
													));
												}
											}
										}
									} else if self.options.accept_truncated_surrogate_pair {
										out.push('\u{fffd}');
										match char::from_u32(codepoint) {
											Some(c) => Some(c),
											None => {
												if self.options.accept_invalid_codepoints {
													Some('\u{fffd}')
												} else {
													return Err(Error::InvalidUnicodeCodePoint(
														Span::new(p_u, self.pos),
														codepoint,
													));
												}
											}
										}
									} else {
										return Err(Error::InvalidLowSurrogate(
											Span::new(p_u, self.pos),
											high as u16,
											codepoint,
										));
									}
								}
								None => {
									if (0xd800..=0xdbff).contains(&codepoint) {
										high_surrogate = Some((p_u, codepoint));
										None
									} else {
										match char::from_u32(codepoint) {
											Some(c) => Some(c),
											None => {
												if self.options.accept_invalid_codepoints {
													Some('\u{fffd}')
												} else {
													return Err(Error::InvalidUnicodeCodePoint(
														Span::new(p_u, self.pos),
														codepoint,
													));
												}
											}
										}
									}
								}
							};
							if let Some(c) = decoded {
								if let Some((p_high, high)) = high_surrogate.take() {
									if self.options.accept_truncated_surrogate_pair {
										out.push('\u{fffd}');
									} else {
										return Err(Error::MissingLowSurrogate(
											Span::new(p_high, self.pos),
											high as u16,
										));
									}
								}
								out.push(c);
							}
						}
						_ => return Err(Error::Unexpected(esc_pos, Some(esc as char))),
					}
				}
				Some(_) => return Err(self.unexpected_at(self.pos)),
				None => return Err(Error::Unexpected(bytes.len(), None)),
			}
		}
	}

	fn parse_hex4(&mut self) -> Result<u32, Error> {
		let mut acc = 0u32;
		for _ in 0..4 {
			let (p, b) = self.next_byte()?;
			let d = match b {
				b'0'..=b'9' => (b - b'0') as u32,
				b'a'..=b'f' => (b - b'a' + 10) as u32,
				b'A'..=b'F' => (b - b'A' + 10) as u32,
				_ => return Err(Error::Unexpected(p, Some(b as char))),
			};
			acc = (acc << 4) | d;
		}
		Ok(acc)
	}

	fn parse_number(&mut self, ctx: super::Context) -> Result<NumberBuf, Error> {
		let start = self.pos;
		let bytes = self.input;
		let mut i = start;
		if bytes.get(i) == Some(&b'-') {
			i += 1;
		}
		match bytes.get(i) {
			Some(&b'0') => {
				i += 1;
			}
			Some(b) if (b'1'..=b'9').contains(b) => {
				i += 1;
				while let Some(&b) = bytes.get(i) {
					if !b.is_ascii_digit() {
						break;
					}
					i += 1;
				}
			}
			_ => return Err(Error::Unexpected(i, self.char_at(i))),
		}
		if matches!(bytes.get(i), Some(&b'.') | Some(&b'e') | Some(&b'E')) {
			return self.parse_number_slow(ctx, start);
		}
		let term_ok = match bytes.get(i) {
			None => true,
			Some(&c) => ctx.follows(c as char),
		};
		if !term_ok {
			return Err(Error::Unexpected(i, self.char_at(i)));
		}
		self.pos = i;
		let buf: smallvec::SmallVec<[u8; crate::SMALL_STRING_CAPACITY]> =
			smallvec::SmallVec::from_slice(&bytes[start..i]);
		// SAFETY: the fast path consumed a valid JSON integer.
		Ok(unsafe { NumberBuf::new_unchecked(buf) })
	}

	fn parse_number_slow(&mut self, ctx: super::Context, start: usize) -> Result<NumberBuf, Error> {
		let bytes = self.input;
		let mut i = start;

		enum State {
			Init,
			FirstDigit,
			Zero,
			NonZero,
			FractionalFirst,
			FractionalRest,
			ExponentSign,
			ExponentFirst,
			ExponentRest,
		}

		let mut state = State::Init;
		while i < bytes.len() {
			let b = bytes[i];
			let c = b as char;
			match state {
				State::Init => match b {
					b'-' => state = State::FirstDigit,
					b'0' => state = State::Zero,
					b'1'..=b'9' => state = State::NonZero,
					_ => return Err(Error::Unexpected(i, Some(c))),
				},
				State::FirstDigit => match b {
					b'0' => state = State::Zero,
					b'1'..=b'9' => state = State::NonZero,
					_ => return Err(Error::Unexpected(i, Some(c))),
				},
				State::Zero => match b {
					b'.' => state = State::FractionalFirst,
					b'e' | b'E' => state = State::ExponentSign,
					_ => {
						if ctx.follows(c) {
							break;
						}
						return Err(Error::Unexpected(i, Some(c)));
					}
				},
				State::NonZero => match b {
					b'0'..=b'9' => state = State::NonZero,
					b'.' => state = State::FractionalFirst,
					b'e' | b'E' => state = State::ExponentSign,
					_ => {
						if ctx.follows(c) {
							break;
						}
						return Err(Error::Unexpected(i, Some(c)));
					}
				},
				State::FractionalFirst => match b {
					b'0'..=b'9' => state = State::FractionalRest,
					_ => return Err(Error::Unexpected(i, Some(c))),
				},
				State::FractionalRest => match b {
					b'0'..=b'9' => state = State::FractionalRest,
					b'e' | b'E' => state = State::ExponentSign,
					_ => {
						if ctx.follows(c) {
							break;
						}
						return Err(Error::Unexpected(i, Some(c)));
					}
				},
				State::ExponentSign => match b {
					b'+' | b'-' => state = State::ExponentFirst,
					b'0'..=b'9' => state = State::ExponentRest,
					_ => return Err(Error::Unexpected(i, Some(c))),
				},
				State::ExponentFirst => match b {
					b'0'..=b'9' => state = State::ExponentRest,
					_ => return Err(Error::Unexpected(i, Some(c))),
				},
				State::ExponentRest => match b {
					b'0'..=b'9' => state = State::ExponentRest,
					_ => {
						if ctx.follows(c) {
							break;
						}
						return Err(Error::Unexpected(i, Some(c)));
					}
				},
			}
			i += 1;
		}

		let terminating = matches!(
			state,
			State::Zero | State::NonZero | State::FractionalRest | State::ExponentRest
		);

		if !terminating {
			return Err(Error::Unexpected(i, self.char_at(i)));
		}

		self.pos = i;
		let buf: smallvec::SmallVec<[u8; crate::SMALL_STRING_CAPACITY]> =
			smallvec::SmallVec::from_slice(&bytes[start..i]);
		// SAFETY: the state machine guarantees a valid JSON number.
		Ok(unsafe { NumberBuf::new_unchecked(buf) })
	}

	/// Parses the next value-fragment header. For containers, opens the
	/// container and returns the appropriate stack item info; for
	/// primitives, fully consumes them and returns the [`Value`].
	fn begin(&mut self, ctx: super::Context) -> Result<Begin, Error> {
		self.skip_whitespace();
		let frag = self.mapper.begin_fragment(self.pos);
		match self.peek() {
			Some(b'n') => {
				self.expect_keyword(b"null")?;
				self.mapper.end_fragment(frag, self.pos);
				Ok(Begin::Value(Value::Null))
			}
			Some(b't') => {
				self.expect_keyword(b"true")?;
				self.mapper.end_fragment(frag, self.pos);
				Ok(Begin::Value(Value::Boolean(true)))
			}
			Some(b'f') => {
				self.expect_keyword(b"false")?;
				self.mapper.end_fragment(frag, self.pos);
				Ok(Begin::Value(Value::Boolean(false)))
			}
			Some(b'"') => {
				let s = self.parse_string()?;
				self.mapper.end_fragment(frag, self.pos);
				Ok(Begin::Value(Value::String(s)))
			}
			Some(b'-' | b'0'..=b'9') => {
				let n = self.parse_number(ctx)?;
				self.mapper.end_fragment(frag, self.pos);
				Ok(Begin::Value(Value::Number(n)))
			}
			Some(b'[') => {
				self.pos += 1;
				self.skip_whitespace();
				if self.peek() == Some(b']') {
					self.pos += 1;
					self.mapper.end_fragment(frag, self.pos);
					Ok(Begin::Value(Value::Array(Array::new())))
				} else {
					Ok(Begin::ArrayItem { frag })
				}
			}
			Some(b'{') => {
				self.pos += 1;
				self.skip_whitespace();
				if self.peek() == Some(b'}') {
					self.pos += 1;
					// Mirror legacy: empty objects do NOT receive
					// `end_fragment` (the reserved entry keeps span(p,p)
					// and volume 0). Empty arrays do — see the `[` arm.
					Ok(Begin::Value(Value::Object(Object::new())))
				} else {
					let (key, entry_frag) = self.read_entry_header()?;
					Ok(Begin::ObjectEntry {
						frag,
						key,
						entry_frag,
					})
				}
			}
			Some(b) => Err(Error::Unexpected(self.pos, Some(b as char))),
			None => Err(Error::Unexpected(self.pos, None)),
		}
	}

	/// Reads the start of an object entry: position is at the opening
	/// quote of the key. Reserves the entry fragment, the key fragment,
	/// parses the key string, ends the key fragment, and consumes the `:`.
	fn read_entry_header(&mut self) -> Result<(Key, usize), Error> {
		let entry_frag = self.mapper.begin_fragment(self.pos);
		let key_frag = self.mapper.begin_fragment(self.pos);
		if self.peek() != Some(b'"') {
			return Err(self.unexpected_at(self.pos));
		}
		let key: Key = self.parse_string()?;
		self.mapper.end_fragment(key_frag, self.pos);
		self.skip_whitespace();
		match self.next_byte()? {
			(_, b':') => Ok((key, entry_frag)),
			(p, b) => Err(Error::Unexpected(p, Some(b as char))),
		}
	}

	fn parse_root(&mut self) -> Result<Value, Error> {
		let mut stack: Vec<StackItem> = Vec::with_capacity(8);
		let mut value: Option<Value> = None;

		loop {
			match stack.pop() {
				None => match value.take() {
					Some(v) => {
						self.skip_whitespace();
						return match self.peek() {
							Some(b) => Err(Error::Unexpected(self.pos, Some(b as char))),
							None => Ok(v),
						};
					}
					None => match self.begin(super::Context::None)? {
						Begin::Value(v) => value = Some(v),
						Begin::ArrayItem { frag } => stack.push(StackItem::ArrayItem {
							array: Array::new(),
							frag,
						}),
						Begin::ObjectEntry {
							frag,
							key,
							entry_frag,
						} => stack.push(StackItem::ObjectEntry {
							object: Object::new(),
							frag,
							key,
							entry_frag,
						}),
					},
				},
				Some(StackItem::Array { array, frag }) => {
					self.skip_whitespace();
					match self.next_byte()? {
						(_, b',') => stack.push(StackItem::ArrayItem { array, frag }),
						(_, b']') => {
							self.mapper.end_fragment(frag, self.pos);
							value = Some(Value::Array(array));
						}
						(p, b) => return Err(Error::Unexpected(p, Some(b as char))),
					}
				}
				Some(StackItem::ArrayItem { mut array, frag }) => {
					let v = match value.take() {
						Some(v) => v,
						None => match self.begin(super::Context::Array)? {
							Begin::Value(v) => v,
							Begin::ArrayItem { frag: f2 } => {
								stack.push(StackItem::ArrayItem { array, frag });
								stack.push(StackItem::ArrayItem {
									array: Array::new(),
									frag: f2,
								});
								continue;
							}
							Begin::ObjectEntry {
								frag: f2,
								key: k2,
								entry_frag: e2,
							} => {
								stack.push(StackItem::ArrayItem { array, frag });
								stack.push(StackItem::ObjectEntry {
									object: Object::new(),
									frag: f2,
									key: k2,
									entry_frag: e2,
								});
								continue;
							}
						},
					};
					array.push(v);
					stack.push(StackItem::Array { array, frag });
				}
				Some(StackItem::Object { object, frag }) => {
					self.skip_whitespace();
					match self.next_byte()? {
						(_, b',') => {
							self.skip_whitespace();
							let (key, entry_frag) = self.read_entry_header()?;
							stack.push(StackItem::ObjectEntry {
								object,
								frag,
								key,
								entry_frag,
							});
						}
						(_, b'}') => {
							self.mapper.end_fragment(frag, self.pos);
							value = Some(Value::Object(object));
						}
						(p, b) => return Err(Error::Unexpected(p, Some(b as char))),
					}
				}
				Some(StackItem::ObjectEntry {
					mut object,
					frag,
					key,
					entry_frag,
				}) => {
					let v = match value.take() {
						Some(v) => v,
						None => match self.begin(super::Context::ObjectValue)? {
							Begin::Value(v) => v,
							Begin::ArrayItem { frag: f2 } => {
								stack.push(StackItem::ObjectEntry {
									object,
									frag,
									key,
									entry_frag,
								});
								stack.push(StackItem::ArrayItem {
									array: Array::new(),
									frag: f2,
								});
								continue;
							}
							Begin::ObjectEntry {
								frag: f2,
								key: k2,
								entry_frag: e2,
							} => {
								stack.push(StackItem::ObjectEntry {
									object,
									frag,
									key,
									entry_frag,
								});
								stack.push(StackItem::ObjectEntry {
									object: Object::new(),
									frag: f2,
									key: k2,
									entry_frag: e2,
								});
								continue;
							}
						},
					};
					self.mapper.end_fragment(entry_frag, self.pos);
					object.push(key, v);
					stack.push(StackItem::Object { object, frag });
				}
			}
		}
	}
}
