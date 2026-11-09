//! Give memory back to Windows while Themisio isn't being used, as Microsoft Edge does for
//! background tabs: when the window has been in the background for a while, or is minimized,
//! its working set is trimmed (pages it touches again come back on demand).

/// Seconds in the background before the working set is trimmed.
const AFTER: f64 = 20.0;

#[derive(Default)]
pub struct Trim {
    background_since: Option<f64>,
    trimmed: bool,
}

impl Trim {
    pub fn tick(&mut self, ctx: &egui::Context, now: f64) {
        let (focused, minimized) = ctx.input(|i| (i.viewport().focused.unwrap_or(true), i.viewport().minimized.unwrap_or(false)));
        if focused && !minimized {
            self.background_since = None;
            self.trimmed = false;
            return;
        }
        let since = *self.background_since.get_or_insert(now);
        if self.trimmed {
            return;
        }
        if minimized || now - since >= AFTER {
            trim_working_set();
            self.trimmed = true;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(AFTER - (now - since) + 0.5));
        }
    }
}

/// Also used after an app is closed, so the memory it freed leaves the working set.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn trim_working_set() {
    // SAFETY: both calls take no pointers; GetCurrentProcess returns a pseudo-handle that needs no
    // closing, and EmptyWorkingSet only asks the memory manager to page out this process.
    let result = unsafe { windows::Win32::System::ProcessStatus::EmptyWorkingSet(windows::Win32::System::Threading::GetCurrentProcess()) };
    if let Err(e) = result {
        log::debug!("EmptyWorkingSet: {e}");
    } else {
        log::debug!("working set trimmed");
    }
}

#[cfg(not(windows))]
pub fn trim_working_set() {}

/// How long Windows took between creating this process and running `main` (loading the program
/// from disk and any antivirus scan of it), in milliseconds.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn since_process_start_ms() -> Option<u64> {
    use windows::Win32::Foundation::FILETIME;
    let mut created = FILETIME::default();
    let (mut exit, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: GetCurrentProcess returns a pseudo-handle; the four pointers are to live locals.
    unsafe { windows::Win32::System::Threading::GetProcessTimes(windows::Win32::System::Threading::GetCurrentProcess(), &mut created, &mut exit, &mut kernel, &mut user) }.ok()?;
    let created = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    // FILETIME counts 100 ns ticks since 1601-01-01.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?;
    let now = (now.as_secs() + 11_644_473_600) * 10_000_000 + u64::from(now.subsec_nanos() / 100);
    Some(now.checked_sub(created)? / 10_000)
}

#[cfg(not(windows))]
pub fn since_process_start_ms() -> Option<u64> {
    None
}
