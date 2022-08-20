use clap::{clap_derive::ArgEnum, Parser};
use xshell::cmd;

#[derive(ArgEnum, Clone, Copy)]
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
    KVM64,
    QEMU64,
    RV64,
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
    /// Which simulator to use when executing the binary.
    #[clap(arg_enum, long, default_value = "none")]
    accel: Accelerator,

    /// CPU type to emulate.
    #[clap(arg_enum, long, default_value = "qemu64")]
    cpu: CPU,

    /// Number of CPUs to emulate.
    #[clap(long, default_value = "4")]
    smp: usize,

    // RAM size in MB.
    #[clap(long, default_value = "512")]
    ram: usize,

    /// Enables debug logging to the specified location.
    #[clap(long)]
    log: bool,

    /// Stops QEMU from automatically exiting when a triple fault occurs.
    #[clap(long)]
    no_shutdown: bool,

    /// Which type of block driver to use for root drive.
    #[clap(arg_enum, long, default_value = "virt-io")]
    block: BlockDriver,
}

pub fn run(options: Options) -> Result<(), xshell::Error> {
    let shell = xshell::Shell::new()?;

    let qemu_exe_type = match options.cpu {
        CPU::RV64 => "qemu-system-riscv64",
        _ => "qemu-system-x86_64",
    };
    let machine_str = match options.cpu {
        CPU::RV64 => {
            if let Accelerator::KVM = options.accel {
                panic!("cannot use KVM acceleration with rv64");
            } else {
                "virt".to_string()
            }
        }
        _ => format!("q35{}", if let Accelerator::KVM = options.accel { ",accel=kvm" } else { "" }),
    };
    let cpu_str = format!("{:?}", options.cpu).to_lowercase();
    let smp_str = format!("{}", options.smp);
    let ram_str = format!("{}", options.ram);
    let block_driver_str = format!("{:?}", options.block);
    let log_str = if options.log { vec!["-d", "int,guest_errors", "-D", ".debug/qemu.log"] } else { vec![] };
    let files_str = match options.cpu {
        CPU::RV64 => {
            vec!["-bios", "resources/fw_jump.fd", "-kernel", ".hdd/root/linuiz/kernel_rv64.elf"]
        }
        _ => {
            vec!["-bios", "resources/OVMF.fd", "-drive", "format=raw,file=fat:rw:.hdd/root/"]
        }
    };
    let no_shutdown = if options.no_shutdown { vec!["-no-shutdown"] } else { vec![] };

    cmd!(
        shell,
        "
        {qemu_exe_type}
            -no-reboot
            {files_str...}
            -machine {machine_str}
            -cpu {cpu_str}
            -smp {smp_str}
            -m {ram_str}M
            -serial mon:stdio
            -net none
            -drive format=raw,file=.hdd/disk0.img,id=disk1,if=none
            -device {block_driver_str},drive=disk1,serial=deadbeef
            {log_str...}
            {no_shutdown...}
        "
    )
    .run()?;

    Ok(())
}
