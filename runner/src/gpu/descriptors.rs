use ash::vk;

use super::{context::Context, Destroy};

pub struct Descriptors {
    pub layout: vk::DescriptorSetLayout,
    pub pool: vk::DescriptorPool,
    pub sets: Vec<vk::DescriptorSet>,
}

impl Destroy<Context> for Descriptors {
    unsafe fn destroy_with(&mut self, ctx: &mut Context) {
        ctx.destroy_descriptor_pool(self.pool, None);
        ctx.destroy_descriptor_set_layout(self.layout, None);
    }
}
