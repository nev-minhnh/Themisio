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
