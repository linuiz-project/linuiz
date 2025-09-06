use core::num::NonZero;

pub const fn align_up(value: usize, alignment_bits: NonZero<u32>) -> usize {
    (value.wrapping_neg() & (1usize << alignment_bits.get()).wrapping_neg()).wrapping_neg()
}

pub const fn align_up_div(value: usize, alignment_bits: NonZero<u32>) -> usize {
    align_up(value, alignment_bits) / (1usize << alignment_bits.get())
}

pub const fn align_down(value: usize, alignment_bits: NonZero<u32>) -> usize {
    (value >> alignment_bits.get()) << alignment_bits.get()
}

pub const fn align_down_div(value: usize, alignment_bits: NonZero<u32>) -> usize {
    value >> alignment_bits.get()
}

#[cfg(test)]
mod tests {
    use core::num::NonZero;

    #[test]
    pub fn align_up() {
        assert_eq!(
            super::align_up(0xFC, unsafe { NonZero::new_unchecked(4) }),
            0x100
        );
    }

    #[test]
    pub fn align_up_div() {
        assert_eq!(
            super::align_up_div(0xFC, unsafe { NonZero::new_unchecked(4) }),
            0x10
        );
    }

    #[test]
    pub fn align_down() {
        assert_eq!(
            super::align_down(0xFC, unsafe { NonZero::new_unchecked(4) }),
            0xF0
        );
    }

    #[test]
    pub fn align_down_div() {
        assert_eq!(
            super::align_down_div(0xFC, unsafe { NonZero::new_unchecked(4) }),
            0xF
        );
    }
}
