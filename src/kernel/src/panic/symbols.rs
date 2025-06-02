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

static SYMBOL_TABLE: spin::Once<(SymbolTable<AnyEndian>, StringTable)> = spin::Once::new();

pub fn parse(kernel_file_request: &limine::request::ExecutableFileRequest) {
    if let Err(error) = SYMBOL_TABLE.try_call_once(|| {
        let kernel_file = kernel_file_request
            .get_response()
            .map(|response| {
                // Safety: Bootloader guarantees the address and size of the executable file will be correct.
                //         Additionally, given the context, it also guarantees the file will be mapped into memory.
                unsafe {
                    core::slice::from_raw_parts::<'static>(
                        response.file().addr(),
                        response.file().size().try_into().unwrap(),
                    )
                }
            })
            .ok_or(Error::NoKernelFile)?;

        let kernel_elf = ElfBytes::<'static, AnyEndian>::minimal_parse(kernel_file)?;

        match kernel_elf.symbol_table() {
            Ok(Some(symbol_table)) => Ok(symbol_table),
            Ok(None) => Err(Error::NoSymbolTable),
            Err(error) => Err(Error::ElfParser(error)),
        }
    }) {
        error!("Failed to parse kernel symbols: {error:?}");
    }
}

pub fn get_name(address: Address<Virtual>) -> Option<&'static str> {
    let (symbols, strings) = SYMBOL_TABLE.get()?;

    let symbol = symbols.iter().find(|symbol| {
        (symbol.st_value..(symbol.st_value + symbol.st_size)).contains(&address.get().try_into().unwrap())
    })?;

    let Ok(string) = strings.get(symbol.st_name.try_into().unwrap()) else {
        error!("Could not parse symbol name: {:#X}", symbol.st_name);
        return None;
    };

    Some(string)
}
