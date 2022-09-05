use clap::{clap_derive::ArgEnum, Parser};
use xshell::cmd;

#[derive(ArgEnum, Clone, Copy, PartialEq, Eq)]
pub enum Accelerator {
    KVM,
    None,
}

impl core::fmt::Debug for Accelerator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Accelerator::KVM => "q35,accel=kvm",
            Accelerator::None => "q35",
        })
    }
}

#[derive(ArgEnum, Debug, Clone, Copy)]
pub enum CPU {
    Host,
    Max,
    QEMU64,
    RV64,
}

impl CPU {
    pub const fn as_str(&self) -> &str {
        match self {
            CPU::Host => "host",
            CPU::Max => "max",
            CPU::QEMU64 => "qemu64",
            CPU::RV64 => "rv64",
        }
    }
}

#[derive(ArgEnum, Clone, Copy)]
pub enum BlockDriver {
    AHCI,
    NVME,
    VirtIO,
}

impl core::fmt::Debug for BlockDriver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            BlockDriver::AHCI => "ahci",
            BlockDriver::NVME => "nvme",
            BlockDriver::VirtIO => "virtio-blk-pci",
        })
    }
}

#[derive(Parser)]
pub struct Options {
    /// CPU type to emulate.
    #[clap(arg_enum, long, default_value = "qemu64")]
    cpu: CPU,

    #[clap(arg_enum, long, default_value = "none")]
    accel: Accelerator,

    /// Number of CPUs to emulate.
    #[clap(long, default_value = "4")]
    smp: usize,

    // RAM size in MB.
    #[clap(long, default_value = "512")]
    ram: usize,

    /// Enables debug logging to the specified location.
    #[clap(long)]
    log: bool,

    /// Which type of block driver to use for root drive.
    #[clap(arg_enum, long, default_value = "virt-io")]
    block: BlockDriver,

    #[clap(long)]
    no_build: bool,

    #[clap(flatten)]
    build_options: crate::build::Options,
}

pub fn run(shell: &xshell::Shell, options: Options) -> Result<(), xshell::Error> {
    if !options.no_build {
        crate::build::build(shell, options.build_options)?;
    }

    let qemu_exe_str = match options.cpu {
        CPU::RV64 => "qemu-system-riscv64",
        _ => "qemu-system-x86_64",
    };

    let mut arguments = vec![];

    arguments.push("-machine");
    arguments.push(match options.cpu {
        CPU::RV64 => "virt",
        CPU::Host | CPU::Max | CPU::QEMU64 if options.accel == Accelerator::KVM => "q35,accel=kvm",
        _ => "q35",
    });

    arguments.push("-cpu");
    arguments.push(options.cpu.as_str());

    arguments.push("-smp");
    let smp_string = options.smp.to_string();
    arguments.push(&smp_string);

    arguments.push("-m");
    let memory_string = format!("{}M", options.ram);
    arguments.push(&memory_string);

    arguments.push("-device");
    let device_string = format!("{:?},drive=disk1,serial=deadbeef", options.block);
    arguments.push(&device_string);

    if options.log {
        arguments.push("-d");
        arguments.push("int,guest_errors");
        arguments.push("-D");
        arguments.push(".debug/qemu.log");
    }

    match options.cpu {
        CPU::RV64 => {
            arguments.push("-bios");
            arguments.push("resources/fw_jump.fd");
            arguments.push("-kernel");
            arguments.push(".hdd/root/linuiz/kernel_rv64.elf");
        }
        _ => {
            arguments.push("-bios");
            arguments.push("resources/OVMF.fd");
            arguments.push("-drive");
            arguments.push("format=raw,file=fat:rw:.hdd/root/");
        }
    };

    cmd!(
        shell,
        "
        {qemu_exe_str}
            {arguments...}
            -serial mon:stdio
            -drive format=raw,file=.hdd/disk0.img,id=disk1,if=none
            -net none
        "
    )
    .run()?;

    Ok(())
}
