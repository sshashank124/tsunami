mod device;
mod features;
mod instance;
mod queue;
mod surface;
mod validator;

use std::ops::{Deref, DerefMut};

use ash::vk;
pub use gpu_allocator::vulkan as gpu_alloc;

use winit::window::Window;

use crate::util::Destroy;

use self::{device::Device, instance::Instance, surface::Surface};

pub struct Context {
    pub instance: Instance,
    physical_device: vk::PhysicalDevice,
    pub surface: Surface,
    pub device: Device,
}

impl Context {
    pub fn init(window: &Window) -> Self {
        let instance = Instance::new(&window.title());
        let surface_handle = instance.create_surface_on(window);

        let (physical_device, queue_families, surface_config) =
            instance.get_physical_device_and_info(&surface_handle);

        let device = Device::new(&instance, physical_device, &queue_families);

        let surface = Surface::new(surface_handle, surface_config);

        Self {
            instance,
            physical_device,
            surface,
            device,
        }
    }

    pub fn refresh_surface_capabilities(&mut self) -> bool {
        self.surface.refresh_capabilities(self.physical_device)
    }
}

impl Destroy<()> for Context {
    unsafe fn destroy_with(&mut self, _: ()) {
        self.device.destroy_with(());
        self.surface.destroy_with(());
        self.instance.destroy_with(());
    }
}

impl Deref for Context {
    type Target = Device;
    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl DerefMut for Context {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.device
    }
}
