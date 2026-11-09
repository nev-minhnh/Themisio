//! Shell motion: easing curves and whether to animate at all.

use crate::settings::Motion;

/// Fluent's "decelerate" feel (fast start, long soft landing), as an ease-out cubic.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Fluent's "accelerate" (for things leaving), as an ease-in quadratic.
pub fn ease_in(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

/// Whether the shell animates, from the setting and (for `System`) Windows' Animation effects.
pub struct MotionState {
    last_check: f64,
    system_reduced: bool,
}

impl Default for MotionState {
    fn default() -> Self {
        Self { last_check: f64::NEG_INFINITY, system_reduced: system_reduced() }
    }
}

impl MotionState {
    pub fn animate(&mut self, setting: Motion, now: f64) -> bool {
        match setting {
            Motion::On => true,
            Motion::Off => false,
            Motion::System => {
                // The setting can change while Themisio runs; asking is cheap but not free.
                if now - self.last_check > 5.0 {
                    self.last_check = now;
                    self.system_reduced = system_reduced();
                }
                !self.system_reduced
            }
        }
    }
}

/// Settings › Accessibility › Visual effects › Animation effects is off.
#[cfg(windows)]
pub fn system_reduced() -> bool {
    windows::UI::ViewManagement::UISettings::new().and_then(|s| s.AnimationsEnabled()).map(|on| !on).unwrap_or(false)
}

#[cfg(not(windows))]
pub fn system_reduced() -> bool {
    false
}
