use elf::{ElfBytes, endian::AnyEndian, string_table::StringTable, symbol::SymbolTable};
use libsys::{Address, Virtual};

#[derive(Debug, Error)]
pub enum Error {
    #[error("bootloader did not provide kernel file")]
    NoKernelFile,

    #[error("provided file is malformed")]
    ElfParser(#[from] elf::ParseError),

    #[error("no symbol table found")]
    NoSymbolTable,
}

crate::singleton! {
    pub Symbols {
        tables: Option<(SymbolTable<'static, AnyEndian>, StringTable<'static>)>,
    }

    fn init(kernel_file_request: &limine::request::ExecutableFileRequest) {
        let Some(response) = kernel_file_request.get_response() else {
            error!("Bootloader didn't provide response to kernel file request.");
            return Self { tables: None };
        };

        // Safety: Bootloader guarantees the address and size of the executable file will be correct.
        //         Additionally, given the context, it also guarantees the file will be mapped into memory.
        let kernel_file = unsafe {
            core::slice::from_raw_parts::<'static>(
                response.file().addr(),
                response.file().size().try_into().unwrap(),
            )
        };

        let Ok(kernel_elf) =
            ElfBytes::<'static, AnyEndian>::minimal_parse(kernel_file).inspect_err(|error| {
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
            tables: Some(symbol_table)
        }
    }
}

impl Symbols {
    pub fn get_name(address: Address<Virtual>) -> Option<&'static str> {
        let (symbols, strings) = Symbols::get_static().tables.as_ref()?;

        let symbol = symbols.iter().find(|symbol| {
            (symbol.st_value..(symbol.st_value + symbol.st_size))
                .contains(&address.get().try_into().unwrap())
        })?;

        let Ok(string) = strings.get(symbol.st_name.try_into().unwrap()) else {
            error!("Could not parse symbol name: {:#X}", symbol.st_name);
            return None;
        };

        Some(string)
    }
}
