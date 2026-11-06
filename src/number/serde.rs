use crate::number::{Buffer, InvalidNumber, Number, NumberBuf};
use de::{Deserialize, Deserializer};
use ser::{Serialize, SerializeStruct, Serializer};
use serde::{de, forward_to_deserialize_any, ser};
use std::error::Error as StdError;
use std::{fmt, marker::PhantomData};

/// Structure name used to serialize number with arbitrary precision.
///
/// This is the same used by `serde_json`, to ensure compatibility with this
/// crate. Only `serde_json` with its `arbitrary_precision` feature, and the
/// serializer of this crate, read a struct of this name as a number. Every
/// other serializer sees a struct of one field, so [`Number`] uses it only for
/// a value that no primitive holds.
const TOKEN: &str = "$serde_json::private::Number";

impl Serialize for Number {
	/// Serializes the number through the narrowest primitive that holds its
	/// value.
	///
	/// A number written without a fraction or an exponent goes through
	/// `serialize_i64` or `serialize_u64`, and through `serialize_i128` or
	/// `serialize_u128` past their range. Any other number goes through
	/// `serialize_f64` when [`Number::as_f64_lossless`] holds its value, so
	/// `1.5E2` is serialized as the `f64` `150.0`, as `serde_json` itself would
	/// read it. So is `-0`, as the `f64` `-0.0`, since no integer type has a
	/// negative zero.
	///
	/// A number that no primitive holds, an integer past `u128` or a decimal
	/// past the precision of `f64`, is serialized as the struct `serde_json`
	/// uses for arbitrary precision. Only `serde_json` with its
	/// `arbitrary_precision` feature, and the serializer of this crate, read it
	/// back as a number.
	#[inline]
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let integer = !self.has_decimal_point() && !self.has_exponent() && self.as_str() != "-0";
		if integer && let Some(v) = self.as_i64() {
			serializer.serialize_i64(v)
		} else if integer && let Some(v) = self.as_u64() {
			serializer.serialize_u64(v)
		} else if integer && let Ok(v) = self.as_str().parse::<i128>() {
			serializer.serialize_i128(v)
		} else if integer && let Ok(v) = self.as_str().parse::<u128>() {
			serializer.serialize_u128(v)
		} else if !integer && let Some(v) = self.as_f64_lossless() {
			serializer.serialize_f64(v)
		} else {
			let mut s = serializer.serialize_struct(TOKEN, 1)?;
			s.serialize_field(TOKEN, self.as_str())?;
			s.end()
		}
	}
}

impl<B: Buffer> Serialize for NumberBuf<B> {
	#[inline]
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		self.as_number().serialize(serializer)
	}
}

impl<'de, B: Buffer> Deserialize<'de> for NumberBuf<B> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer.deserialize_any(Visitor(PhantomData))
	}
}

/// Number visitor.
pub struct Visitor<B>(PhantomData<B>);

impl<'de, B: Buffer> de::Visitor<'de> for Visitor<B> {
	type Value = NumberBuf<B>;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		formatter.write_str("JSON number")
	}

	#[inline]
	fn visit_u64<E: de::Error>(self, value: u64) -> Result<NumberBuf<B>, E> {
		Ok(value.into())
	}

	#[inline]
	fn visit_i64<E: de::Error>(self, value: i64) -> Result<NumberBuf<B>, E> {
		Ok(value.into())
	}

	#[inline]
	fn visit_f64<E: de::Error>(self, value: f64) -> Result<NumberBuf<B>, E> {
		NumberBuf::try_from(value)
			.map_err(|_| E::invalid_value(de::Unexpected::Float(value), &self))
	}

	fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
	where
		A: de::MapAccess<'de>,
	{
		struct Key;

		impl<'de> Deserialize<'de> for Key {
			fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
			where
				D: Deserializer<'de>,
			{
				struct KeyVisitor;

				impl de::Visitor<'_> for KeyVisitor {
					type Value = Key;

					fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
						formatter.write_str("a valid number field")
					}

					fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
					where
						E: de::Error,
					{
						if v == TOKEN {
							Ok(Key)
						} else {
							Err(de::Error::custom("expected field with custom name"))
						}
					}
				}

				deserializer.deserialize_identifier(KeyVisitor)
			}
		}

		struct Value<B>(NumberBuf<B>);

		impl<'de, B: Buffer> Deserialize<'de> for Value<B> {
			fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
			where
				D: Deserializer<'de>,
			{
				struct ValueVisitor<B>(PhantomData<B>);

				impl<B: Buffer> de::Visitor<'_> for ValueVisitor<B> {
					type Value = Value<B>;

					fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
						formatter.write_str("string containing a JSON number")
					}

					fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
					where
						E: de::Error,
					{
						self.visit_string(v.to_owned())
					}

					fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
					where
						E: de::Error,
					{
						match NumberBuf::new(B::from_vec(v.into_bytes())) {
							Ok(v) => Ok(Value(v)),
							Err(InvalidNumber(bytes)) => Err(de::Error::custom(InvalidNumber(
								String::from_utf8_lossy(bytes.as_ref()).into_owned(),
							))),
						}
					}
				}

				deserializer.deserialize_identifier(ValueVisitor(PhantomData))
			}
		}

		match map.next_key()? {
			Some(Key) => {
				let value: Value<B> = map.next_value()?;
				Ok(value.0)
			}
			None => Err(de::Error::invalid_type(de::Unexpected::Map, &self)),
		}
	}
}

/// Unexpected value that is not a number.
#[derive(Debug)]
pub struct Unexpected(String);

impl fmt::Display for Unexpected {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

impl StdError for Unexpected {}

impl de::Error for Unexpected {
	fn custom<T>(msg: T) -> Self
	where
		T: fmt::Display,
	{
		Self(msg.to_string())
	}

	fn invalid_type(unexp: de::Unexpected, exp: &dyn de::Expected) -> Self {
		if let de::Unexpected::Unit = unexp {
			Self::custom(format_args!("invalid type: null, expected {exp}"))
		} else {
			Self::custom(format_args!("invalid type: {unexp}, expected {exp}"))
		}
	}
}

impl<'de, B: Buffer> Deserializer<'de> for NumberBuf<B> {
	type Error = Unexpected;

	#[inline(always)]
	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.as_number().deserialize_any(visitor)
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf option unit unit_struct seq tuple
		tuple_struct map struct newtype_struct enum identifier ignored_any
	}
}

impl<'de, B: Buffer> Deserializer<'de> for &NumberBuf<B> {
	type Error = Unexpected;

	#[inline(always)]
	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.as_number().deserialize_any(visitor)
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf option unit unit_struct seq tuple
		tuple_struct map struct newtype_struct enum identifier ignored_any
	}
}

impl<'de> Deserializer<'de> for &Number {
	type Error = Unexpected;

	#[inline(always)]
	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		if let Some(u) = self.as_u64() {
			visitor.visit_u64(u)
		} else if let Some(i) = self.as_i64() {
			visitor.visit_i64(i)
		} else {
			visitor.visit_f64(self.as_f64_lossy())
		}
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf option unit unit_struct seq tuple
		tuple_struct map struct newtype_struct enum identifier ignored_any
	}
}

#[cfg(test)]
mod tests {
	use crate::{Number, NumberBuf, Parse, Value, number::decimal::Decimal};

	fn serialized_by_serde_json(text: &str) -> String {
		serde_json::to_string(Number::new(text).unwrap()).unwrap()
	}

	fn serialized_by_jstrict(text: &str) -> Value {
		crate::to_value(Number::new(text).unwrap()).unwrap()
	}

	fn number_value(text: &str) -> Value {
		Value::Number(text.parse().unwrap())
	}

	macro_rules! serde_json_tests {
		{ $($id:ident: $input:literal => $output:literal),* } => {
			$(
				#[test]
				fn $id() {
					assert_eq!(serialized_by_serde_json($input), $output);
				}
			)*
		};
	}

	serde_json_tests! {
		a_short_decimal_is_serialized_as_an_f64: "1.1" => "1.1",
		a_decimal_with_trailing_zeros_is_serialized_as_its_f64: "1.10" => "1.1",
		a_decimal_with_an_integer_value_stays_an_f64: "1.0" => "1.0",
		an_exponent_form_is_serialized_as_its_f64: "1.5E2" => "150.0",
		an_integer_exponent_is_serialized_as_an_f64: "1e3" => "1000.0",
		negative_zero_with_a_fraction_keeps_its_sign: "-0.0" => "-0.0",
		negative_zero_written_as_an_integer_keeps_its_sign: "-0" => "-0.0",
		zero_is_serialized_as_an_integer: "0" => "0",
		a_plain_integer_is_serialized_as_an_integer: "42" => "42",
		a_negative_integer_is_serialized_as_an_integer: "-42" => "-42",
		the_smallest_i64_is_serialized_as_an_i64: "-9223372036854775808" => "-9223372036854775808",
		the_largest_u64_is_serialized_as_a_u64: "18446744073709551615" => "18446744073709551615",
		an_integer_past_u64_is_serialized_as_a_u128: "18446744073709551616" => "18446744073709551616",
		an_integer_below_i64_is_serialized_as_an_i128: "-9223372036854775809" => "-9223372036854775809",
		an_integer_past_u128_falls_back_to_the_arbitrary_precision_struct: "340282366920938463463374607431768211456" => r#"{"$serde_json::private::Number":"340282366920938463463374607431768211456"}"#,
		an_integer_below_i128_falls_back_to_the_arbitrary_precision_struct: "-170141183460469231731687303715884105729" => r#"{"$serde_json::private::Number":"-170141183460469231731687303715884105729"}"#,
		the_largest_f64_is_serialized_as_an_f64: "1.7976931348623157e308" => "1.7976931348623157e+308",
		the_smallest_subnormal_f64_is_serialized_as_an_f64: "5e-324" => "5e-324",
		a_decimal_past_f64_precision_falls_back_to_the_arbitrary_precision_struct: "1.00000000000000000001" => r#"{"$serde_json::private::Number":"1.00000000000000000001"}"#,
		a_subnormal_written_past_f64_precision_falls_back_to_the_arbitrary_precision_struct: "4.9406564584124654e-324" => r#"{"$serde_json::private::Number":"4.9406564584124654e-324"}"#,
		a_number_past_the_f64_range_falls_back_to_the_arbitrary_precision_struct: "1e400" => r#"{"$serde_json::private::Number":"1e400"}"#
	}

	#[test]
	fn a_decimal_reaches_a_serde_json_value_as_a_number() {
		let value = serde_json::to_value(Number::new("1.1").unwrap()).unwrap();
		assert_eq!(value, serde_json::json!(1.1));
	}

	#[test]
	fn a_plain_serde_json_value_refuses_an_integer_past_u64() {
		assert!(serde_json::to_value(Number::new("18446744073709551616").unwrap()).is_err());
	}

	#[test]
	fn every_number_in_a_document_reaches_a_serde_json_value_as_a_number() {
		let (document, _) = Value::parse_str(
			r#"{"@version": 1.1, "@protected": true, "numbers": [1, -2, 1.5E2, 1e3, 0.25]}"#,
		)
		.unwrap();
		assert_eq!(
			serde_json::to_value(&document).unwrap(),
			serde_json::json!({"@version": 1.1, "@protected": true, "numbers": [1, -2, 150.0, 1000.0, 0.25]})
		);
	}

	#[test]
	fn a_number_written_by_serde_json_reads_back_with_the_same_value() {
		for text in [
			"1.1",
			"1.5E2",
			"1e3",
			"-0",
			"-0.0",
			"42",
			"-9223372036854775808",
			"18446744073709551615",
			"5e-324",
		] {
			let written = serialized_by_serde_json(text);
			let read: NumberBuf = serde_json::from_str(&written).unwrap();
			assert_eq!(
				Decimal::of(&read),
				Decimal::of(Number::new(text).unwrap()),
				"{text} was written as {written}"
			);
		}
	}

	#[test]
	fn the_serializer_of_this_crate_keeps_the_sign_of_negative_zero() {
		assert_eq!(serialized_by_jstrict("-0"), number_value("-0"));
	}

	#[test]
	fn the_serializer_of_this_crate_reads_an_f64_back_in_its_shortest_form() {
		assert_eq!(serialized_by_jstrict("1.5E2"), number_value("150"));
		assert_eq!(serialized_by_jstrict("1.1"), number_value("1.1"));
	}

	#[test]
	fn the_serializer_of_this_crate_keeps_a_128_bit_integer() {
		assert_eq!(
			serialized_by_jstrict("340282366920938463463374607431768211455"),
			number_value("340282366920938463463374607431768211455")
		);
		assert_eq!(
			serialized_by_jstrict("-170141183460469231731687303715884105728"),
			number_value("-170141183460469231731687303715884105728")
		);
	}

	#[test]
	fn the_serializer_of_this_crate_keeps_a_number_no_primitive_holds() {
		assert_eq!(
			serialized_by_jstrict("340282366920938463463374607431768211456"),
			number_value("340282366920938463463374607431768211456")
		);
		assert_eq!(
			serialized_by_jstrict("1.00000000000000000001"),
			number_value("1.00000000000000000001")
		);
	}
}
