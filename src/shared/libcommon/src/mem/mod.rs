
mod volatile;

pub use volatile::*;

pub trait InteriorRef {
    type RefType<'a, T>
    where
        T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T;
}

pub struct Ref;
impl InteriorRef for Ref {
    type RefType<'a, T> = &'a T where T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T {
        r
    }
}

pub struct Mut;
impl InteriorRef for Mut {
    type RefType<'a, T> = &'a mut T where T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T {
        &**r
    }
}
