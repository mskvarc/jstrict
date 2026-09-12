use crate::number::{Buffer, Number, NumberBuf};
use sonic_rs::JsonNumberTrait;

impl<B: Buffer> From<sonic_rs::Number> for NumberBuf<B> {
	#[inline]
	fn from(n: sonic_rs::Number) -> Self {
		if let Some(u) = n.as_u64() {
			NumberBuf::from(u)
		} else if let Some(i) = n.as_i64() {
			NumberBuf::from(i)
		} else {
			let f = n.as_f64().unwrap_or(0.0);
			NumberBuf::try_from(f).unwrap_or_else(|_| NumberBuf::from(0u64))
		}
	}
}

impl<B: Buffer> From<NumberBuf<B>> for sonic_rs::Number {
	#[inline(always)]
	fn from(n: NumberBuf<B>) -> Self {
		Self::from(n.as_number())
	}
}

impl<'n> From<&'n Number> for sonic_rs::Number {
	fn from(n: &'n Number) -> Self {
		if let Some(u) = n.as_u64() {
			sonic_rs::Number::from(u)
		} else if let Some(i) = n.as_i64() {
			sonic_rs::Number::from(i)
		} else {
			sonic_rs::Number::from_f64(n.as_f64_lossy())
				.unwrap_or_else(|| sonic_rs::Number::from(0_i64))
		}
	}
}
