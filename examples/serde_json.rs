//! This example shows how to convert a `serde_json::Value` into/from a
//! `jstrict::Value` using the `serde_json` feature.

fn main() {
	// First we create a `serde_json` value.
	let a = serde_json::json!({
		"foo": 1,
		"bar": [2, 3]
	});

	// We convert the `serde_json` value into a `jstrict` value.
	let b = jstrict::Value::from_serde_json(a);

	// We convert it back into a `serde_json` value.
	let _ = jstrict::Value::into_serde_json(b);
}
