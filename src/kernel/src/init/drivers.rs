use crate::proc::Artifact;
use elf::{endian::AnyEndian, ElfBytes};
use try_alloc::vec::TryVec;

pub fn load_artifacts<'a>() -> Option<TryVec<Artifact>> {
    #[limine::limine_tag]
    static LIMINE_MODULES: limine::ModuleRequest = limine::ModuleRequest::new(crate::boot::LIMINE_REV);

    let mut artifacts = TryVec::new();

    let Some(modules) = LIMINE_MODULES.get_response() else { return None };
    for module in modules
        .modules()
        .iter()
        // Filter out modules that don't end with our driver postfix.
        .filter(|module| module.path().ends_with("drivers"))
    {
        let archive = tar_no_std::TarArchiveRef::new(module.data());
        for entry in archive.entries() {
            debug!("Attempting to parse driver blob: {}", entry.filename());

            match ElfBytes::<AnyEndian>::minimal_parse(entry.data()) {
                Ok(elf) => {
                    // TODO smarter loading & failure behaviour
                    artifacts.push(Artifact::try_from(&elf).unwrap()).ok();
                }
                Err(err) => warn!("Failed to parse driver blob into ELF: {:?}", err),
            }
        }
    }

    Some(artifacts)
}
