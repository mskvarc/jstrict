//! JSON value kinds.
use core::fmt;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign};

/// Value kind.
///
/// The six kinds of JSON value. `Kind` values combine with `|` and `&` into a
/// [`KindSet`], which is how expected-kind errors are reported.
///
/// ```
/// # use jstrict::{Kind, KindSet};
/// let expected: KindSet = Kind::Number | Kind::String;
/// assert_eq!(expected.as_disjunction().to_string(), "number or string");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Kind {
	/// The `null` value.
	Null,

	/// A boolean, `true` or `false`.
	Boolean,

	/// A number.
	Number,

	/// A string.
	String,

	/// An array.
	Array,

	/// An object.
	Object,
}

impl BitOr for Kind {
	type Output = KindSet;

	fn bitor(self, other: Self) -> KindSet {
		KindSet::from(self) | KindSet::from(other)
	}
}

impl BitOr<KindSet> for Kind {
	type Output = KindSet;

	fn bitor(self, other: KindSet) -> KindSet {
		KindSet::from(self) | other
	}
}

impl BitAnd for Kind {
	type Output = KindSet;

	fn bitand(self, other: Self) -> KindSet {
		KindSet::from(self) & KindSet::from(other)
	}
}

impl BitAnd<KindSet> for Kind {
	type Output = KindSet;

	fn bitand(self, other: KindSet) -> KindSet {
		KindSet::from(self) & other
	}
}

impl fmt::Display for Kind {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Null => write!(f, "null"),
			Self::Boolean => write!(f, "boolean"),
			Self::Number => write!(f, "number"),
			Self::String => write!(f, "string"),
			Self::Array => write!(f, "array"),
			Self::Object => write!(f, "object"),
		}
	}
}

macro_rules! kind_set {
	($($id:ident ($const:ident): $mask:literal),*) => {
		/// Set of JSON value [`Kind`].
		#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
		pub struct KindSet(u8);

		impl KindSet {
			$(
				#[doc = concat!("Set containing only [`Kind::", stringify!($id), "`].")]
				pub const $const: Self = Self($mask);
			)*

			/// Set containing every [`Kind`].
			pub const fn all() -> Self {
				Self($($mask)|*)
			}
		}

		impl std::ops::BitOr<Kind> for KindSet {
			type Output = Self;

			fn bitor(self, other: Kind) -> Self {
				match other {
					$(
						Kind::$id => Self(self.0 | $mask)
					),*
				}
			}
		}

		impl std::ops::BitOrAssign<Kind> for KindSet {
			fn bitor_assign(&mut self, other: Kind) {
				match other {
					$(
						Kind::$id => self.0 |= $mask
					),*
				}
			}
		}

		impl std::ops::BitAnd<Kind> for KindSet {
			type Output = Self;

			fn bitand(self, other: Kind) -> Self {
				match other {
					$(
						Kind::$id => Self(self.0 & $mask)
					),*
				}
			}
		}

		impl std::ops::BitAndAssign<Kind> for KindSet {
			fn bitand_assign(&mut self, other: Kind) {
				match other {
					$(
						Kind::$id => self.0 &= $mask
					),*
				}
			}
		}

		impl From<Kind> for KindSet {
			fn from(value: Kind) -> Self {
				match value {
					$(
						Kind::$id => Self($mask)
					),*
				}
			}
		}

		/// Iterator over the [`Kind`]s of a [`KindSet`], in declaration order.
		#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
		pub struct KindSetIter(u8);

		impl Iterator for KindSetIter {
			type Item = Kind;

			fn size_hint(&self) -> (usize, Option<usize>) {
				let len = self.0.count_ones() as usize;
				(len, Some(len))
			}

			fn next(&mut self) -> Option<Kind> {
				$(
					if self.0 & $mask != 0 {
						self.0 &= !$mask;
						return Some(Kind::$id)
					}
				)*

				None
			}
		}

		impl DoubleEndedIterator for KindSetIter {
			fn next_back(&mut self) -> Option<Kind> {
				let mut result = None;

				$(
					if self.0 & $mask != 0 {
						result = Some((Kind::$id, $mask));
					}
				)*

				result.map(|(kind, mask)| {
					self.0 &= !mask;
					kind
				})
			}
		}

		impl core::iter::FusedIterator for KindSetIter {}
		impl core::iter::ExactSizeIterator for KindSetIter {}
	};
}

kind_set! {
	Null (NULL):       0b00_0001,
	Boolean (BOOLEAN): 0b00_0010,
	Number (NUMBER):   0b00_0100,
	String (STRING):   0b00_1000,
	Array (ARRAY):     0b01_0000,
	Object (OBJECT):   0b10_0000
}

impl KindSet {
	/// The empty set.
	#[must_use]
	pub const fn none() -> Self {
		Self(0)
	}

	/// Returns the number of kinds in this set.
	#[must_use]
	pub const fn len(&self) -> usize {
		self.0.count_ones() as usize
	}

	/// Checks if this set is empty.
	#[must_use]
	pub const fn is_empty(&self) -> bool {
		self.0 == 0
	}

	/// Iterates over the kinds in this set, in declaration order.
	#[must_use]
	pub const fn iter(&self) -> KindSetIter {
		KindSetIter(self.0)
	}

	/// Displays this set as a disjunction.
	///
	/// # Example
	///
	/// ```
	/// # use jstrict::{Kind, KindSet};
	/// let set = Kind::Null | Kind::String | Kind::Object;
	/// assert_eq!(set.as_disjunction().to_string(), "null, string or object");
	/// assert_eq!(KindSet::ARRAY.as_disjunction().to_string(), "array");
	/// assert_eq!(KindSet::all().as_disjunction().to_string(), "anything");
	/// assert_eq!(KindSet::none().as_disjunction().to_string(), "nothing");
	/// ```
	#[must_use]
	pub const fn as_disjunction(self) -> KindSetDisjunction {
		KindSetDisjunction(self)
	}

	/// Displays this set as a conjunction.
	///
	/// # Example
	///
	/// ```
	/// # use jstrict::{Kind, KindSet};
	/// let set = Kind::Null | Kind::String | Kind::Object;
	/// assert_eq!(set.as_conjunction().to_string(), "null, string and object");
	/// assert_eq!(KindSet::ARRAY.as_conjunction().to_string(), "array");
	/// assert_eq!(KindSet::all().as_conjunction().to_string(), "anything");
	/// assert_eq!(KindSet::none().as_conjunction().to_string(), "nothing");
	/// ```
	#[must_use]
	pub const fn as_conjunction(self) -> KindSetConjunction {
		KindSetConjunction(self)
	}
}

impl BitOr for KindSet {
	type Output = Self;

	fn bitor(self, other: Self) -> Self {
		Self(self.0 | other.0)
	}
}

impl BitOrAssign for KindSet {
	fn bitor_assign(&mut self, other: Self) {
		self.0 |= other.0;
	}
}

impl BitAnd for KindSet {
	type Output = Self;

	fn bitand(self, other: Self) -> Self {
		Self(self.0 & other.0)
	}
}

impl BitAndAssign for KindSet {
	fn bitand_assign(&mut self, other: Self) {
		self.0 &= other.0;
	}
}

impl IntoIterator for &KindSet {
	type IntoIter = KindSetIter;
	type Item = Kind;

	fn into_iter(self) -> KindSetIter {
		self.iter()
	}
}

impl IntoIterator for KindSet {
	type IntoIter = KindSetIter;
	type Item = Kind;

	fn into_iter(self) -> KindSetIter {
		self.iter()
	}
}

impl fmt::Display for KindSet {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for (i, kind) in self.into_iter().enumerate() {
			if i > 0 {
				f.write_str(", ")?;
			}

			kind.fmt(f)?;
		}

		Ok(())
	}
}

/// Displays a `KindSet` as a disjunction.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KindSetDisjunction(pub KindSet);

impl fmt::Display for KindSetDisjunction {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.0 == KindSet::all() {
			f.write_str("anything")
		} else {
			let mut iter = self.0.into_iter();
			match iter.next_back() {
				Some(last) => {
					if let Some(first) = iter.next() {
						first.fmt(f)?;
						for k in iter {
							f.write_str(", ")?;
							k.fmt(f)?;
						}
						f.write_str(" or ")?;
					}

					last.fmt(f)
				}
				None => f.write_str("nothing"),
			}
		}
	}
}

/// Displays a `KindSet` as a conjunction.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KindSetConjunction(pub KindSet);

impl fmt::Display for KindSetConjunction {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.0 == KindSet::all() {
			f.write_str("anything")
		} else {
			let mut iter = self.0.into_iter();
			match iter.next_back() {
				Some(last) => {
					if let Some(first) = iter.next() {
						first.fmt(f)?;
						for k in iter {
							f.write_str(", ")?;
							k.fmt(f)?;
						}
						f.write_str(" and ")?;
					}

					last.fmt(f)
				}
				None => f.write_str("nothing"),
			}
		}
	}
}
