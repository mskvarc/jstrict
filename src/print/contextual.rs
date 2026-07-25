//! Printing adapters for the [`contextual`] crate.
//!
//! These mirror [`Print`](super::Print), [`PrintWithSize`](super::PrintWithSize)
//! and [`PrecomputeSize`](super::PrecomputeSize), threading a `&C` context
//! through so that types whose rendering depends on external state (an IRI
//! namespace, a vocabulary, …) can be printed as JSON. Wrapping such a value
//! in `contextual::Contextual` gives back the plain print traits.
use contextual::{Contextual, WithContext};
use std::collections::HashSet;
use std::fmt;

use super::{Options, Size};

/// [`Print`](super::Print) with an additional context.
pub trait PrintWithContext<C> {
	/// Writes the value to `f`, resolving it against `context`.
	fn contextual_fmt_with(
		&self,
		context: &C,
		f: &mut fmt::Formatter,
		options: &Options,
		indent: usize,
	) -> fmt::Result;
}

impl<T: PrintWithContext<C> + ?Sized, C> PrintWithContext<C> for &T {
	fn contextual_fmt_with(
		&self,
		context: &C,
		f: &mut fmt::Formatter,
		options: &Options,
		indent: usize,
	) -> fmt::Result {
		T::contextual_fmt_with(*self, context, f, options, indent)
	}
}

impl<T: PrintWithContext<C>, C> super::Print for Contextual<T, &C> {
	fn fmt_with(&self, f: &mut fmt::Formatter, options: &Options, indent: usize) -> fmt::Result {
		self.0.contextual_fmt_with(self.1, f, options, indent)
	}
}

/// [`PrintWithSize`](super::PrintWithSize) with an additional context.
pub trait PrintWithSizeAndContext<C> {
	/// Writes the value to `f` against `context`, taking its layout from
	/// `sizes[*index]`.
	fn contextual_fmt_with_size(
		&self,
		context: &C,
		f: &mut std::fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> std::fmt::Result;
}

impl<T: PrintWithSizeAndContext<C> + ?Sized, C> PrintWithSizeAndContext<C> for &T {
	fn contextual_fmt_with_size(
		&self,
		context: &C,
		f: &mut std::fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> std::fmt::Result {
		T::contextual_fmt_with_size(*self, context, f, options, indent, sizes, index)
	}
}

impl<T: PrintWithSizeAndContext<C>, C> super::PrintWithSize for Contextual<T, &C> {
	fn fmt_with_size(
		&self,
		f: &mut std::fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> std::fmt::Result {
		self.0
			.contextual_fmt_with_size(self.1, f, options, indent, sizes, index)
	}
}

/// [`PrecomputeSize`](super::PrecomputeSize) with an additional context.
pub trait PrecomputeSizeWithContext<C> {
	/// Computes the printed size of this value against `context`.
	fn contextual_pre_compute_size(
		&self,
		context: &C,
		options: &Options,
		sizes: &mut Vec<Size>,
	) -> Size;
}

impl<T: PrecomputeSizeWithContext<C> + ?Sized, C> PrecomputeSizeWithContext<C> for &T {
	fn contextual_pre_compute_size(
		&self,
		context: &C,
		options: &Options,
		sizes: &mut Vec<Size>,
	) -> Size {
		T::contextual_pre_compute_size(*self, context, options, sizes)
	}
}

impl<T: PrecomputeSizeWithContext<C>, C> super::PrecomputeSize for Contextual<T, &C> {
	fn pre_compute_size(&self, options: &Options, sizes: &mut Vec<Size>) -> Size {
		self.0.contextual_pre_compute_size(self.1, options, sizes)
	}
}

impl<T: PrecomputeSizeWithContext<C>, M, C> PrecomputeSizeWithContext<C> for locspan::Meta<T, M> {
	fn contextual_pre_compute_size(
		&self,
		context: &C,
		options: &Options,
		sizes: &mut Vec<Size>,
	) -> Size {
		self.0.contextual_pre_compute_size(context, options, sizes)
	}
}

impl<T: PrintWithSizeAndContext<C>, M, C> PrintWithSizeAndContext<C> for locspan::Meta<T, M> {
	fn contextual_fmt_with_size(
		&self,
		context: &C,
		f: &mut std::fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> std::fmt::Result {
		self.0
			.contextual_fmt_with_size(context, f, options, indent, sizes, index)
	}
}

impl<T: PrintWithContext<C>, M, C> PrintWithContext<C> for locspan::Meta<T, M> {
	fn contextual_fmt_with(
		&self,
		context: &C,
		f: &mut std::fmt::Formatter,
		options: &Options,
		indent: usize,
	) -> std::fmt::Result {
		self.0.contextual_fmt_with(context, f, options, indent)
	}
}

impl<T: PrecomputeSizeWithContext<C>, C> PrecomputeSizeWithContext<C> for locspan::Stripped<T> {
	fn contextual_pre_compute_size(
		&self,
		context: &C,
		options: &Options,
		sizes: &mut Vec<Size>,
	) -> Size {
		self.0.contextual_pre_compute_size(context, options, sizes)
	}
}

impl<T: PrintWithSizeAndContext<C>, C> PrintWithSizeAndContext<C> for locspan::Stripped<T> {
	fn contextual_fmt_with_size(
		&self,
		context: &C,
		f: &mut std::fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> std::fmt::Result {
		self.0
			.contextual_fmt_with_size(context, f, options, indent, sizes, index)
	}
}

impl<T: PrintWithContext<C>, C> PrintWithContext<C> for locspan::Stripped<T> {
	fn contextual_fmt_with(
		&self,
		context: &C,
		f: &mut std::fmt::Formatter,
		options: &Options,
		indent: usize,
	) -> std::fmt::Result {
		self.0.contextual_fmt_with(context, f, options, indent)
	}
}

impl<T: PrecomputeSizeWithContext<C>, C> PrecomputeSizeWithContext<C> for [T] {
	fn contextual_pre_compute_size(
		&self,
		context: &C,
		options: &Options,
		sizes: &mut Vec<Size>,
	) -> Size {
		super::pre_compute_array_size(self.iter().map(|i| i.with(context)), options, sizes)
	}
}

impl<T: PrintWithSizeAndContext<C>, C> PrintWithSizeAndContext<C> for [T] {
	fn contextual_fmt_with_size(
		&self,
		context: &C,
		f: &mut std::fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> std::fmt::Result {
		super::print_array(
			self.iter().map(|i| i.with(context)),
			f,
			options,
			indent,
			sizes,
			index,
		)
	}
}

impl<T: PrecomputeSizeWithContext<C>, C> PrecomputeSizeWithContext<C> for HashSet<T> {
	fn contextual_pre_compute_size(
		&self,
		context: &C,
		options: &Options,
		sizes: &mut Vec<Size>,
	) -> Size {
		super::pre_compute_array_size(self.iter().map(|i| i.with(context)), options, sizes)
	}
}

impl<T: PrintWithSizeAndContext<C>, C> PrintWithSizeAndContext<C> for HashSet<T> {
	fn contextual_fmt_with_size(
		&self,
		context: &C,
		f: &mut std::fmt::Formatter,
		options: &Options,
		indent: usize,
		sizes: &[Size],
		index: &mut usize,
	) -> std::fmt::Result {
		super::print_array(
			self.iter().map(|i| i.with(context)),
			f,
			options,
			indent,
			sizes,
			index,
		)
	}
}
