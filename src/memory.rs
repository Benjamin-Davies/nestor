//! Because nestor eagerly loads everything into memory, it can be quite a
//! memory hog. This file contains a few platform-specific methods to avoid
//! bringing the whole system down as a consequence.

#[cfg(target_os = "linux")]
pub fn im_a_sacrifice() {
    if let Err(err) = std::fs::write("/proc/self/oom_score_adj", "1000") {
        tracing::warn!("Failed to adjust OOM score: {err}");
    }
}

#[cfg(target_os = "macos")]
pub fn im_a_sacrifice() {
    std::thread::spawn(macos::poll_mem_pressure);
}

#[cfg(target_os = "macos")]
mod macos {
    use std::{io, mem, process, thread, time::Duration};

    fn pressure_level() -> io::Result<i32> {
        let mut level: libc::c_int = 0;
        let mut len = mem::size_of::<libc::c_int>();
        let rc = unsafe {
            libc::sysctlbyname(
                c"kern.memorystatus_vm_pressure_level".as_ptr(),
                (&raw mut level).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };

        if rc != 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(level)
        }
    }

    pub fn poll_mem_pressure() {
        let mut first_error = true;
        loop {
            match pressure_level() {
                Ok(pressure) => {
                    if pressure == 4 {
                        process::exit(1);
                    }
                }
                Err(err) => {
                    if first_error {
                        tracing::warn!("Failed to get memory pressure: {err}");
                        first_error = false;
                    }
                }
            }

            thread::sleep(Duration::from_millis(500));
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn im_a_sacrifice() {
    tracing::info!("Memory tweaks not supported");
}
