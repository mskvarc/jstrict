//! Dropping nested values without recursion.
//!
//! The derived drop of a JSON tree recurses once per nesting level, so a
//! document the non-recursive parser accepts could overflow the stack when it
//! goes out of scope. [`Array`](crate::Array) and [`Object`](crate::Object)
//! drop through [`dismantle`] instead: every nested container is moved onto a
//! heap worklist and emptied there, so the drop of each container only ever
//! sees empty children.
use crate::{Value, object::Entry};

/// Drops every value of `pending` and all of their descendants without
/// recursing.
///
/// Each popped value has its nested containers moved onto `pending` before it
/// drops, so it drops with no nested container left in it.
pub(crate) fn dismantle(pending: &mut Vec<Value>) {
	while let Some(mut value) = pending.pop() {
		release_children(&mut value, pending);
	}
}

/// Moves the nested containers directly under `value` onto `pending`.
fn release_children(value: &mut Value, pending: &mut Vec<Value>) {
	match value {
		Value::Array(items) => pending.extend(items.drain(..).filter(Value::is_nested_container)),
		Value::Object(object) => pending.extend(
			object
				.drain_entries()
				.map(Entry::into_value)
				.filter(Value::is_nested_container),
		),
		Value::Null | Value::Boolean(_) | Value::Number(_) | Value::String(_) => {}
	}
}

impl Value {
	/// Whether this value is an array or an object with at least one item:
	/// the only values whose drop could recurse.
	pub(crate) const fn is_nested_container(&self) -> bool {
		match self {
			Self::Array(items) => !items.is_empty(),
			Self::Object(object) => !object.is_empty(),
			Self::Null | Self::Boolean(_) | Self::Number(_) | Self::String(_) => false,
		}
	}
}
