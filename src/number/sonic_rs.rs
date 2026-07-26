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
			let f = n.as_f64().expect("invalid `sonic_rs::Number`");
			NumberBuf::try_from(f).expect("invalid `sonic_rs::Number`")
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
			sonic_rs::Number::from_f64(n.as_f64_lossy()).expect("non-finite JSON number")
		}
	}
}
