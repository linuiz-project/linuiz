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
    #[clap(arg_enum, long, default_value = "kvm64")]
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

    /// Which type of block driver to use for root drive.
    #[clap(arg_enum, long, default_value = "virt-io")]
    block: BlockDriver,
}

pub fn run(options: Options) -> Result<(), xshell::Error> {
    let shell = xshell::Shell::new()?;

    let machine_str = format!("q35{}", if let Accelerator::KVM = options.accel { ",accel=kvm" } else { "" });
    let cpu_str = format!("{:?}", options.cpu).to_lowercase();
    let smp_str = format!("{}", options.smp);
    let ram_str = format!("{}", options.ram);
    let block_driver_str = format!("{:?}", options.block);
    let mut log_str = vec![];
    if options.log {
        log_str.push(String::from("-d"));
        log_str.push(String::from("int,guest_errors"));
        log_str.push(String::from("-D"));
        log_str.push(String::from(".debug/qemu.log"));
    }

    cmd!(
        shell,
        "
        qemu-system-x86_64
            -no-reboot
            -bios /usr/share/ovmf/OVMF.fd
            -machine {machine_str}
            -cpu {cpu_str}
            -smp {smp_str}
            -m {ram_str}M
            -serial mon:stdio
            -net none
            -display none
            -drive format=raw,file=fat:rw:.hdd/root/
            -drive format=raw,file=.hdd/disk0.img,id=disk1,if=none
            -device {block_driver_str},drive=disk1,serial=deadbeef
            {log_str...}
        "
    )
    .run()?;

    Ok(())
}
