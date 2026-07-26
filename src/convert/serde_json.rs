use crate::{Object, Value, object::Entry};

impl Value {
	/// Converts a [`serde_json::Value`] into a `Value`.
	///
	/// # Example
	///
	/// ```
	/// // First we create a `serde_json` value.
	/// let a = serde_json::json!({
	///   "foo": 1,
	///   "bar": [2, 3]
	/// });
	///
	/// // We convert the `serde_json` value into a `jstrict` value.
	/// let b = jstrict::Value::from_serde_json(a);
	///
	/// // We convert it back into a `serde_json` value.
	/// let _ = jstrict::Value::into_serde_json(b);
	/// ```
	pub fn from_serde_json(value: serde_json::Value) -> Self {
		match value {
			serde_json::Value::Null => Self::Null,
			serde_json::Value::Bool(b) => Self::Boolean(b),
			serde_json::Value::Number(n) => Self::Number(n.into()),
			serde_json::Value::String(s) => Self::String(s.into()),
			serde_json::Value::Array(a) => {
				let mut out = Vec::with_capacity(a.len());
				for v in a {
					out.push(Self::from_serde_json(v));
				}
				Self::Array(out)
			}
			serde_json::Value::Object(o) => {
				let mut out = Object::with_capacity(o.len());
				for (k, v) in o {
					out.push_entry(Entry::new(k.into(), Self::from_serde_json(v)));
				}
				Self::Object(out)
			}
		}
	}

	/// Converts a `Value` into a [`serde_json::Value`].
	///
	/// # Example
	///
	/// ```
	/// // First we create a `serde_json` value.
	/// let a = serde_json::json!({
	///   "foo": 1,
	///   "bar": [2, 3]
	/// });
	///
	/// // We convert the `serde_json` value into a `jstrict` value.
	/// let b = jstrict::Value::from_serde_json(a);
	///
	/// // We convert it back into a `serde_json` value.
	/// let _ = jstrict::Value::into_serde_json(b);
	/// ```
	pub fn into_serde_json(self) -> serde_json::Value {
		match self {
			Self::Null => serde_json::Value::Null,
			Self::Boolean(b) => serde_json::Value::Bool(b),
			Self::Number(n) => serde_json::Value::Number(n.into()),
			Self::String(s) => serde_json::Value::String(s.into_string()),
			Self::Array(a) => {
				let mut out = Vec::with_capacity(a.len());
				for v in a {
					out.push(Value::into_serde_json(v));
				}
				serde_json::Value::Array(out)
			}
			Self::Object(o) => {
				let mut out = serde_json::Map::with_capacity(o.len());
				for Entry { key, value } in o {
					out.insert(key.into_string(), Value::into_serde_json(value));
				}
				serde_json::Value::Object(out)
			}
		}
	}
}

impl From<serde_json::Value> for Value {
	#[inline(always)]
	fn from(value: serde_json::Value) -> Self {
		Self::from_serde_json(value)
	}
}

impl From<Value> for serde_json::Value {
	fn from(value: Value) -> Self {
		value.into_serde_json()
	}
}
