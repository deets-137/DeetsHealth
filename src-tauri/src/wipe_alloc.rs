// The zeroing allocator (GLOBALS.md §10.2; DeetsEngine request doc §8, "Not wiped"):
// every block of memory the app frees is overwritten with zeros first.
//
// `Shielded<T>` and the engine's memory store wipe what they hold. But record data also passes
// through copies that no wrapper owns: serde_json's parse of a raw resource, the engine's
// encoded keys and unpacked fields, the text of an error, a `String` on its way to a card.
// This covers all of them, in our code and in every crate, without a change to any of them.
//
// A grown or shrunk block is moved by hand (allocate, copy, wipe, free), because the system's
// own `realloc` may free the old block without wiping it.
//
// Cost: one write over each block as it is freed, and a copy on every realloc. Measured by
// `cargo test -p deetshealth --lib wipe_alloc -- --ignored --nocapture` (GLOBALS.md §10.2).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{compiler_fence, AtomicBool, Ordering};

pub struct Zeroing;

/// On in every build. Only the cost measurement below turns it off, to compare.
static WIPE: AtomicBool = AtomicBool::new(true);

unsafe impl GlobalAlloc for Zeroing {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        System.alloc_zeroed(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if WIPE.load(Ordering::Relaxed) {
            std::ptr::write_bytes(ptr, 0, layout.size());
            // The zeros must reach memory before the block is handed back: no reordering past
            // the free, and no removal of a write the compiler thinks nobody reads.
            compiler_fence(Ordering::SeqCst);
            std::hint::black_box(ptr);
        }
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let new = self.alloc(new_layout);
        if !new.is_null() {
            std::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
            self.dealloc(ptr, layout);
        }
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// A load shaped like the record view's: parse a record's raw JSON and drop it, many times.
    fn workload() -> u128 {
        let raw = r#"{"resourceType":"Observation","id":"o1","status":"final","category":[{"coding":[{"code":"laboratory"}]}],
            "code":{"coding":[{"system":"http://loinc.org","code":"2345-7","display":"Glucose"}],"text":"Glucose"},
            "effectiveDateTime":"2024-03-05T09:00:00-05:00","valueQuantity":{"value":5.10,"unit":"mmol/L"},
            "referenceRange":[{"low":{"value":3.9},"high":{"value":5.5}}],"note":[{"text":"a note of some length for the test"}]}"#;
        let started = Instant::now();
        for _ in 0..20_000 {
            let v: serde_json::Value = serde_json::from_str(raw).unwrap();
            let mut s = v.to_string();
            s.push_str(&v["id"].to_string());
            std::hint::black_box(s);
        }
        started.elapsed().as_millis()
    }

    #[test]
    #[ignore = "a measurement, not a check: run with --ignored --nocapture"]
    fn the_cost_of_wiping() {
        workload();
        WIPE.store(false, Ordering::Relaxed);
        let off = workload();
        WIPE.store(true, Ordering::Relaxed);
        let on = workload();
        println!("20 000 parses: {off} ms without the wipe, {on} ms with it");
    }

    #[test]
    fn a_block_keeps_its_bytes_through_realloc() {
        let mut v: Vec<u8> = (0..=255).collect();
        v.reserve(10_000);
        assert!(v.iter().copied().eq(0..=255));
        v.shrink_to_fit();
        assert!(v.iter().copied().eq(0..=255));
    }
}
