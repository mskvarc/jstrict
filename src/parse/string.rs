use super::{Context, Error, Parse, Parser};
use compact_str::CompactString;
use decoded_char::DecodedChar;
use locspan::Span;

fn is_control(c: char) -> bool {
	('\u{0000}'..='\u{001f}').contains(&c)
}

fn parse_hex4<C, E>(parser: &mut Parser<C, E>) -> Result<u32, Error<E>>
where
	C: Iterator<Item = Result<DecodedChar, E>>,
{
	match parser.next_char()? {
		(p, Some(c)) => match c.to_digit(16) {
			Some(h3) => match parser.next_char()? {
				(p, Some(c)) => match c.to_digit(16) {
					Some(h2) => match parser.next_char()? {
						(p, Some(c)) => match c.to_digit(16) {
							Some(h1) => match parser.next_char()? {
								(p, Some(c)) => match c.to_digit(16) {
									Some(h0) => Ok(h3 << 12 | h2 << 8 | h1 << 4 | h0),
									None => Err(Error::unexpected(p, Some(c))),
								},
								(p, unexpected) => Err(Error::unexpected(p, unexpected)),
							},
							None => Err(Error::unexpected(p, Some(c))),
						},
						(p, unexpected) => Err(Error::unexpected(p, unexpected)),
					},
					None => Err(Error::unexpected(p, Some(c))),
				},
				(p, unexpected) => Err(Error::unexpected(p, unexpected)),
			},
			None => Err(Error::unexpected(p, Some(c))),
		},
		(p, unexpected) => Err(Error::unexpected(p, unexpected)),
	}
}

fn decode_unicode_escape<C, E>(
	parser: &mut Parser<C, E>,
	result: &mut String,
	high_surrogate: &mut Option<(usize, u32)>,
	p: usize,
) -> Result<Option<char>, Error<E>>
where
	C: Iterator<Item = Result<DecodedChar, E>>,
{
	let codepoint = parse_hex4(parser)?;
	if let Some((p_high, high)) = high_surrogate.take() {
		if (0xdc00..=0xdfff).contains(&codepoint) {
			let low = codepoint;
			let codepoint = ((high - 0xd800) << 10 | (low - 0xdc00)) + 0x01_0000;
			match char::from_u32(codepoint) {
				Some(c) => Ok(Some(c)),
				None => {
					if parser.options.accept_invalid_codepoints {
						Ok(Some('\u{fffd}'))
					} else {
						Err(Error::InvalidUnicodeCodePoint(
							Span::new(p_high, parser.position),
							codepoint,
						))
					}
				}
			}
		} else if parser.options.accept_truncated_surrogate_pair {
			result.push('\u{fffd}');
			match char::from_u32(codepoint) {
				Some(c) => Ok(Some(c)),
				None => {
					if parser.options.accept_invalid_codepoints {
						Ok(Some('\u{fffd}'))
					} else {
						Err(Error::InvalidUnicodeCodePoint(
							Span::new(p, parser.position),
							codepoint,
						))
					}
				}
			}
		} else {
			Err(Error::InvalidLowSurrogate(
				Span::new(p, parser.position),
				high,
				codepoint,
			))
		}
	} else if (0xd800..=0xdbff).contains(&codepoint) {
		*high_surrogate = Some((p, codepoint));
		Ok(None)
	} else {
		match char::from_u32(codepoint) {
			Some(c) => Ok(Some(c)),
			None => {
				if parser.options.accept_invalid_codepoints {
					Ok(Some('\u{fffd}'))
				} else {
					Err(Error::InvalidUnicodeCodePoint(
						Span::new(p, parser.position),
						codepoint,
					))
				}
			}
		}
	}
}

impl Parse for CompactString {
	fn parse_in<C, E>(
		parser: &mut Parser<C, E>,
		_context: Context,
	) -> Result<(Self, usize), Error<E>>
	where
		C: Iterator<Item = Result<DecodedChar, E>>,
	{
		let i = parser.begin_fragment();
		match parser.next_char()? {
			(_, Some('"')) => {
				// Assemble in a plain `String`, then build the `CompactString`
				// once: a `CompactString` pays an inline-vs-heap check on every
				// `push`, which dominates this per-character loop.
				let mut result = String::new();
				let mut high_surrogate: Option<(usize, u32)> = None;
				loop {
					let c = match parser.next_char()? {
						(p, Some('"')) => {
							if let Some((p_high, high)) = high_surrogate {
								if parser.options.accept_truncated_surrogate_pair {
									result.push('\u{fffd}');
								} else {
									break Err(Error::MissingLowSurrogate(
										Span::new(p_high, p),
										high,
									));
								}
							}

							parser.end_fragment(i);
							break Ok((Self::new(result.as_str()), i));
						}
						(_, Some('\\')) => match parser.next_char()? {
							(_, Some(c @ ('"' | '\\' | '/'))) => c,
							(_, Some('b')) => '\u{0008}',
							(_, Some('t')) => '\u{0009}',
							(_, Some('n')) => '\u{000a}',
							(_, Some('f')) => '\u{000c}',
							(_, Some('r')) => '\u{000d}',
							(p, Some('u')) => {
								match decode_unicode_escape(
									parser,
									&mut result,
									&mut high_surrogate,
									p,
								)? {
									Some(c) => c,
									None => continue,
								}
							}
							(p, unexpected) => break Err(Error::unexpected(p, unexpected)),
						},
						(_, Some(c)) if !is_control(c) => c,
						(p, unexpected) => break Err(Error::unexpected(p, unexpected)),
					};

					if let Some((p_high, high)) = high_surrogate.take() {
						if parser.options.accept_truncated_surrogate_pair {
							result.push('\u{fffd}');
						} else {
							break Err(Error::MissingLowSurrogate(
								Span::new(p_high, parser.position),
								high,
							));
						}
					}

					result.push(c);
				}
			}
			(p, unexpected) => Err(Error::unexpected(p, unexpected)),
		}
	}
}
