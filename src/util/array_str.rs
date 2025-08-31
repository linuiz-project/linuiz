use core::{ascii::Char, mem::transmute_copy};

#[repr(transparent)]
#[derive(PartialEq, Eq)]
pub struct AsciiStr<const N: usize>([Char; N]);

impl<const N: usize> AsciiStr<N> {
    pub const fn new(bytes: [u8; N]) -> Option<Self> {
        if bytes.is_ascii() {
            // Safety:
            // - `bytes` and `Self.0` are size `N`.
            // - `bytes` is checked valid as ASCII bytes, so the `u8`s are directly
            //   transmutable to `core::ascii::Char`.
            let bytes_ascii = unsafe { transmute_copy(&bytes) };
            Some(Self(bytes_ascii))
        } else {
            None
        }
    }

    pub fn new_lossy(mut bytes: [u8; N]) -> Self {
        bytes
            .iter_mut()
            .filter(|byte| !byte.is_ascii())
            .for_each(|byte| *byte = b'?');

        Self({
            // Safety:
            // - `bytes` and `Self.0` are size `N`.
            // - Non-ASCII `u8`s in `bytes` have been replaced, so the `u8`s are directly
            //   transmutable to `core::ascii::Char`.
            unsafe { transmute_copy(&bytes) }
        })
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<const N: usize> core::fmt::Debug for AsciiStr<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_str().fmt(f)
    }
}

impl<const N: usize> core::fmt::Display for AsciiStr<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_str().fmt(f)
    }
}
