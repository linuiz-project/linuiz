use crate::util::sync::Once;
use elf::{ElfBytes, endian::AnyEndian, string_table::StringTable, symbol::SymbolTable};

#[derive(Debug, Error)]
pub enum Error {
    #[error("bootloader did not provide kernel file")]
    NoKernelFile,

    #[error("provided file is malformed")]
    ElfParser(#[from] elf::ParseError),

    #[error("no symbol table found")]
    NoSymbolTable,
}

pub struct KernelSymbols {
    tables: Option<(SymbolTable<'static, AnyEndian>, StringTable<'static>)>,
}

static KERNEL_SYMBOLS: Once<KernelSymbols> = Once::new();

impl KernelSymbols {
    pub fn init(kernel_file_request: &limine::request::ExecutableFileRequest) {
        KERNEL_SYMBOLS.call_once(|| {
            let Some(response) = kernel_file_request.get_response() else {
                error!("Bootloader didn't provide response to kernel file request.");
                return Self { tables: None };
            };

            // Safety: Bootloader guarantees the address and size of the executable file
            // will be correct.         Additionally, given the context, it also
            // guarantees the file will be mapped into         memory.
            let kernel_file = unsafe {
                core::slice::from_raw_parts::<'static>(
                    response.file().addr(),
                    response.file().size().try_into().unwrap(),
                )
            };

            let Ok(kernel_elf) = ElfBytes::<'static, AnyEndian>::minimal_parse(kernel_file)
                .inspect_err(|error| {
                    error!("Failed to parse kernel ELF: {error:?}");
                })
            else {
                return Self { tables: None };
            };

            let Ok(symbol_table) = kernel_elf.symbol_table().inspect_err(|error| {
                error!("Failed to parse kernel symbol table: {error:?}");
            }) else {
                return Self { tables: None };
            };

            let Some(symbol_table) = symbol_table else {
                error!("Kernel file has no symbol table.");
                return Self { tables: None };
            };

            Self {
                tables: Some(symbol_table),
            }
        });
    }

    fn get_static() -> Option<&'static Self> {
        KERNEL_SYMBOLS.get()
    }

    pub fn get_name(trace_address: usize) -> Option<&'static str> {
        let trace_address = u64::try_from(trace_address)
            .inspect_err(|error| {
                warn!("Failed to convert symbol address: {error:?}");
            })
            .ok()?;

        let (symbols, strings) = KernelSymbols::get_static()?.tables.as_ref()?;

        let symbol = symbols.iter().find(|symbol| {
            (trace_address >= symbol.st_value)
                && ((trace_address - symbol.st_value) <= symbol.st_size)
        })?;
        let symbol_name_index = usize::try_from(symbol.st_name)
            .inspect_err(|error| {
                warn!("Failed to convert symbol name index: {error:?}");
            })
            .ok()?;

        let Ok(string) = strings.get(symbol_name_index) else {
            error!("Could not parse symbol name: {:#X}", symbol.st_name);
            return None;
        };

        Some(string)
    }
}
