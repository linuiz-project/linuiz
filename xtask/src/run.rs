use std::path::Path;

#[derive(Debug, ValueEnum, Clone, Copy, PartialEq, Eq)]
pub enum Accelerator {
    Kvm,
    None,
}

#[derive(Debug, ValueEnum, Clone, Copy, PartialEq, Eq)]
pub enum Cpu {
    Host,
    Max,
    Qemu64,
    Rv64,
}

#[derive(Debug, ValueEnum, Clone, Copy, PartialEq, Eq)]
pub enum BlockDriver {
    Ahci,
    Nvme,
    Virtio,
}

#[derive(Parser)]
#[group(skip)]
pub struct Options {
    /// CPU type to emulate.
    #[arg(long, default_value = "qemu64")]
    cpu: Cpu,

    /// Emulation accelerator to use.
    #[arg(long, default_value = "none")]
    accel: Accelerator,

    /// Number of CPUs to emulate.
    #[arg(long, default_value = "4")]
    smp: usize,

    // RAM size in MB.
    #[arg(long, default_value = "512")]
    ram: usize,

    /// Enables debug logging.
    #[arg(long)]
    log: bool,

    /// Which type of block driver to use for root drive.
    #[arg(long, default_value = "virtio")]
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
    #[arg(short, long)]
    norun: bool,

    /// Puts QEMU in GDB debug mode, awaiting signal from the debugger to begin execution.
    #[arg(short, long)]
    gdb: bool,
}

pub fn run<P: AsRef<Path>>(
    sh: &xshell::Shell,
    temp_dir: P,
    options: Options,
) -> anyhow::Result<()> {
    if !options.nobuild {
        crate::build::build(sh, temp_dir.as_ref(), options.build_options)?;
    }

    let mut run_cmd = {
        match options.cpu {
            Cpu::Host | Cpu::Max | Cpu::Qemu64 => {
                // Create a temporary copy of the OVMF vars firmware to avoid overwriting
                // the fresh copy that's saved to the repository.
                let ovmf_vars_fd_copy = temp_dir.as_ref().join("vars.fd");
                sh.copy_file("run/ovmf/x86_64/vars.fd", &ovmf_vars_fd_copy)?;

                cmd!(sh, "qemu-system-x86_64")
                    .args([
                        "-drive",
                        "if=pflash,index=0,readonly=on,format=raw,file=run/ovmf/x86_64/code.fd",
                    ])
                    .args([
                        "-drive",
                        &format!(
                            "if=pflash,index=1,format=raw,file={}",
                            ovmf_vars_fd_copy.to_string_lossy()
                        ),
                    ])
                    .args(["-drive", "format=raw,file=fat:rw:run/system"])
            }

            Cpu::Rv64 => unimplemented!(),
        }
    }
    .arg("-no-shutdown")
    .arg("-no-reboot")
    .args(["-serial", "mon:stdio"])
    .args(["-drive", "format=raw,file=run/disk0.img,id=disk1,if=none"])
    .args(["-net", "none"])
    .args(["-M", "smm=off"])
    .args([
        "-machine",
        match (options.cpu, options.accel) {
            (Cpu::Rv64, Accelerator::None) => "virt",
            (Cpu::Rv64, accel) => panic!("invalid accelerator for RISC-V: {accel:?}"),

            (_, Accelerator::Kvm) => "q35,accel=kvm",
            (_, Accelerator::None) => "q35",
        },
    ])
    .args([
        "-cpu",
        match options.cpu {
            Cpu::Host => "host",
            Cpu::Max => "max",
            Cpu::Qemu64 => "qemu64",
            Cpu::Rv64 => "rv64",
        },
    ])
    .args(["-smp", &options.smp.to_string()])
    .args(["-m", &format!("{}M", options.ram)])
    .args([
        "-device",
        match options.block {
            BlockDriver::Ahci => "ahci,drive=disk1,serial=deadbeef",
            BlockDriver::Nvme => "nvme,drive=disk1,serial=deadbeef",
            BlockDriver::Virtio => "virtio-blk-pci,drive=disk1,serial=deadbeef",
        },
    ]);

    if options.log {
        run_cmd = run_cmd
            .args(["-d", "int,guest_errors"])
            .args(["-D", ".debug/qemu.log"]);
    }

    if options.nographic {
        run_cmd = run_cmd.arg("-nographic");
    }

    if options.gdb {
        run_cmd = run_cmd.args(["-S", "-s"]);
    }

    if options.norun {
        println!("cmd: {run_cmd}");
    } else {
        run_cmd.run()?;
    }

    Ok(())
}
