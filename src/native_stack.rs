//! Keeping recursion off the caller's native stack.
//!
//! Printing and serializing a [`Value`](crate::Value) recurse once per nesting
//! level. A caller may run them on a thread whose stack it has already partly
//! used, such as an async worker deep inside a request handler, so the
//! nesting depth alone cannot decide whether the thread's stack suffices.
//!
//! Each recursion point runs its callee through [`on_sufficient_stack`]: when
//! less than [`RED_ZONE`] of the current stack is left, the callee runs on a
//! freshly allocated segment of [`SEGMENT`] bytes instead. Crates built on
//! `jstrict` use the same guard, so one red zone governs every recursion of
//! a document.

/// Stack left below which a recursion level moves to a fresh segment.
///
/// It has to exceed the native stack one level uses between two recursion
/// points, which is largest in unoptimized builds.
pub const RED_ZONE: usize = 128 * 1024;

/// Size of each fresh stack segment.
pub const SEGMENT: usize = 2 * 1024 * 1024;

/// Runs `callee`, on a fresh stack segment when the current one is nearly used.
pub fn on_sufficient_stack<R>(callee: impl FnOnce() -> R) -> R {
	stacker::maybe_grow(RED_ZONE, SEGMENT, callee)
}
