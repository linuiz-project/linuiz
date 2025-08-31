use libsys::syscall::klog::Error;

pub fn process_klog(_level: log::Level, _str_address: usize, _str_len: usize) -> Result<(), Error> {
    todo!()

    // let str_ptr = core::ptr::with_exposed_provenance::<u8>(str_ptr_arg);

    // // TODO abstract this into a function
    // LocalState::with_scheduler(|scheduler| {
    //     use crate::task::Error as TaskError;
    //     use libsys::{Address, page_size};

    //     let str_start = str_ptr.addr();
    //     let str_end = str_start + str_len;

    //     let task = scheduler.task_mut().ok_or()?;
    //     for address in (str_start..str_end)
    //         .step_by(page_size())
    //         .map(Address::new_truncate)
    //     {
    //         match task.demand_map(address) {
    //             Ok(()) | Err(TaskError::AlreadyMapped) => {}

    //             err => {
    //                 warn!("Failed to demand map: {err:X?}");
    //                 return Err(Error::NotMapped);
    //             }
    //         }
    //     }

    //     Ok(Success::Ok)
    // })?;

    // // Safety: TODO
    // let str_slice = unsafe { core::slice::from_raw_parts(str_ptr, str_len) };
    // let str = core::str::from_utf8(str_slice)?;

    // log!(level, "[KLOG]: {str}");

    // Ok(())
}
