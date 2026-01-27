use crate::colorease::ColorEaseUniform;
use crate::termwindow::render::pane_border::{BlurUniform, BorderUniform};
use crate::termwindow::webgpu::ShaderUniform;
use crate::termwindow::RenderFrame;
use crate::uniforms::UniformBuilder;
use ::window::glium;
use ::window::glium::uniforms::{
    MagnifySamplerFilter, MinifySamplerFilter, Sampler, SamplerWrapFunction,
};
use ::window::glium::{BlendingFunction, LinearBlendingFactor, Surface};
use config::FreeTypeLoadTarget;
use wgpu::util::DeviceExt;

impl crate::TermWindow {
    pub fn call_draw(&mut self, frame: &mut RenderFrame) -> anyhow::Result<()> {
        match frame {
            RenderFrame::Glium(ref mut frame) => self.call_draw_glium(frame),
            RenderFrame::WebGpu => self.call_draw_webgpu(),
        }
    }

    fn call_draw_webgpu(&mut self) -> anyhow::Result<()> {
        use crate::termwindow::webgpu::WebGpuTexture;

        let webgpu = self.webgpu.as_ref().unwrap();
        let render_state = self.render_state.as_ref().unwrap();

        let output = webgpu.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = webgpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        let tex = render_state.glyph_cache.borrow().atlas.texture();
        let tex = tex.downcast_ref::<WebGpuTexture>().unwrap();
        let texture_view = tex.create_view(&wgpu::TextureViewDescriptor::default());

        let texture_linear_bind_group =
            webgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &webgpu.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&webgpu.texture_linear_sampler),
                    },
                ],
                label: Some("linear bind group"),
            });

        let texture_nearest_bind_group =
            webgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &webgpu.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&webgpu.texture_nearest_sampler),
                    },
                ],
                label: Some("nearest bind group"),
            });

        let mut cleared = false;
        let foreground_text_hsb = self.config.foreground_text_hsb;
        let foreground_text_hsb = [
            foreground_text_hsb.hue,
            foreground_text_hsb.saturation,
            foreground_text_hsb.brightness,
        ];

        let milliseconds = self.created.elapsed().as_millis() as u32;
        let projection = euclid::Transform3D::<f32, f32, f32>::ortho(
            -(self.dimensions.pixel_width as f32) / 2.0,
            self.dimensions.pixel_width as f32 / 2.0,
            self.dimensions.pixel_height as f32 / 2.0,
            -(self.dimensions.pixel_height as f32) / 2.0,
            -1.0,
            1.0,
        )
        .to_arrays_transposed();

        for layer in render_state.layers.borrow().iter() {
            for idx in 0..3 {
                let vb = &layer.vb.borrow()[idx];
                let (vertex_count, index_count) = vb.vertex_index_count();
                let vertex_buffer;
                let uniforms;
                if vertex_count > 0 {
                    let mut vertices = vb.current_vb_mut();
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: if cleared {
                                    wgpu::LoadOp::Load
                                } else {
                                    wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.,
                                        g: 0.,
                                        b: 0.,
                                        a: 0.,
                                    })
                                },
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    cleared = true;

                    uniforms = webgpu.create_uniform(ShaderUniform {
                        foreground_text_hsb,
                        milliseconds,
                        projection,
                    });

                    render_pass.set_pipeline(&webgpu.render_pipeline);
                    render_pass.set_bind_group(0, &uniforms, &[]);
                    render_pass.set_bind_group(1, &texture_linear_bind_group, &[]);
                    render_pass.set_bind_group(2, &texture_nearest_bind_group, &[]);
                    vertex_buffer = vertices.webgpu_mut().recreate();
                    vertex_buffer.unmap();
                    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    render_pass
                        .set_index_buffer(vb.indices.webgpu().slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..index_count as _, 0, 0..1);
                }

                vb.next_index();
            }
        }

        // Render glow effect for pane borders
        self.render_border_glow(&mut encoder, &view)?;

        // submit will accept anything that implements IntoIter
        let webgpu = self.webgpu.as_ref().unwrap();
        webgpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Renders the glow effect for pane borders using two-pass Gaussian blur.
    ///
    /// Flow:
    /// 1. Render borders to `border_texture`
    /// 2. Apply horizontal blur: `border_texture` → `blur_temp`
    /// 3. Apply vertical blur: `blur_temp` → screen
    fn render_border_glow(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        screen_view: &wgpu::TextureView,
    ) -> anyhow::Result<()> {
        let webgpu = self.webgpu.as_ref().unwrap();
        let glow_textures = webgpu.glow_textures.borrow();

        // Collect border data for all panes
        let panes = self.get_panes_to_render();
        let border_data = self.collect_pane_border_data(&panes);
        let vertices = self.generate_border_vertices(&border_data);

        if vertices.is_empty() {
            return Ok(());
        }

        // Create vertex buffer for border rendering
        let border_vertex_buffer = webgpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Border Glow Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create projection matrix for border rendering
        let projection = euclid::Transform3D::<f32, f32, f32>::ortho(
            -(self.dimensions.pixel_width as f32) / 2.0,
            self.dimensions.pixel_width as f32 / 2.0,
            self.dimensions.pixel_height as f32 / 2.0,
            -(self.dimensions.pixel_height as f32) / 2.0,
            -1.0,
            1.0,
        )
        .to_arrays_transposed();

        // Calculate animation time in seconds for border animations
        let animation_time = self.created.elapsed().as_secs_f32();

        // Read animation settings from config
        let animation_config = &config::configuration().claude_terminal.animation;
        let pulse_idle_enabled = if animation_config.pulse_idle { 1.0 } else { 0.0 };
        let blink_permission_enabled = if animation_config.blink_permission { 1.0 } else { 0.0 };
        let blink_rate_hz = animation_config.blink_rate_hz;
        let pulse_period_secs = animation_config.pulse_period_secs;

        let border_uniform_bind_group = webgpu.border_pipeline.create_uniform_bind_group(
            &webgpu.device,
            BorderUniform {
                projection,
                animation_time,
                pulse_idle_enabled,
                blink_permission_enabled,
                blink_rate_hz,
                pulse_period_secs,
                _padding: [0.0, 0.0, 0.0],
            },
        );

        // Pass 1: Render borders to border_texture
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Border Glow Pass 1 - Render Borders"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &glow_textures.border_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&webgpu.border_pipeline.pipeline);
            render_pass.set_bind_group(0, &border_uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, border_vertex_buffer.slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);
        }

        // Create blur uniform with values from config
        let border_config = &config::configuration().claude_terminal.border;
        let blur_uniform = BlurUniform {
            tex_size: [glow_textures.width as f32, glow_textures.height as f32],
            blur_scale: border_config.glow_radius as f32,
            glow_opacity: border_config.glow_opacity,
        };
        let blur_uniform_bind_group = webgpu.blur_pipeline.create_uniform_bind_group(
            &webgpu.device,
            blur_uniform,
        );

        // Create texture bind groups for blur passes
        let border_texture_bind_group = webgpu.blur_pipeline.create_texture_bind_group(
            &webgpu.device,
            &glow_textures.border_view,
        );
        let blur_temp_texture_bind_group = webgpu.blur_pipeline.create_texture_bind_group(
            &webgpu.device,
            &glow_textures.blur_temp_view,
        );

        // Pass 2: Horizontal blur (border_texture → blur_temp)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Border Glow Pass 2 - Horizontal Blur"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &glow_textures.blur_temp_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&webgpu.blur_pipeline.h_pipeline);
            render_pass.set_bind_group(0, &blur_uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &border_texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, webgpu.blur_pipeline.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }

        // Pass 3: Vertical blur (blur_temp → screen, with alpha blending)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Border Glow Pass 3 - Vertical Blur"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: screen_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Preserve existing content
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&webgpu.blur_pipeline.v_pipeline);
            render_pass.set_bind_group(0, &blur_uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &blur_temp_texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, webgpu.blur_pipeline.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }

        Ok(())
    }

    fn call_draw_glium(&mut self, frame: &mut glium::Frame) -> anyhow::Result<()> {
        use window::glium::texture::SrgbTexture2d;

        let gl_state = self.render_state.as_ref().unwrap();
        let tex = gl_state.glyph_cache.borrow().atlas.texture();
        let tex = tex.downcast_ref::<SrgbTexture2d>().unwrap();

        frame.clear_color(0., 0., 0., 0.);

        let projection = euclid::Transform3D::<f32, f32, f32>::ortho(
            -(self.dimensions.pixel_width as f32) / 2.0,
            self.dimensions.pixel_width as f32 / 2.0,
            self.dimensions.pixel_height as f32 / 2.0,
            -(self.dimensions.pixel_height as f32) / 2.0,
            -1.0,
            1.0,
        )
        .to_arrays_transposed();

        let use_subpixel = match self
            .config
            .freetype_render_target
            .unwrap_or(self.config.freetype_load_target)
        {
            FreeTypeLoadTarget::HorizontalLcd | FreeTypeLoadTarget::VerticalLcd => true,
            _ => false,
        };

        let dual_source_blending = glium::DrawParameters {
            blend: glium::Blend {
                color: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceOneColor,
                    destination: LinearBlendingFactor::OneMinusSourceOneColor,
                },
                alpha: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceOneColor,
                    destination: LinearBlendingFactor::OneMinusSourceOneColor,
                },
                constant_value: (0.0, 0.0, 0.0, 0.0),
            },

            ..Default::default()
        };

        let alpha_blending = glium::DrawParameters {
            blend: glium::Blend {
                color: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceAlpha,
                    destination: LinearBlendingFactor::OneMinusSourceAlpha,
                },
                alpha: BlendingFunction::Addition {
                    source: LinearBlendingFactor::One,
                    destination: LinearBlendingFactor::OneMinusSourceAlpha,
                },
                constant_value: (0.0, 0.0, 0.0, 0.0),
            },
            ..Default::default()
        };

        // Clamp and use the nearest texel rather than interpolate.
        // This prevents things like the box cursor outlines from
        // being randomly doubled in width or height
        let atlas_nearest_sampler = Sampler::new(&*tex)
            .wrap_function(SamplerWrapFunction::Clamp)
            .magnify_filter(MagnifySamplerFilter::Nearest)
            .minify_filter(MinifySamplerFilter::Nearest);

        let atlas_linear_sampler = Sampler::new(&*tex)
            .wrap_function(SamplerWrapFunction::Clamp)
            .magnify_filter(MagnifySamplerFilter::Linear)
            .minify_filter(MinifySamplerFilter::Linear);

        let foreground_text_hsb = self.config.foreground_text_hsb;
        let foreground_text_hsb = (
            foreground_text_hsb.hue,
            foreground_text_hsb.saturation,
            foreground_text_hsb.brightness,
        );

        let milliseconds = self.created.elapsed().as_millis() as u32;

        let cursor_blink: ColorEaseUniform = (*self.cursor_blink_state.borrow()).into();
        let blink: ColorEaseUniform = (*self.blink_state.borrow()).into();
        let rapid_blink: ColorEaseUniform = (*self.rapid_blink_state.borrow()).into();

        for layer in gl_state.layers.borrow().iter() {
            for idx in 0..3 {
                let vb = &layer.vb.borrow()[idx];
                let (vertex_count, index_count) = vb.vertex_index_count();
                if vertex_count > 0 {
                    let vertices = vb.current_vb_mut();
                    let subpixel_aa = use_subpixel && idx == 1;

                    let mut uniforms = UniformBuilder::default();

                    uniforms.add("projection", &projection);
                    uniforms.add("atlas_nearest_sampler", &atlas_nearest_sampler);
                    uniforms.add("atlas_linear_sampler", &atlas_linear_sampler);
                    uniforms.add("foreground_text_hsb", &foreground_text_hsb);
                    uniforms.add("subpixel_aa", &subpixel_aa);
                    uniforms.add("milliseconds", &milliseconds);
                    uniforms.add_struct("cursor_blink", &cursor_blink);
                    uniforms.add_struct("blink", &blink);
                    uniforms.add_struct("rapid_blink", &rapid_blink);

                    frame.draw(
                        vertices.glium().slice(0..vertex_count).unwrap(),
                        vb.indices.glium().slice(0..index_count).unwrap(),
                        gl_state.glyph_prog.as_ref().unwrap(),
                        &uniforms,
                        if subpixel_aa {
                            &dual_source_blending
                        } else {
                            &alpha_blending
                        },
                    )?;
                }

                vb.next_index();
            }
        }

        Ok(())
    }
}
