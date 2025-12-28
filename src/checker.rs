use crate::try_umount::DRIVER_FD;

const KSU_INSTALL_MAGIC1: u32 = 0xDEADBEEF;
const KSU_INSTALL_MAGIC2: u32 = 0xCAFEBABE;

#[repr(C)]
struct GetInfoCmd {
    version: u32,
    flags: u32,
}

fn grab_fd() -> i32 {
    let mut fd = -1;

    unsafe {
        libc::syscall(
            libc::SYS_reboot,
            KSU_INSTALL_MAGIC1,
            KSU_INSTALL_MAGIC2,
            0,
            &mut fd,
        );
    };

    fd
}

fn info() -> Option<GetInfoCmd> {
    let mut cmd = GetInfoCmd {
        version: 0,
        flags: 0,
    };

    let fd = *DRIVER_FD.get_or_init(grab_fd());
    let ret = unsafe { libc::ioctl(fd as libc::c_int, KSU_IOCTL_GET_INFO, &mut cmd) };

    if ret < 0 { None } else { Some(cmd) }
}

fn version() -> Option<u32> {
    info().map(|info| info.version)
}

pub fn check() {
    let status = version().is_some_and(|v| {
        log::info!("KernelSU Version: {v}");
        true
    });

    if !status {
        log::error!("only support KernelSU!!");
        panic!();
    }
}
