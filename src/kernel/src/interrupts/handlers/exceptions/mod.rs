mod arch;
pub use arch::*;

mod page_fault;

#[doc(hidden)]
#[inline(never)]
pub fn ex_handler(exception: ArchException) {
    trace!("Handling exception: {:X?}", exception);

    match exception {
        // Safety: Function is called once per this page fault exception.
        ArchException::PageFault(_, _, _, address) => unsafe {
            if let Err(err) = page_fault::handler(address) {
                panic!("error handling page fault: {}", err)
            }
        },

        exception => panic!("unhandled exception: {:?}", exception),
    };
}
