mod descriptors;
mod pass;
mod pipeline;
mod swapchain;
mod sync_state;
mod uniforms;

use ash::vk;

use shared::{bytemuck, UniformObjects};

use crate::{
    gpu::{
        buffer::Buffer, command_builder::CommandBuilder, command_pool::CommandPool,
        context::Context, image::Image, sampled_image::SampledImage, Destroy,
    },
    model::Model,
};

use self::{
    descriptors::Descriptors, pass::Pass, pipeline::Pipeline, swapchain::Swapchain,
    sync_state::SyncState, uniforms::Uniforms,
};

pub struct Renderer {
    pass: Pass,
    pipeline: Pipeline,
    descriptors: Descriptors,

    // model
    model: Model,

    // drawing
    command_pools: Vec<CommandPool>,
    command_buffers: Vec<vk::CommandBuffer>,

    vertex_index_buffer: Buffer,
    texture: SampledImage,

    // state
    pub uniforms: Uniforms,
    state: SyncState,

    // Recreate on resize
    swapchain: Swapchain,
}

pub enum Error {
    NeedsRecreating,
}

impl Renderer {
    pub fn create(ctx: &mut Context) -> Self {
        let pass = Pass::create(ctx);
        let descriptors = Descriptors::create(ctx);
        let pipeline = Pipeline::create(ctx, *pass, descriptors.layout);

        let (command_pools, command_buffers) = Self::create_command_pools_and_buffers(ctx);

        let model = Model::demo_viking_room();

        let mut setup = CommandBuilder::new(ctx, ctx.device.queues.graphics());

        let vertex_index_buffer = Self::init_vertex_index_buffer(ctx, &mut setup, &model);
        let texture = Self::init_texture(ctx, &mut setup, &model);

        let uniforms = Uniforms::create(ctx);
        descriptors.bind_descriptors(ctx, &uniforms, &texture);

        let state = SyncState::create(ctx);

        let swapchain = Swapchain::create(ctx, &mut setup, &pass);

        setup.finish(ctx);

        Self {
            pass,
            pipeline,
            descriptors,

            model,

            command_pools,
            command_buffers,

            vertex_index_buffer,
            texture,

            uniforms,
            state,

            swapchain,
        }
    }

    pub fn create_command_pools_and_buffers(
        ctx: &Context,
    ) -> (Vec<CommandPool>, Vec<vk::CommandBuffer>) {
        let pool_infos = vec![
            vk::CommandPoolCreateInfo::builder().build();
            ctx.surface.config.image_count as usize
        ];
        let pools =
            CommandPool::create_multiple(ctx, ctx.queues.graphics().family_index, &pool_infos);

        let buffer_info = vk::CommandBufferAllocateInfo::builder();
        let buffers = pools
            .iter()
            .map(|pool| pool.allocate_command_buffer(ctx, &buffer_info))
            .collect::<Vec<_>>();

        (pools, buffers)
    }

    fn init_vertex_index_buffer(
        ctx: &mut Context,
        setup: &mut CommandBuilder,
        model: &Model,
    ) -> Buffer {
        let data_sources = &[
            bytemuck::cast_slice(&model.mesh.vertices),
            bytemuck::cast_slice(&model.mesh.indices),
        ];
        let create_info = vk::BufferCreateInfo::builder()
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER);

        Buffer::create_with_staged_data(
            ctx,
            setup,
            "Vertex+Index Buffer",
            *create_info,
            data_sources,
        )
    }

    fn init_texture(ctx: &mut Context, setup: &mut CommandBuilder, model: &Model) -> SampledImage {
        let image = Image::create_from_image(ctx, setup, "Texture", &model.texture);
        SampledImage::from_image(ctx, image)
    }

    pub fn render(&mut self, ctx: &Context, uniforms: &UniformObjects) -> Result<(), Error> {
        unsafe {
            ctx.wait_for_fences(self.state.in_flight_fence(), true, u64::MAX)
                .expect("Failed to wait for `in_flight` fence");
        }

        let (image_index, needs_recreating) = self
            .swapchain
            .acquire_next_image_and_signal(self.state.image_available_semaphore()[0]);
        let image_index = image_index as usize;

        self.command_pools[image_index].reset(ctx);

        self.uniforms.update(image_index, uniforms);

        let needs_recreating = needs_recreating || {
            unsafe {
                ctx.reset_fences(self.state.in_flight_fence())
                    .expect("Failed to reset `in_flight` fence");
            }

            self.draw(
                ctx,
                image_index,
                self.state.image_available_semaphore(),
                self.state.render_finished_semaphore(),
                self.state.in_flight_fence()[0],
            );

            self.swapchain
                .present_to_when(ctx, image_index, self.state.render_finished_semaphore())
        };

        self.state.advance();

        (!needs_recreating)
            .then_some(())
            .ok_or(Error::NeedsRecreating)
    }

    pub fn draw(
        &self,
        ctx: &Context,
        image_index: usize,
        wait_on: &[vk::Semaphore],
        signal_to: &[vk::Semaphore],
        fence: vk::Fence,
    ) {
        self.record_commands_for_frame(ctx, image_index);

        let submit_infos = [vk::SubmitInfo::builder()
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
            .wait_semaphores(wait_on)
            .command_buffers(&self.command_buffers[crate::util::solo_range(image_index)])
            .signal_semaphores(signal_to)
            .build()];

        unsafe {
            ctx.queue_submit(**ctx.queues.graphics(), &submit_infos, fence)
                .expect("Failed to submit commands through the `graphics` queue");
        }
    }

    fn record_commands_for_frame(&self, ctx: &Context, image_index: usize) {
        let command_buffer = self.command_buffers[image_index];

        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 0.0],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            },
        ];

        let pass_info_template = vk::RenderPassBeginInfo::builder()
            .render_pass(*self.pass)
            .render_area(
                vk::Rect2D::builder()
                    .extent(ctx.surface.config.extent)
                    .build(),
            )
            .clear_values(&clear_values)
            .build();

        let command_buffer_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            ctx.begin_command_buffer(command_buffer, &command_buffer_info)
                .expect("Failed to begin recording command buffer");
        }

        let mut pass_info = pass_info_template;
        pass_info.framebuffer = self.swapchain.framebuffers[image_index];

        let viewports = [vk::Viewport::builder()
            .width(ctx.surface.config.extent.width as f32)
            .height(ctx.surface.config.extent.height as f32)
            .max_depth(1.0)
            .build()];

        let scissors = [vk::Rect2D::builder()
            .extent(ctx.surface.config.extent)
            .build()];

        unsafe {
            ctx.cmd_begin_render_pass(command_buffer, &pass_info, vk::SubpassContents::INLINE);

            ctx.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                *self.pipeline,
            );

            ctx.cmd_set_viewport_with_count(command_buffer, &viewports);

            ctx.cmd_set_scissor_with_count(command_buffer, &scissors);

            let vertex_buffers = [*self.vertex_index_buffer];
            ctx.cmd_bind_vertex_buffers(command_buffer, 0, &vertex_buffers, &[0]);

            ctx.cmd_bind_index_buffer(
                command_buffer,
                *self.vertex_index_buffer,
                self.model.mesh.vertex_data_size() as u64,
                vk::IndexType::UINT32,
            );

            let descriptor_sets = [self.descriptors.sets[image_index]];
            ctx.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline.layout,
                0,
                &descriptor_sets,
                &[],
            );

            ctx.cmd_draw_indexed(
                command_buffer,
                self.model.mesh.indices.len() as u32,
                1,
                0,
                0,
                0,
            );

            ctx.cmd_end_render_pass(command_buffer);

            ctx.end_command_buffer(command_buffer)
                .expect("Failed to end recording command buffer");
        }
    }

    pub fn recreate(&mut self, ctx: &mut Context) {
        unsafe {
            self.swapchain.destroy_with(ctx);
        }
        let mut setup = CommandBuilder::new(ctx, ctx.device.queues.graphics());
        self.swapchain = Swapchain::create(ctx, &mut setup, &self.pass);
        setup.finish(ctx);
    }
}

impl Destroy<Context> for Renderer {
    unsafe fn destroy_with(&mut self, ctx: &mut Context) {
        self.swapchain.destroy_with(ctx);

        self.state.destroy_with(ctx);
        self.uniforms.destroy_with(ctx);

        self.texture.destroy_with(ctx);
        self.vertex_index_buffer.destroy_with(ctx);

        for command_pool in &mut self.command_pools {
            command_pool.destroy_with(ctx);
        }

        self.descriptors.destroy_with(ctx);
        self.pipeline.destroy_with(ctx);
        self.pass.destroy_with(ctx);
    }
}
