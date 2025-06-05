use core::num::NonZero;

static HHDM: spin::Once<Hhdm> = spin::Once::new();

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hhdm(NonZero<usize>);

impl Hhdm {
    pub fn init(hhdm_request: &limine::request::HhdmRequest) {
        HHDM.call_once(|| {
            // Zero-based memory offset of the start of the HHDM.
            let hhdm_offset = hhdm_request
                .get_response()
                .expect("bootloader did not provide response to higher-half direct map request")
                .offset();

            debug!("HHDM @ {hhdm_offset:#X}");

            Hhdm(NonZero::new(usize::try_from(hhdm_offset).unwrap()).unwrap())
        });
    }

    pub fn offset() -> NonZero<usize> {
        HHDM.get()
            .expect("higher-half direct map has not been initialized")
            .0
    }
}
