use ash::{vk::{self, Handle}};
use openxr as xr;

#[cfg_attr(target_os = "android", ndk_glue::main)]
fn main() {
    let entry = xr::Entry::load()
        .expect("Couldn't find the OpenXR loader; try enabling the \"static\" feature");

    #[cfg(target_os = "android")]
    entry.initialize_android_loader().unwrap();

    let extensions = entry.enumerate_extensions().unwrap();
    println!("Extensions: {:#?}", extensions);

    assert!(extensions.khr_vulkan_enable || extensions.khr_vulkan_enable2);

    let mut enabled_extensions = xr::ExtensionSet::default();
    if extensions.khr_vulkan_enable2 {
        enabled_extensions.khr_vulkan_enable2 = true;
    } else {
        enabled_extensions.khr_vulkan_enable = true;
    }
    #[cfg(target_os = "android")]
    {
        enabled_extensions.khr_android_create_instance = true;
    }

    let instance = entry
        .create_instance(
            &xr::ApplicationInfo {
                application_name: "openxr",
                ..Default::default()
            },
            &enabled_extensions,
            &[],
        ).unwrap();
    let instance_props = instance.properties().unwrap();
    println!(
        "Loaded OpenXR runtime: {} {}",
        instance_props.runtime_name, instance_props.runtime_version
    );

    let system = instance
        .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
        .unwrap();

    // TODO VK Version asserts
    let vk_target_version = vk::make_api_version(0, 1, 1, 0); // Vulkan 1.1 guarantees multiview support

    unsafe {
        let vk_entry = ash::Entry::load().unwrap();

        let vk_app_info = vk::ApplicationInfo::builder()
            .application_version(0)
            .engine_version(0)
            .api_version(vk_target_version);

        let vk_instance = {
            let vk_instance = instance
                .create_vulkan_instance(
                    system,
                    std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                    &vk::InstanceCreateInfo::builder().application_info(&vk_app_info) as *const _
                        as *const _,
                )
                .expect("OpenXR error creating Vulkan instance")
                .map_err(vk::Result::from_raw)
                .expect("Vulkan error creating Vulkan instance");
            ash::Instance::load(
                vk_entry.static_fn(),
                vk::Instance::from_raw(vk_instance as _),
            )
        };

        let vk_physical_device = vk::PhysicalDevice::from_raw(
            instance
                .vulkan_graphics_device(system, vk_instance.handle().as_raw() as _)
                .unwrap() as _,
        );

        let queue_family_index = vk_instance
            .get_physical_device_queue_family_properties(vk_physical_device)
            .into_iter()
            .enumerate()
            .find_map(|(queue_family_index, info)| {
                if info.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    Some(queue_family_index as u32)
                } else {
                    None
                }
            })
            .unwrap();

        let vk_device = {
            let device_queue_create_info = [vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_family_index)
                .queue_priorities(&[1.0])
                .build()];

            let mut multiview_features = vk::PhysicalDeviceMultiviewFeatures {
                multiview: vk::TRUE,
                ..Default::default()
            };

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&device_queue_create_info)
                .push_next(&mut multiview_features);

            let vk_device = instance
                .create_vulkan_device(
                    system,
                    std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                    vk_physical_device.as_raw() as _,
                    &device_create_info as *const _ as *const _,
                )
                .expect("OpenXR error creating Vulkan device")
                .map_err(vk::Result::from_raw)
                .expect("Vulkan error creating Vulkan device");

            ash::Device::load(vk_instance.fp_v1_0(), vk::Device::from_raw(vk_device as _))
        };
    }
}
