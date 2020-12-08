use core::cell::UnsafeCell;
use uefi::{prelude::BootServices, proto::Protocol, Handle};

pub fn get_protocol_unwrap<P: Protocol>(
    boot_services: &BootServices,
    handle: Handle,
) -> Option<&mut P> {
    acquire_protocol_unwrapped(boot_services.handle_protocol(handle))
}

pub fn locate_protocol_unwrap<P: Protocol>(boot_services: &BootServices) -> Option<&mut P> {
    acquire_protocol_unwrapped(boot_services.locate_protocol::<P>())
}

fn acquire_protocol_unwrapped<P: Protocol>(result: uefi::Result<&UnsafeCell<P>>) -> Option<&mut P> {
    match result {
        Ok(unsafe_cell_completion) => {
            info!("Protocol found, attempting to acquire...");

            if !unsafe_cell_completion.status().is_success() {
                panic!(
                    "failed to locate and acquire protocol: {:?}",
                    unsafe_cell_completion.status()
                );
            } else {
                info!("Protocol acquired, attempting to unwrap...");
                Some(unsafe { &mut *(unsafe_cell_completion.unwrap().get() as *mut P) })
            }
        }
        Err(error) => panic!("{:?}", error.status()),
    }
}
