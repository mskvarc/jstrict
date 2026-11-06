use crate::number::Number;
use smallvec::SmallVec;

/// The value of a JSON number, apart from how it is written.
///
/// `150`, `150.00`, `1.5E2` and `15000e-2` have the same decimal value. Zero
/// has a single value, whether it is written `0`, `-0` or `0.0e7`.
#[derive(PartialEq, Eq, Debug)]
pub(super) struct Decimal {
	/// Whether the value is below zero. Never set for zero.
	negative: bool,

	/// The significant digits as ASCII, without leading or trailing zeros.
	/// Empty for zero.
	digits: SmallVec<[u8; 24]>,

	/// The power of ten that scales the significant digits to the value.
	scale: i64,
}

impl Decimal {
	/// Reads the decimal value of `number`.
	///
	/// Returns `None` when the value is not zero and its scale does not fit an
	/// `i64`, which only an exponent of nineteen digits or more can cause.
	pub(super) fn of(number: &Number) -> Option<Decimal> {
		let bytes = number.as_bytes();
		let (negative, unsigned) = match bytes.split_first() {
			Some((b'-', unsigned)) => (true, unsigned),
			Some(_) | None => (false, bytes),
		};
		let mut parts = unsigned.splitn(2, |byte| matches!(byte, b'e' | b'E'));
		let mantissa = parts.next().unwrap_or_default();
		let exponent = parts.next().unwrap_or_default();
		let mut parts = mantissa.splitn(2, |byte| *byte == b'.');
		let integer = parts.next().unwrap_or_default();
		let fraction = parts.next().unwrap_or_default();

		let mut digits: SmallVec<[u8; 24]> = integer
			.iter()
			.chain(fraction)
			.copied()
			.skip_while(|digit| *digit == b'0')
			.collect();
		let trailing_zeros = digits
			.iter()
			.rev()
			.take_while(|digit| **digit == b'0')
			.count();
		digits.truncate(digits.len() - trailing_zeros);
		if digits.is_empty() {
			return Some(Decimal {
				negative: false,
				digits,
				scale: 0,
			});
		}

		let scale = exponent_of(exponent)?
			.checked_sub(i64::try_from(fraction.len()).ok()?)?
			.checked_add(i64::try_from(trailing_zeros).ok()?)?;
		Some(Decimal {
			negative,
			digits,
			scale,
		})
	}
}

/// Reads the exponent part of a number, the text after its `e` or `E`, as
/// `0` when the number has none.
///
/// Returns `None` when the exponent does not fit an `i64`.
fn exponent_of(exponent: &[u8]) -> Option<i64> {
	let (negative, digits) = match exponent.split_first() {
		Some((b'-', digits)) => (true, digits),
		Some((b'+', digits)) => (false, digits),
		Some(_) | None => (false, exponent),
	};
	digits.iter().try_fold(0_i64, |value, digit| {
		let digit = i64::from(digit - b'0');
		let value = value.checked_mul(10)?;
		if negative {
			value.checked_sub(digit)
		} else {
			value.checked_add(digit)
		}
	})
}

#[cfg(test)]
mod tests {
	use crate::number::{Number, decimal::Decimal};

	fn decimal(text: &str) -> Option<Decimal> {
		Decimal::of(Number::new(text).unwrap())
	}

	macro_rules! same_value_tests {
		{ $($id:ident: $a:literal == $b:literal),* } => {
			$(
				#[test]
				fn $id() {
					assert_eq!(decimal($a).unwrap(), decimal($b).unwrap());
				}
			)*
		};
	}

	macro_rules! different_value_tests {
		{ $($id:ident: $a:literal != $b:literal),* } => {
			$(
				#[test]
				fn $id() {
					assert_ne!(decimal($a).unwrap(), decimal($b).unwrap());
				}
			)*
		};
	}

	same_value_tests! {
		an_exponent_and_a_plain_integer_have_the_same_value: "1.5E2" == "150",
		trailing_fraction_zeros_do_not_change_the_value: "150.000" == "150",
		a_negative_exponent_can_cancel_trailing_integer_zeros: "15000e-2" == "150",
		an_explicit_plus_exponent_reads_as_the_bare_one: "1.5e+2" == "1.5E2",
		leading_exponent_zeros_do_not_change_the_value: "1e007" == "1e7",
		an_upper_case_exponent_reads_as_a_lower_case_one: "2E+3" == "2000",
		leading_fraction_zeros_shift_the_scale: "0.0015" == "1.5e-3",
		a_negative_number_keeps_its_sign_through_an_exponent: "-0.25E1" == "-2.5",
		negative_zero_is_zero: "-0" == "0",
		zero_with_a_fraction_and_an_exponent_is_zero: "-0.000e12" == "0",
		zero_with_an_exponent_past_i64_is_still_zero: "0e99999999999999999999" == "0"
	}

	different_value_tests! {
		opposite_signs_differ: "-1.5" != "1.5",
		a_shifted_digit_differs: "15" != "150",
		a_digit_past_f64_precision_still_differs: "1.00000000000000000001" != "1",
		an_extra_significant_digit_differs: "1.5" != "1.55"
	}

	#[test]
	fn an_exponent_past_i64_has_no_decimal_value() {
		assert_eq!(decimal("1e9223372036854775808"), None);
		assert_eq!(decimal("1e-9223372036854775809"), None);
	}

	#[test]
	fn the_widest_i64_exponents_still_have_a_decimal_value() {
		assert!(decimal("1e9223372036854775807").is_some());
		assert!(decimal("1e-9223372036854775808").is_some());
	}
}
