use core::cell::UnsafeCell;
use uefi::{prelude::BootServices, proto::Protocol, Handle, Status};

pub fn get_protocol<P: Protocol>(boot_services: &BootServices, handle: Handle) -> Option<&mut P> {
    acquire_protocol_unwrapped(boot_services.handle_protocol(handle))
}

pub fn locate_protocol<P: Protocol>(boot_services: &BootServices) -> Option<&mut P> {
    acquire_protocol_unwrapped(boot_services.locate_protocol::<P>())
}

fn acquire_protocol_unwrapped<P: Protocol>(result: uefi::Result<&UnsafeCell<P>>) -> Option<&mut P> {
    if let Ok(completion) = result {
        if completion.status() == Status::SUCCESS {
            Some(unsafe { &mut *(completion.unwrap().get()) })
        } else {
            None
        }
    } else {
        None
    }
}
