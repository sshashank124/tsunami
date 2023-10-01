use ash::vk;

use crate::{
    device::Device,
    instance::Instance,
    surface::{Surface, SurfaceConfig},
    swapchain::Swapchain,
    util::{self, info, Destroy},
};

pub struct RenderPipeline {
    pub render_pass: vk::RenderPass,
    pub swapchain: Swapchain,
    pub pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
    command_pool: vk::CommandPool,
    pub command_buffers: Vec<vk::CommandBuffer>,
    state: SyncState,
}

struct SyncState {
    image_available: Vec<vk::Semaphore>,
    render_finished: Vec<vk::Semaphore>,
    in_flight: Vec<vk::Fence>,
    current_frame: usize,
}

impl RenderPipeline {
    pub fn create(device: &Device, surface: &mut Surface, instance: &Instance) -> Self {
        let render_pass = Self::create_render_pass(device, surface.config.surface_format.format);
        let swapchain = Swapchain::create(device, surface, render_pass, instance);
        let (pipeline, layout) = Self::create_pipeline(device, surface.config.extent, render_pass);
        let command_pool = device.create_command_pool();
        let command_buffers = Self::create_command_buffers(
            device,
            &surface.config,
            render_pass,
            &swapchain,
            pipeline,
            command_pool,
        );
        let state = SyncState::create(device);

        Self {
            render_pass,
            swapchain,
            pipeline,
            layout,
            command_pool,
            command_buffers,
            state,
        }
    }

    fn create_pipeline(
        device: &Device,
        surface_extent: vk::Extent2D,
        render_pass: vk::RenderPass,
    ) -> (vk::Pipeline, vk::PipelineLayout) {
        let shader_module = util::create_shader_module_from_file(device, info::SHADER_FILE);

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(shader_module)
                .name(info::VERTEX_SHADER_ENTRY_POINT)
                .build(),
            vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(shader_module)
                .name(info::FRAGMENT_SHADER_ENTRY_POINT)
                .build(),
        ];

        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder();

        let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let viewports = [vk::Viewport::builder()
            .width(surface_extent.width as f32)
            .height(surface_extent.height as f32)
            .max_depth(1.0)
            .build()];

        let scissors = [vk::Rect2D::builder().extent(surface_extent).build()];

        let viewport_info = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&viewports)
            .scissors(&scissors);

        let rasterization_info = vk::PipelineRasterizationStateCreateInfo::builder()
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK);

        let multisample_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let color_blend_attachments = [vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .build()];

        let color_blend_info =
            vk::PipelineColorBlendStateCreateInfo::builder().attachments(&color_blend_attachments);

        let layout_create_info = vk::PipelineLayoutCreateInfo::builder();

        let layout = unsafe {
            device
                .create_pipeline_layout(&layout_create_info, None)
                .expect("Failed to create graphics pipeline layout")
        };

        let create_infos = [vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly_info)
            .viewport_state(&viewport_info)
            .rasterization_state(&rasterization_info)
            .multisample_state(&multisample_info)
            .color_blend_state(&color_blend_info)
            .layout(layout)
            .render_pass(render_pass)
            .build()];

        let pipeline = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &create_infos, None)
                .expect("Failed to create graphics pipeline")
        }[0];

        unsafe { device.destroy_shader_module(shader_module, None) };

        (pipeline, layout)
    }

    pub fn create_render_pass(device: &Device, format: vk::Format) -> vk::RenderPass {
        let color_attachments = [vk::AttachmentDescription::builder()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build()];

        let color_attachment_references = [vk::AttachmentReference::builder()
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()];

        let subpasses = [vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_references)
            .build()];

        let dependencies = [vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .build()];

        let create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&color_attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        unsafe {
            device
                .create_render_pass(&create_info, None)
                .expect("Failed to create render pass")
        }
    }

    fn create_command_buffers(
        device: &Device,
        surface_config: &SurfaceConfig,
        render_pass: vk::RenderPass,
        swapchain: &Swapchain,
        pipeline: vk::Pipeline,
        command_pool: vk::CommandPool,
    ) -> Vec<vk::CommandBuffer> {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .command_buffer_count(surface_config.image_count);

        let command_buffers = unsafe {
            device
                .allocate_command_buffers(&allocate_info)
                .expect("Failed to allocate command buffers")
        };

        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        }];

        let render_pass_info_template = vk::RenderPassBeginInfo::builder()
            .render_pass(render_pass)
            .render_area(vk::Rect2D::builder().extent(surface_config.extent).build())
            .clear_values(&clear_values)
            .build();

        for (&framebuffer, &command_buffer) in swapchain.framebuffers.iter().zip(&command_buffers) {
            let command_buffer_info = vk::CommandBufferBeginInfo::builder();

            unsafe {
                device
                    .begin_command_buffer(command_buffer, &command_buffer_info)
                    .expect("Failed to begin recording command buffer");
            }

            let mut render_pass_info = render_pass_info_template;
            render_pass_info.framebuffer = framebuffer;

            unsafe {
                device.cmd_begin_render_pass(
                    command_buffer,
                    &render_pass_info,
                    vk::SubpassContents::INLINE,
                );
                device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline);
                device.cmd_draw(command_buffer, 3, 1, 0, 0);
                device.cmd_end_render_pass(command_buffer);
                device
                    .end_command_buffer(command_buffer)
                    .expect("Failed to end recording command buffer");
            }
        }

        command_buffers
    }

    pub fn render(&mut self, device: &Device) {
        unsafe {
            device
                .wait_for_fences(self.state.in_flight_fence(), true, u64::MAX)
                .expect("Failed to wait for `in_flight` fence");

            let image_index = self
                .swapchain
                .acquire_next_image(self.state.image_available_semaphore()[0]);

            device
                .reset_fences(self.state.in_flight_fence())
                .expect("Failed to reset `in_flight` fence");

            self.render_to(device, image_index);

            self.swapchain.present_to_when(
                device,
                image_index,
                self.state.render_finished_semaphore(),
            );
        }

        self.state.advance();
    }

    unsafe fn render_to(&self, device: &Device, image_index: u32) {
        let submit_infos = [vk::SubmitInfo::builder()
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
            .wait_semaphores(self.state.image_available_semaphore())
            .command_buffers(&self.command_buffers[util::solo_range(image_index as usize)])
            .signal_semaphores(self.state.render_finished_semaphore())
            .build()];

        device
            .queue_submit(
                device.queue.graphics,
                &submit_infos,
                self.state.in_flight_fence()[0],
            )
            .expect("Failed to submit through the `graphics` queue");
    }
}

impl<'a> Destroy<&'a Device> for RenderPipeline {
    unsafe fn destroy_with(&self, device: &'a Device) {
        self.state.destroy_with(device);
        device.destroy_command_pool(self.command_pool, None);
        device.destroy_pipeline(self.pipeline, None);
        device.destroy_pipeline_layout(self.layout, None);
        self.swapchain.destroy_with(device);
        device.destroy_render_pass(self.render_pass, None);
    }
}

impl SyncState {
    fn create(device: &Device) -> Self {
        let mut image_available = Vec::with_capacity(info::MAX_FRAMES_IN_FLIGHT);
        let mut render_finished = Vec::with_capacity(info::MAX_FRAMES_IN_FLIGHT);
        let mut in_flight = Vec::with_capacity(info::MAX_FRAMES_IN_FLIGHT);

        for _ in 0..info::MAX_FRAMES_IN_FLIGHT {
            image_available.push(device.create_semaphore("image_available"));
            render_finished.push(device.create_semaphore("render_finished"));
            in_flight.push(device.create_fence("in_flight", true));
        }

        Self {
            image_available,
            render_finished,
            in_flight,
            current_frame: 0,
        }
    }

    fn image_available_semaphore(&self) -> &[vk::Semaphore] {
        &self.image_available[util::solo_range(self.current_frame)]
    }

    fn render_finished_semaphore(&self) -> &[vk::Semaphore] {
        &self.render_finished[util::solo_range(self.current_frame)]
    }

    fn in_flight_fence(&self) -> &[vk::Fence] {
        &self.in_flight[util::solo_range(self.current_frame)]
    }

    fn advance(&mut self) {
        self.current_frame = (self.current_frame + 1) % info::MAX_FRAMES_IN_FLIGHT;
    }
}

impl<'a> Destroy<&'a Device> for SyncState {
    unsafe fn destroy_with(&self, device: &'a Device) {
        for i in 0..info::MAX_FRAMES_IN_FLIGHT {
            device.destroy_semaphore(self.image_available[i], None);
            device.destroy_semaphore(self.render_finished[i], None);
            device.destroy_fence(self.in_flight[i], None);
        }
    }
}