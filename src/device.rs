use serde::Serialize;

/// What the Metal device probe observed. Every field is read from the live
/// device; nothing is inferred from the model name.
#[derive(Serialize, Debug, Clone)]
pub struct DeviceReport {
    pub metal_available: bool,
    pub device_name: Option<String>,
    pub unified_memory: Option<bool>,
    pub recommended_max_working_set_bytes: Option<u64>,
    pub max_buffer_length_bytes: Option<u64>,
    pub max_threads_per_threadgroup: Option<[u64; 3]>,
    /// Highest Apple GPU family the device reports supporting (apple5..apple9).
    pub apple_gpu_family: Option<String>,
    pub registry_id: Option<u64>,
    pub note: Option<String>,
}

#[cfg(target_os = "macos")]
pub fn probe() -> DeviceReport {
    use metal::{Device, MTLGPUFamily};

    let Some(dev) = Device::system_default() else {
        return DeviceReport {
            metal_available: false,
            device_name: None,
            unified_memory: None,
            recommended_max_working_set_bytes: None,
            max_buffer_length_bytes: None,
            max_threads_per_threadgroup: None,
            apple_gpu_family: None,
            registry_id: None,
            note: Some(
                "no system default Metal device — GPU proving is impossible on this host".into(),
            ),
        };
    };

    let families = [
        (MTLGPUFamily::Apple9, "apple9"),
        (MTLGPUFamily::Apple8, "apple8"),
        (MTLGPUFamily::Apple7, "apple7"),
        (MTLGPUFamily::Apple6, "apple6"),
        (MTLGPUFamily::Apple5, "apple5"),
    ];
    let family = families
        .iter()
        .find(|(f, _)| dev.supports_family(*f))
        .map(|(_, name)| (*name).to_string());

    let tg = dev.max_threads_per_threadgroup();

    DeviceReport {
        metal_available: true,
        device_name: Some(dev.name().to_string()),
        unified_memory: Some(dev.has_unified_memory()),
        recommended_max_working_set_bytes: Some(dev.recommended_max_working_set_size()),
        max_buffer_length_bytes: Some(dev.max_buffer_length()),
        max_threads_per_threadgroup: Some([tg.width, tg.height, tg.depth]),
        apple_gpu_family: family,
        registry_id: Some(dev.registry_id()),
        note: None,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn probe() -> DeviceReport {
    DeviceReport {
        metal_available: false,
        device_name: None,
        unified_memory: None,
        recommended_max_working_set_bytes: None,
        max_buffer_length_bytes: None,
        max_threads_per_threadgroup: None,
        apple_gpu_family: None,
        registry_id: None,
        note: Some("not macOS — Metal does not exist on this platform".into()),
    }
}
