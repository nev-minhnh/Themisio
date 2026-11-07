//! How wgpu finds a GPU and presents: PdfCraft's choices (its `apps/pdfcraft/src/main.rs`,
//! `configure_gpu` and helpers, MIT OR Apache-2.0), shared by all four apps in Themisio's window,
//! plus Themisio's presentation setting.
//!
//! - Draw on the GPU that drives the display (PdfCraft #378, #8, #445), the integrated one by
//!   preference on laptops (battery, and no discrete-GPU corruption after sleep).
//! - Windows: Direct3D 12 (OpenGL as fallback), never loading every Vulkan driver (PdfCraft #37);
//!   DirectComposition presentation so live resizing doesn't shimmer (PdfCraft #2738).
//! - The surface may be as large as the adapter allows (big monitors at 250 %).

use crate::settings::Presentation;

pub fn configure(native: &mut eframe::NativeOptions, presentation: Presentation) {
    native.wgpu_options.surface.present_mode = match presentation {
        Presentation::Balanced => eframe::wgpu::PresentMode::AutoVsync,
        Presentation::LowLatency => eframe::wgpu::PresentMode::AutoNoVsync,
    };
    // One frame queued at most: input shows on screen a frame sooner.
    native.wgpu_options.surface.desired_maximum_frame_latency = Some(match presentation {
        Presentation::Balanced => 2,
        Presentation::LowLatency => 1,
    });
    configure_gpu(native);
}
fn configure_gpu(native: &mut eframe::NativeOptions) {
    let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut native.wgpu_options.wgpu_setup else { return };
    // Nothing draws or dispatches indirectly, so wgpu's check of indirect arguments only costs a
    // compute shader when the device is created, one that Metal can't compile on an Intel Iris
    // Pro: the device is lost and the window falls back to OpenGL. `WGPU_VALIDATION_INDIRECT_CALL=1`
    // turns the check back on.
    setup.instance_descriptor.flags = setup.instance_descriptor.flags.difference(eframe::wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL).with_env();
    // egui-wgpu requests only 8192 pixels even when the adapter can render larger surfaces.
    // A restored 3440-point window at 250% DPI requests 8600 pixels and otherwise panics in
    // Surface::configure before the app starts. Keep the renderer's other compatible requirements,
    // but enable the adapter's actual 2D texture extent (also for smaller/downlevel adapters).
    let device_descriptor = std::sync::Arc::clone(&setup.device_descriptor);
    setup.device_descriptor = std::sync::Arc::new(move |adapter| {
        let mut descriptor = device_descriptor(adapter);
        descriptor.required_limits = surface_texture_limits(descriptor.required_limits, &adapter.limits());
        // Themisio: small GPU memory blocks. wgpu's default ("Performance") reserves 128-256 MB
        // device blocks and 64-128 MB host blocks at the first allocation; on an integrated GPU
        // that is ordinary RAM, and it was most of Themisio 0.1's memory. Egui's textures are small,
        // so 8-64 MB blocks cost nothing in speed.
        descriptor.memory_hints = eframe::wgpu::MemoryHints::MemoryUsage;
        descriptor
    });
    if std::env::var_os("WGPU_POWER_PREF").is_none() {
        setup.power_preference = eframe::wgpu::PowerPreference::LowPower;
        let displays = display_gpus();
        let compositor: Option<(u32, u32)> = None; // Wayland compositor GPU: not asked in Themisio.
        if !displays.is_empty() || compositor.is_some() {
            setup.native_adapter_selector = Some(std::sync::Arc::new(move |adapters, surface| {
                let usable: Vec<&eframe::wgpu::Adapter> = adapters.iter().filter(|a| surface.is_none_or(|s| a.is_surface_supported(s))).collect();
                let infos: Vec<(u32, u32, eframe::wgpu::DeviceType)> = usable
                    .iter()
                    .map(|a| {
                        let info = a.get_info();
                        (info.vendor, info.device, info.device_type)
                    })
                    .collect();
                let picked = pick_adapter(&infos, &displays, compositor).and_then(|i| usable.get(i)).map(|a| (*a).clone());
                match &picked {
                    // Which GPU draws is the first question when a window stays black (#8, #378, #445).
                    Some(a) => {
                        let info = a.get_info();
                        log::info!("drawing on {} ({:?}, {:?})", info.name, info.device_type, info.backend);
                    }
                    None => log::warn!("no GPU can draw to this window; {} were considered", usable.len()),
                }
                picked.ok_or_else(|| "no GPU can draw to this window".to_string())
            }));
        }
    }
    if cfg!(target_os = "windows") && std::env::var_os("WGPU_BACKEND").is_none() {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12 | eframe::wgpu::Backends::GL;
    }
    if cfg!(target_os = "windows") {
        setup.instance_descriptor.backend_options.dx12.presentation_system = dx12_presentation(eframe::wgpu::Dx12SwapchainKind::from_env());
    }
}

/// How Direct3D 12 presents on Windows: through a DirectComposition visual (issue #2738), unless
/// `WGPU_DX12_PRESENTATION_SYSTEM` asked for something else (`asked`).
fn dx12_presentation(asked: Option<eframe::wgpu::Dx12SwapchainKind>) -> eframe::wgpu::Dx12SwapchainKind {
    asked.unwrap_or(eframe::wgpu::Dx12SwapchainKind::DxgiFromVisual)
}

/// Surface textures must fit the device's enabled limits, not just the physical GPU's limits.
/// Clamp egui-wgpu's backend-specific requirements before enabling the adapter's full 2D extent.
fn surface_texture_limits(mut required: eframe::wgpu::Limits, supported: &eframe::wgpu::Limits) -> eframe::wgpu::Limits {
    required = required.or_worse_values_from(supported);
    required.max_texture_dimension_2d = supported.max_texture_dimension_2d;
    required
}

/// A GPU with a connected monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DisplayGpu {
    /// PCI `(vendor, device)` ids.
    pci: (u32, u32),
    /// Whether it drives the display the user most likely looks at: on Linux a built-in panel
    /// (`eDP`, `LVDS` or `DSI`), on Windows the primary display.
    primary: bool,
}

/// The GPUs with a connected monitor, as far as the system says: Linux reads the DRM connectors
/// in sysfs, Windows asks for the display devices attached to the desktop. Empty elsewhere.
fn display_gpus() -> Vec<DisplayGpu> {
    #[cfg(target_os = "linux")]
    {
        linux_display_gpus(std::path::Path::new("/sys/class/drm"))
    }
    #[cfg(target_os = "windows")]
    {
        windows_display_gpus()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// Records one display of GPU `pci`; a GPU driving several displays is one entry.
#[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
fn add_display_gpu(gpus: &mut Vec<DisplayGpu>, pci: (u32, u32), primary: bool) {
    match gpus.iter_mut().find(|g| g.pci == pci) {
        Some(gpu) => gpu.primary |= primary,
        None => gpus.push(DisplayGpu { pci, primary }),
    }
}

/// The GPUs driving a display attached to the desktop, from `EnumDisplayDevices`. Each entry it
/// lists is one display device of an adapter (`\\.\DISPLAY1`, "NVIDIA GeForce RTX 3090"), with the
/// adapter's PCI ids in its device id (`PCI\VEN_10DE&DEV_2204&SUBSYS_40421458&REV_A1`); a GPU with
/// no monitor lists its devices as not attached. Issue #378: a Ryzen desktop with the monitors on
/// an NVIDIA card, where the integrated GPU would otherwise be chosen.
#[cfg(target_os = "windows")]
fn windows_display_gpus() -> Vec<DisplayGpu> {
    use winsafe::co::DISPLAY_DEVICE as Flags;
    let mut gpus: Vec<DisplayGpu> = Vec::new();
    // A machine has a handful of display devices; the cap only bounds a runaway enumeration.
    for device in winsafe::EnumDisplayDevices(None, None).take(256) {
        let device = match device {
            Ok(d) => d,
            // The enumeration ends with "no more items" or, from a driver, with any error.
            Err(e) => {
                log::debug!("display devices: {e}");
                break;
            }
        };
        if !device.StateFlags.has(Flags::ATTACHED_TO_DESKTOP) {
            continue;
        }
        let Some(pci) = pci_ids(&device.DeviceID()) else { continue };
        add_display_gpu(&mut gpus, pci, device.StateFlags.has(Flags::PRIMARY_DEVICE));
    }
    gpus
}

/// The PCI `(vendor, device)` ids in a Windows device id such as
/// `PCI\VEN_10DE&DEV_2204&SUBSYS_40421458&REV_A1`; `None` for any other kind of device.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn pci_ids(device_id: &str) -> Option<(u32, u32)> {
    let field = |key: &str| {
        device_id.split(['\\', '&']).find_map(|part| {
            let (name, hex) = part.split_at_checked(key.len())?;
            if name.eq_ignore_ascii_case(key) { u32::from_str_radix(hex, 16).ok() } else { None }
        })
    };
    Some((field("VEN_")?, field("DEV_")?))
}

/// Whether a DRM connector name (`eDP-1`, `LVDS-1`, `DSI-1`) is a laptop's built-in panel.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn is_internal_panel(connector: &str) -> bool {
    ["eDP", "LVDS", "DSI"].iter().any(|kind| connector.starts_with(kind))
}

/// The GPUs with a connected monitor, read from the DRM connectors under `drm` (`card1-DP-3/status`
/// is `connected`, `card1/device/{vendor,device}` hold `0x10de`).
#[cfg(target_os = "linux")]
fn linux_display_gpus(drm: &std::path::Path) -> Vec<DisplayGpu> {
    let read_hex = |p: std::path::PathBuf| -> Option<u32> {
        let s = std::fs::read_to_string(p).ok()?;
        u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
    };
    let mut gpus: Vec<DisplayGpu> = Vec::new();
    let Ok(entries) = std::fs::read_dir(drm) else { return gpus };
    // A machine has a handful of connectors; the cap only bounds a pathological sysfs.
    for entry in entries.flatten().take(256) {
        let name = entry.file_name();
        let Some((card, connector)) = name.to_str().and_then(|n| n.split_once('-')) else { continue };
        let connected = std::fs::read_to_string(entry.path().join("status")).is_ok_and(|s| s.trim() == "connected");
        if !connected {
            continue;
        }
        let device = drm.join(card).join("device");
        let (Some(v), Some(d)) = (read_hex(device.join("vendor")), read_hex(device.join("device"))) else { continue };
        add_display_gpu(&mut gpus, (v, d), is_internal_panel(connector));
    }
    gpus
}

/// Index of the adapter to draw with: the GPU the Wayland compositor draws with (`compositor`, by
/// PCI ids) first, then one that drives a display. Among several of those, the one driving the
/// primary display (a hybrid laptop's panel, #8; the primary display on Windows, #378), or else,
/// with no primary display known (a Linux desktop with a monitor on each GPU, #445), the discrete
/// one. Otherwise the most frugal kind: integrated, discrete, other, virtual, software.
fn pick_adapter(adapters: &[(u32, u32, eframe::wgpu::DeviceType)], displays: &[DisplayGpu], compositor: Option<(u32, u32)>) -> Option<usize> {
    use eframe::wgpu::DeviceType;
    let display = |pci: (u32, u32)| displays.iter().find(|g| g.pci == pci);
    let multi_gpu_desktop = displays.len() > 1 && !displays.iter().any(|g| g.primary);
    adapters
        .iter()
        .enumerate()
        .min_by_key(|(_, (vendor, device, kind))| {
            let shown = display((*vendor, *device));
            let drives_display = shown.is_some();
            let drives_primary = shown.is_some_and(|g| g.primary);
            let rank = match kind {
                // On a desktop with monitors on both, the integrated GPU can accept the window and
                // still fail to present to it (#445).
                DeviceType::DiscreteGpu if drives_display && multi_gpu_desktop => 0,
                DeviceType::IntegratedGpu => 1,
                DeviceType::DiscreteGpu => 2,
                DeviceType::Other => 3,
                DeviceType::VirtualGpu => 4,
                DeviceType::Cpu => 5,
            };
            (compositor != Some((*vendor, *device)), !drives_display, !drives_primary, rank)
        })
        .map(|(i, _)| i)
}

