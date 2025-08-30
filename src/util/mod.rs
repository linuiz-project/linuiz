pub mod sync;

pub trait InteriorBorrow {
    type RefType<'a, T>
    where
        T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T;
}

pub struct SharedBorrow;
impl InteriorBorrow for SharedBorrow {
    type RefType<'a, T>
        = &'a T
    where
        T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T {
        r
    }
}

pub struct ExclusiveBorrow;
impl InteriorBorrow for ExclusiveBorrow {
    type RefType<'a, T>
        = &'a mut T
    where
        T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T {
        r
    }
}
