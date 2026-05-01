//! SWAR/memchr byte scanners shared by the slice parser and printer.
//!
//! All routines walk u64 chunks via SWAR (SIMD-Within-A-Register) tricks so
//! the compiler can autovectorise further on x86_64 / aarch64 release builds.

const LO_BITS: u64 = 0x0101_0101_0101_0101;
const HI_BITS: u64 = 0x8080_8080_8080_8080;

/// Word-wise mask: bit 7 of every byte where `byte < 0x20`.
#[inline(always)]
fn mask_lt_0x20(v: u64) -> u64 {
	v.wrapping_sub(0x20 * LO_BITS) & !v & HI_BITS
}

/// Word-wise mask: bit 7 of every byte equal to `c`.
#[inline(always)]
fn mask_eq(v: u64, c: u8) -> u64 {
	let x = v ^ (c as u64 * LO_BITS);
	x.wrapping_sub(LO_BITS) & !x & HI_BITS
}

/// Returns the index (in bytes from the start of `s`) of the first byte
/// `< 0x20`, or `None`.
pub fn find_lt_0x20(s: &[u8]) -> Option<usize> {
	let mut i = 0;
	while i + 8 <= s.len() {
		let v = u64::from_le_bytes(s[i..i + 8].try_into().unwrap());
		let m = mask_lt_0x20(v);
		if m != 0 {
			return Some(i + (m.trailing_zeros() / 8) as usize);
		}
		i += 8;
	}
	while i < s.len() {
		if s[i] < 0x20 {
			return Some(i);
		}
		i += 1;
	}
	None
}

/// Returns the index of the first byte in `{ b'"', b'\\', 0x00..=0x1f }`,
/// or `None`.
pub fn find_escape(s: &[u8]) -> Option<usize> {
	let mut i = 0;
	while i + 8 <= s.len() {
		let v = u64::from_le_bytes(s[i..i + 8].try_into().unwrap());
		let m = mask_lt_0x20(v) | mask_eq(v, b'"') | mask_eq(v, b'\\');
		if m != 0 {
			return Some(i + (m.trailing_zeros() / 8) as usize);
		}
		i += 8;
	}
	while i < s.len() {
		let b = s[i];
		if b < 0x20 || b == b'"' || b == b'\\' {
			return Some(i);
		}
		i += 1;
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lt_0x20_none() {
		assert_eq!(find_lt_0x20(b"hello world, this is fine."), None);
	}

	#[test]
	fn lt_0x20_tail() {
		let mut s = b"abcdefghijklmno".to_vec();
		s.push(0x07);
		assert_eq!(find_lt_0x20(&s), Some(15));
	}

	#[test]
	fn lt_0x20_first_chunk() {
		let s = b"abc\x01defghij";
		assert_eq!(find_lt_0x20(s), Some(3));
	}

	#[test]
	fn escape_quote() {
		assert_eq!(find_escape(b"abcdefgh\"ijkl"), Some(8));
	}

	#[test]
	fn escape_backslash() {
		assert_eq!(find_escape(b"abcdef\\"), Some(6));
	}

	#[test]
	fn escape_ctrl() {
		assert_eq!(find_escape(b"abc\nxxxxxxxxxxx"), Some(3));
	}

	#[test]
	fn escape_none() {
		assert_eq!(find_escape(b"plain ascii text"), None);
	}
}
