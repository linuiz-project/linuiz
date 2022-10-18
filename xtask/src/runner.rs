use clap::ValueEnum;
use xshell::cmd;

#[derive(ValueEnum, Clone, Copy, PartialEq, Eq)]
pub enum Accelerator {
    Kvm,
    None,
}

impl core::fmt::Debug for Accelerator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Accelerator::Kvm => "q35,accel=kvm",
            Accelerator::None => "q35",
        })
    }
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum CPU {
    Host,
    Max,
    Qemu64,
    Rv64,
}

impl CPU {
    pub const fn as_str(&self) -> &str {
        match self {
            CPU::Host => "host",
            CPU::Max => "max",
            CPU::Qemu64 => "qemu64",
            CPU::Rv64 => "rv64",
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
pub enum BlockDriver {
    Ahci,
    Nvme,
    Virtio,
}

impl core::fmt::Debug for BlockDriver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            BlockDriver::Ahci => "ahci",
            BlockDriver::Nvme => "nvme",
            BlockDriver::Virtio => "virtio-blk-pci",
        })
    }
}

#[derive(clap::Parser)]
pub struct Options {
    /// CPU type to emulate.
    #[arg(value_enum, long, default_value = "qemu64")]
    cpu: CPU,

    #[arg(value_enum, long, default_value = "none")]
    accel: Accelerator,

    /// Number of CPUs to emulate.
    #[arg(long, default_value = "4")]
    smp: usize,

    // RAM size in MB.
    #[arg(long, default_value = "512")]
    ram: usize,

    /// Enables debug logging to the specified location.
    #[arg(long)]
    log: bool,

    /// Which type of block driver to use for root drive.
    #[arg(value_enum, long, default_value = "virtio")]
    block: BlockDriver,

    #[arg(long)]
    no_build: bool,

    // #[command(flatten)]
    // build_options: crate::build::Options,
}

pub fn run(shell: &xshell::Shell, options: Options) -> Result<(), xshell::Error> {
    // if !options.no_build {
    //     crate::build::build(shell, options.build_options)?;
    // }

    let qemu_exe_str = match options.cpu {
        CPU::Rv64 => "qemu-system-riscv64",
        _ => "qemu-system-x86_64",
    };

    let mut arguments = vec![];

    arguments.push("-machine");
    arguments.push(match options.cpu {
        CPU::Rv64 => "virt",
        CPU::Host | CPU::Max | CPU::Qemu64 if options.accel == Accelerator::Kvm => "q35,accel=kvm",
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

    // TODO this doesn't work for AHCI
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
        CPU::Rv64 => {
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
            -no-shutdown
            -no-reboot
            -serial mon:stdio
            -drive format=raw,file=.hdd/disk0.img,id=disk1,if=none
            -net none
            -M smm=off
        "
    )
    .run()?;

    Ok(())
}
