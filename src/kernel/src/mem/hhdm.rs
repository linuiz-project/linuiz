static HHDM: spin::Once<Hhdm> = spin::Once::new();

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hhdm(usize);

impl Hhdm {
    pub fn init(hhdm_request: &limine::request::HhdmRequest) {
        HHDM.call_once(|| {
            // Zero-based memory offset of the start of the HHDM.
            let hhdm_offset = hhdm_request
                .get_response()
                .expect("bootloader did not provide response to higher-half direct map request")
                .offset();

            debug!("HHDM @ {hhdm_offset:#X}");

            Hhdm(usize::try_from(hhdm_offset).unwrap())
        });
    }

    pub fn ptr_offset(byte_offset: usize) -> usize {
        HHDM.get()
            .expect("higher-half direct map has not been initialized")
            .0
            + byte_offset
    }
}
