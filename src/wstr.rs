//! This module contains constructs to work with 16-bit characters (UCS-2 or UTF-16)
#![allow(dead_code)]

use std::marker::PhantomData;
use std::num::NonZeroU16;
use std::ptr::NonNull;

/// A safe iterator over a LPWSTR
/// (aka a pointer to a series of UTF-16 code units terminated by a NULL).
pub struct WStrUnits<'a> {
    // The pointer must never be null...
    lpwstr: NonNull<u16>,
    // ...and the memory it points to must be valid for this lifetime.
    lifetime: PhantomData<&'a [u16]>,
    index: usize,
}

impl WStrUnits<'_> {
    /// Create the iterator. Returns `None` if `lpwstr` is null.
    ///
    /// SAFETY: `lpwstr` must point to a null-terminated wide string that lives
    /// at least as long as the lifetime of this struct.
    pub unsafe fn new(lpwstr: *const u16) -> Option<Self> {
        Some(Self { lpwstr: NonNull::new(lpwstr as _)?, lifetime: PhantomData, index: 0 })
    }

    pub fn peek(&self) -> Option<NonZeroU16> {
        // SAFETY: It's always safe to read the current item because we don't
        // ever move out of the array's bounds.
        unsafe { NonZeroU16::new(*self.lpwstr.as_ptr()) }
    }

    /// Advance the iterator while `predicate` returns true.
    /// Returns the number of items it advanced by.
    pub fn advance_while<P: FnMut(NonZeroU16) -> bool>(&mut self, mut predicate: P) -> usize {
        let mut counter = 0;
        while let Some(w) = self.peek() {
            if !predicate(w) {
                break;
            }
            counter += 1;
            self.next();
        }
        counter
    }

    //pub fn get_index(&self) -> usize {
    //    self.index
    //}
}

impl Iterator for WStrUnits<'_> {
    // This can never return zero as that marks the end of the string.
    type Item = NonZeroU16;
    fn next(&mut self) -> Option<NonZeroU16> {
        // SAFETY: If NULL is reached we immediately return.
        // Therefore it's safe to advance the pointer after that.
        self.index += 1;
        let next = self.peek()?;
        self.lpwstr = unsafe { NonNull::new_unchecked(self.lpwstr.as_ptr().add(1)) };
        Some(next)
    }
}
