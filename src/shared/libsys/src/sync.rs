use core::sync::atomic::AtomicBool;
use uuid::Uuid;

crate::er

enum Ownership<T> {
    Unowned(T),
    Owned(Uuid)
}

pub struct OwnershipCell<T> {
    lock: AtomicBool,
    ownership: Ownership<T>,
}

impl<T> OwnershipCell<T> {
    pub fn new(item: T) -> Self {
        Self { lock: AtomicBool::new(false), ownership: Ownership::Unowned(item)}
    }

    pub fn take_ownership(&mut self) -> (Uuid, T) {
        match self.ownership {
            Ownership::Owned(_) => panic!("cell already owned"),
            Ownership::Unowned(item) => {
                let uuid = Uuid::new_v4();

                self.ownership = 

                (, item)
            }
        }
        


    }

    pub fn return_ownership(&mut self, uuid: Uuid, item: T) {
        match self.ownership {

        }
    }
}
