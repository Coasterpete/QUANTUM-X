//! R0.4 experiment only: explicit WARP selection without changing normal policy.
use std::ffi::OsStr;

fn warp_requested(value: Option<&OsStr>) -> Result<bool, String> {
    match value {
        None => Ok(false),
        Some(value) if value == "warp" => Ok(true),
        Some(_) => Err(
            "QX_SOFTWARE_GPU must be unset or exactly 'warp' (Windows DX12 software adapter)"
                .into(),
        ),
    }
}

fn requested() -> Result<bool, String> {
    warp_requested(std::env::var_os("QX_SOFTWARE_GPU").as_deref())
}

fn descriptor(warp: bool) -> wgpu::InstanceDescriptor {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if warp {
        descriptor.backends = wgpu::Backends::DX12;
    }
    descriptor
}

pub(crate) fn instance() -> Result<wgpu::Instance, String> {
    Ok(wgpu::Instance::new(descriptor(requested()?)))
}

fn is_warp(name: &str, backend: wgpu::Backend, device_type: wgpu::DeviceType) -> bool {
    backend == wgpu::Backend::Dx12
        && device_type == wgpu::DeviceType::Cpu
        && name.contains("Microsoft Basic Render Driver")
}

pub(crate) async fn request_adapter(
    instance: &wgpu::Instance,
    mut options: wgpu::RequestAdapterOptions<'_, '_>,
) -> Result<wgpu::Adapter, String> {
    let warp = requested()?;
    if warp {
        options.force_fallback_adapter = true;
    }
    let adapter = match instance.request_adapter(&options).await {
        Ok(adapter) => adapter,
        Err(error) => {
            if warp {
                // Preserve useful diagnostics even when no surface-compatible
                // software adapter exists. No device is created by enumeration.
                let candidates: Vec<_> = instance
                    .enumerate_adapters(wgpu::Backends::DX12)
                    .await
                    .iter()
                    .map(|a| format!("{:?}", a.get_info()))
                    .collect();
                eprintln!(
                    "QX_SOFTWARE_GPU_FAILURE {}",
                    serde_json::json!({
                        "requested": "warp", "compatible_surface": options.compatible_surface.is_some(),
                        "error": error.to_string(), "candidates": candidates,
                    })
                );
            }
            return Err(error.to_string());
        }
    };
    if warp {
        let info = adapter.get_info();
        let accepted = is_warp(&info.name, info.backend, info.device_type);
        // stderr keeps existing JSON stdout reports machine-readable. Emit
        // before device creation so later initialization failures retain identity.
        eprintln!(
            "QX_SOFTWARE_GPU_ADAPTER {}",
            serde_json::json!({
                "requested": "warp", "accepted": accepted, "name": info.name,
                "backend": format!("{:?}", info.backend),
                "device_type": format!("{:?}", info.device_type),
                "driver": info.driver, "driver_info": info.driver_info,
            })
        );
        if !accepted {
            return Err(format!(
                "QX_SOFTWARE_GPU=warp selected an unexpected adapter: {info:?}"
            ));
        }
    }
    Ok(adapter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_is_explicit_and_rejects_typos() {
        assert!(!warp_requested(None).unwrap());
        assert!(warp_requested(Some(OsStr::new("warp"))).unwrap());
        for value in ["", "WARP", "auto", "hardware", "mesa"] {
            assert!(warp_requested(Some(OsStr::new(value))).is_err());
        }
    }

    #[test]
    fn normal_policy_and_warp_backend_are_distinct() {
        assert_eq!(descriptor(false).backends, wgpu::Backends::default());
        assert_eq!(descriptor(true).backends, wgpu::Backends::DX12);
    }

    #[test]
    fn software_identity_requires_all_three_properties() {
        use wgpu::{Backend, DeviceType};
        let name = "Microsoft Basic Render Driver";
        assert!(is_warp(name, Backend::Dx12, DeviceType::Cpu));
        assert!(!is_warp(name, Backend::Vulkan, DeviceType::Cpu));
        assert!(!is_warp(name, Backend::Dx12, DeviceType::IntegratedGpu));
        assert!(!is_warp(
            "Other CPU adapter",
            Backend::Dx12,
            DeviceType::Cpu
        ));
    }
}
