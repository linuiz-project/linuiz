use uefi::{
    proto::media::file::{File, FileAttribute, FileMode, RegularFile},
    ResultExt,
};

pub fn open_file<F: File>(file: &mut F, name: &str) -> RegularFile {
    debug!("Attempting to load file system object: {}", name);
    match file.open(name, FileMode::Read, FileAttribute::READ_ONLY) {
        // this is unsafe due to the possibility of passing an invalid file handle to external code
        Ok(completion) => unsafe { RegularFile::new(completion.expect("failed to find file")) },
        Err(error) => panic!("{:?}", error),
    }
}

pub fn read_file(file: &mut RegularFile, position: u64, buffer: &mut [u8]) {
    debug!("Reading file contents into memory (pos {}).", position);
    file.set_position(position)
        .expect_success("failed to set position of file");
    file.read(buffer)
        .expect_success("failed to read file into memory");
}
