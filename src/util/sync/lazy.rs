use crate::util::sync::Once;
use core::cell::Cell;

/// A value which is initialized on the first access.
///
/// This type is a thread-safe lazy initializer, and can be used in statics.
pub struct Lazy<T, F = fn() -> T> {
    cell: Once<T>,
    init: Cell<Option<F>>,
}

// Safety:
// We never create a `&F` from a `&Lazy<T, F>` so it is fine to not impl `Sync`
// for `F`.
//
// We do create a `&mut Option<F>` in `Self::force`, but this is properly
// synchronized, so it only happens once so it also does not contribute to this
// impl.
unsafe impl<T, F: Send> Sync for Lazy<T, F> where Once<T>: Sync {}
// auto-derived `Send` impl is good enough.

impl<T, F> Lazy<T, F> {
    /// Creates a new lazy value with the given initializing function.
    pub const fn new(f: F) -> Self {
        Self {
            cell: Once::new(),
            init: Cell::new(Some(f)),
        }
    }
}

impl<T, F: FnOnce() -> T> Lazy<T, F> {
    /// Forces the evaluation of this lazy value and returns a reference to
    /// result. This is equivalent to the `Deref` impl, but is explicit.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::util::sync::Lazy;
    ///
    /// let lazy = Lazy::new(|| 92);
    ///
    /// assert_eq!(Lazy::force(&lazy), &92);
    /// assert_eq!(&*lazy, &92);
    /// ```
    pub fn force(this: &Self) -> &T {
        this.cell.call_once(|| match this.init.take() {
            Some(f) => f(),
            None => panic!("`Lazy` instance has previously been poisoned"),
        })
    }
}

impl<T, F: FnOnce() -> T> core::ops::Deref for Lazy<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        Self::force(self)
    }
}

impl<T: core::fmt::Debug, F> core::fmt::Debug for Lazy<T, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_tuple("Lazy");

        if let Some(x) = self.cell.get() {
            d.field(&x);
        } else {
            d.field(&format_args!("<uninit>"));
        }

        d.finish()
    }
}
