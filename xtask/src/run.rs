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
#[group(skip)]
pub struct Options {
    /// CPU type to emulate.
    #[arg(value_enum, long, default_value = "qemu64")]
    cpu: CPU,

    /// Emulation accelerator to use.
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

    /// Skips invoking the build pipeline for the kernel.
    #[arg(long)]
    nobuild: bool,

    /// Runs the kernel in serial-only mode (no graphics driving).
    #[arg(long)]
    nographic: bool,

    #[clap(flatten)]
    build_options: crate::build::Options,

    /// Skips execution and only prints the QEMU command that would have been executed.
    #[arg(long)]
    mock: bool,

    /// Puts QEMU in GDB debug mode, awaiting signal from the debugger to begin execution.
    #[arg(short, long)]
    gdb: bool,
}

pub fn run(sh: &xshell::Shell, options: Options) -> Result<(), xshell::Error> {
    if !options.nobuild {
        crate::build::build(sh, options.build_options)?;
    }

    let qemu_exe_str = match options.cpu {
        CPU::Rv64 => "qemu-system-riscv64",
        _ => "qemu-system-x86_64",
    };

    let mut cmd = cmd!(
        sh,
        "
        {qemu_exe_str}
            -no-shutdown
            -no-reboot
            -serial mon:stdio
            -drive format=raw,file=build/disk0.img,id=disk1,if=none
            -net none
            -M smm=off
        "
    );

    cmd = cmd.args([
        "-machine",
        match options.cpu {
            CPU::Rv64 => "virt",
            CPU::Host | CPU::Max | CPU::Qemu64 if options.accel == Accelerator::Kvm => "q35,accel=kvm",
            _ => "q35",
        },
    ]);

    cmd = cmd
        // cpu
        .args(["-cpu", options.cpu.as_str()])
        // smp
        .arg("--smp")
        .arg(options.smp.to_string())
        // memory
        .arg("-m")
        .arg(format!("{}M", options.ram))
        // disk
        .arg("-device")
        // TODO this doesn't work for AHCI
        .arg(format!("{:?},drive=disk1,serial=deadbeef", options.block));

    if options.log {
        if !sh.path_exists(".debug/") {
            sh.create_dir(".debug/")?;
        }

        cmd = cmd.args(["-d", "int,guest_errors", "-D", ".debug/qemu.log"]);
    }

    cmd = match options.cpu {
        CPU::Rv64 => unimplemented!(),
        _ => cmd.args(["-bios", "resources/OVMF.fd", "-drive", "format=raw,file=fat:rw:build/root/"]),
    };

    if options.nographic {
        cmd = cmd.arg("-nographic");
    }

    if options.gdb {
        cmd = cmd.args(["-S", "-s"]);
    }

    if options.mock {
        println!("cmd: {}", cmd.to_string());
        Ok(())
    } else {
        cmd.run()
    }
}
