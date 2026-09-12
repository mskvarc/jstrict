use core::hash::{Hash, Hasher};
use core::ptr;

/// Wrapper to view a value without considering the order of
/// the objects entries.
///
/// `Unordered<T>` implements `PartialEq`/`Eq` in terms of
/// [`UnorderedPartialEq`], so `{ "a": 0, "b": 1 }` and `{ "b": 1, "a": 0 }`
/// compare equal — unlike the derived `PartialEq` on
/// [`Value`](crate::Value), which is order-sensitive.
///
/// # Example
///
/// ```
/// use jstrict::{json, BorrowUnordered, Unordered};
///
/// let a = json!({ "a": 0, "b": 1 });
/// let b = json!({ "b": 1, "a": 0 });
///
/// assert_ne!(a, b);
/// assert_eq!(a.as_unordered(), b.as_unordered());
/// assert_eq!(Unordered(a), Unordered(b));
/// ```
#[derive(Debug)]
#[repr(transparent)]
pub struct Unordered<T: ?Sized>(pub T);

/// Free conversion from `&T` to `&Unordered<T>`.
pub trait BorrowUnordered {
	/// Views this value as an [`Unordered`] reference.
	fn as_unordered(&self) -> &Unordered<Self>;
}

impl<T> BorrowUnordered for T {
	fn as_unordered(&self) -> &Unordered<Self> {
		unsafe { &*ptr::from_ref(self).cast::<Unordered<Self>>() }
	}
}

/// Equality that ignores the order of object entries.
pub trait UnorderedPartialEq {
	/// Checks if `self` and `other` are equal modulo object entry order.
	fn unordered_eq(&self, other: &Self) -> bool;
}

impl<T: UnorderedPartialEq> UnorderedPartialEq for Vec<T> {
	fn unordered_eq(&self, other: &Self) -> bool {
		self.len() == other.len() && self.iter().zip(other).all(|(a, b)| a.unordered_eq(b))
	}
}

impl<T: UnorderedPartialEq> PartialEq for Unordered<T> {
	fn eq(&self, other: &Self) -> bool {
		self.0.unordered_eq(&other.0)
	}
}

/// Marker trait stating that [`UnorderedPartialEq`] is an equivalence
/// relation, mirroring `Eq` over `PartialEq`.
pub trait UnorderedEq: UnorderedPartialEq {}

impl<T: UnorderedEq> Eq for Unordered<T> {}

/// Hashing that ignores the order of object entries.
///
/// Implementing this is what gives `Unordered<T>` its `Hash` impl, keeping it
/// consistent with the `PartialEq` derived from [`UnorderedPartialEq`].
pub trait UnorderedHash {
	/// Feeds this value into `state`, ignoring object entry order.
	fn unordered_hash<H: Hasher>(&self, state: &mut H);
}

impl<T: UnorderedHash> Hash for Unordered<T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.0.unordered_hash(state);
	}
}
