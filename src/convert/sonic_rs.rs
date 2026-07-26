use crate::{Object, Value, object::Entry};
use sonic_rs::value::ValueRef;

impl Value {
	/// Converts a [`sonic_rs::Value`] into a `Value`.
	///
	/// # Example
	///
	/// ```
	/// // First we create a `sonic_rs` value.
	/// let a = sonic_rs::json!({
	///   "foo": 1,
	///   "bar": [2, 3]
	/// });
	///
	/// // We convert the `sonic_rs` value into a `jstrict` value.
	/// let b = jstrict::Value::from_sonic_rs(a);
	///
	/// // We convert it back into a `sonic_rs` value.
	/// let _ = jstrict::Value::into_sonic_rs(b);
	/// ```
	pub fn from_sonic_rs(value: sonic_rs::Value) -> Self {
		match value.as_ref() {
			ValueRef::Null => Self::Null,
			ValueRef::Bool(b) => Self::Boolean(b),
			ValueRef::Number(n) => Self::Number(n.into()),
			ValueRef::String(s) => Self::String(s.into()),
			ValueRef::Array(a) => {
				let mut out = Vec::with_capacity(a.len());
				for v in a.iter() {
					out.push(Self::from_sonic_rs(v.clone()));
				}
				Self::Array(out)
			}
			ValueRef::Object(o) => {
				let mut out = Object::with_capacity(o.len());
				for (k, v) in o.iter() {
					out.push_entry(Entry::new(k.into(), Self::from_sonic_rs(v.clone())));
				}
				Self::Object(out)
			}
		}
	}

	/// Converts a `Value` into a [`sonic_rs::Value`].
	///
	/// # Example
	///
	/// ```
	/// // First we create a `sonic_rs` value.
	/// let a = sonic_rs::json!({
	///   "foo": 1,
	///   "bar": [2, 3]
	/// });
	///
	/// // We convert the `sonic_rs` value into a `jstrict` value.
	/// let b = jstrict::Value::from_sonic_rs(a);
	///
	/// // We convert it back into a `sonic_rs` value.
	/// let _ = jstrict::Value::into_sonic_rs(b);
	/// ```
	pub fn into_sonic_rs(self) -> sonic_rs::Value {
		match self {
			Self::Null => sonic_rs::Value::default(),
			Self::Boolean(b) => sonic_rs::Value::from(b),
			Self::Number(n) => sonic_rs::Value::from(sonic_rs::Number::from(n)),
			Self::String(s) => sonic_rs::Value::from(s.as_str()),
			Self::Array(a) => {
				let mut out = sonic_rs::Array::with_capacity(a.len());
				for v in a {
					out.push(Value::into_sonic_rs(v));
				}
				sonic_rs::Value::from(out)
			}
			Self::Object(o) => {
				let mut out = sonic_rs::Object::with_capacity(o.len());
				for Entry { key, value } in o {
					out.insert(&key.as_str(), Value::into_sonic_rs(value));
				}
				sonic_rs::Value::from(out)
			}
		}
	}
}

impl From<sonic_rs::Value> for Value {
	#[inline(always)]
	fn from(value: sonic_rs::Value) -> Self {
		Self::from_sonic_rs(value)
	}
}

impl From<Value> for sonic_rs::Value {
	fn from(value: Value) -> Self {
		value.into_sonic_rs()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sonic_rs::{JsonContainerTrait, JsonValueTrait};

	#[test]
	fn round_trip() {
		let src: sonic_rs::Value = sonic_rs::from_str(
			r#"{"n":null,"b":true,"i":-7,"u":42,"f":1.5,"s":"hi","a":[1,2,3],"o":{"k":"v"}}"#,
		)
		.unwrap();

		let mid = Value::from_sonic_rs(src);
		let back = mid.into_sonic_rs();

		assert!(back.get("n").unwrap().is_null());
		assert_eq!(back.get("b").unwrap().as_bool(), Some(true));
		assert_eq!(back.get("i").unwrap().as_i64(), Some(-7));
		assert_eq!(back.get("u").unwrap().as_u64(), Some(42));
		assert_eq!(back.get("f").unwrap().as_f64(), Some(1.5));
		assert_eq!(back.get("s").unwrap().as_str(), Some("hi"));
		assert_eq!(back.get("a").unwrap().as_array().unwrap().len(), 3);
		assert_eq!(back.get("o").unwrap().get("k").unwrap().as_str(), Some("v"));
	}
}
