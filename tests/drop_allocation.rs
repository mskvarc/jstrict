//! Dropping a shallow value allocates no worklist.
//!
//! Its own test binary, because it replaces the global allocator to count
//! allocations.
use jstrict::{Value, json};
use std::{
	alloc::{GlobalAlloc, Layout, System},
	sync::atomic::{AtomicUsize, Ordering},
};

/// The system allocator, counting the allocations made while counting is on.
struct CountingAllocator;

/// Allocations made since counting was last reset.
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
		unsafe { System.alloc(layout) }
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		unsafe { System.dealloc(ptr, layout) }
	}
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn dropping_a_value_whose_containers_hold_only_scalars_allocates_no_worklist() {
	let value: Value = json!({"a": 1, "b": "text", "c": [true, null, 2.5], "d": [], "e": {}});

	ALLOCATIONS.store(0, Ordering::SeqCst);
	drop(value);

	// The object holds one array of scalars and two empty containers: the
	// array goes on the worklist, the empty containers and scalars drop in
	// place, so the only allocation is the object's own worklist.
	assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 1);

	let flat: Value = json!([1, "two", null, true]);
	ALLOCATIONS.store(0, Ordering::SeqCst);
	drop(flat);
	assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 0);
}
