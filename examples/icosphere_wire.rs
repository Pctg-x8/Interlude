
extern crate interlude;
extern crate thread_scoped;
extern crate nalgebra;
extern crate time;
extern crate alga;
use interlude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::mem::size_of;
use nalgebra::*;

fn main() { game().or_crash(); }
fn game() -> EngineResult<()>
{
	let engine = EngineBuilder::<EmptyInput>::new("com.cterm2.interlude.examples.icosphere_wire", (0, 1, 0),
		"Rendering Icosphere[Wireframe]".into(), &Size2(640, 480))
		.asset_base(std::env::current_dir().unwrap().into()).device_feature_nonsolid_fillmode().launch()?;

	// make framebuffer
	let &Size2(w, h) = engine.render_window().size();
	let vport = Viewport::from(engine.render_window().size());
	let fb = engine.render_window().render_targets().iter().map(|v| Framebuffer::new_for_presented(&engine, v, Some(true), &Size2(w, h)))
		.collect::<Result<Vec<_>, _>>()?;

	let (bp, stg, dev) =
	{
		let (v, i) = generate_icosphere();
		let (v, i) = index_triangles(subdiv_icosahedron(associate_vertex_indices(&v, &i)));
		let bp = BufferPreallocator::new(&engine, &[
			BufferContent::Uniform(size_of::<[CMatrix4; 2]>()),
			BufferContent::Vertex(size_of::<CVector4>() * v.len()),
			BufferContent::Index(size_of::<u16>() * i.len())
			// (std::mem::size_of::<[[CVector4; 3]; 80]>(), BufferDataType::Vertex)
		]);
		let (dev, stg) = bp.instantiate()?;
		stg.map().map(|m|
		{
			m.range_mut::<CVector4>(bp.offset(1), v.len()).copy_from_slice(&v[..]);
			m.range_mut::<u16>(bp.offset(2), i.len()).copy_from_slice(&i[..]);
			let proj = Perspective3::new(w as f32 / h as f32, 30.0f32.to_radians(), 0.1, 100.0).unwrap() *
				view_matrix(Vector3::new(5.0, 2.0, 30.0), Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0));
			*m.map_mut::<[CMatrix4; 2]>(bp.offset(0)) = [*proj.as_ref(), *Rotation3::new(Vector3::new(0.0, 1.0, 0.0).normalize() * 0.0).to_homogeneous().as_ref()];
		})?;
		(bp.independence(), stg, dev)
	};

	// load shaders and build pipeline state
	let vshader = VertexShader::from_asset(&engine, "examples.icosphere.vert", "main",
		&[VertexBinding::PerVertex(std::mem::size_of::<CVector4>() as u32)],
		&[VertexAttribute(0, Format::Component(32, PackedPixelOrder::RGBA, FormatType::Float), 0)])?;
	let fshader = FragmentShader::from_asset(&engine, "examples.icosphere.frag", "main")?;
	let dsl_cam = DescriptorSetLayout::new(&engine, vec![Descriptor::Uniform(1, ShaderStage::Vertex.into())].into())?;
	let psl = PipelineLayout::new(&engine, &[&dsl_cam], &[])?;
	let ps_mold = GraphicsPipelineBuilder::new(&psl, PreciseRenderPass(fb[0].renderpass(), 0))
		.primitive_topology(PrimitiveTopology::TriangleList(false))
		.vertex_shader(PipelineShaderProgram::unspecialized(&vshader))
		.rasterizer_enable_wired_mode()
		.viewport_scissors(&[ViewportWithScissorRect::default_scissor(&vport)])
		.fragment_shader(PipelineShaderProgram(fshader.clone(), vec![
			(0, ConstantEntry::Float(1.0)),
			(1, ConstantEntry::Float(1.0)),
			(2, ConstantEntry::Float(1.0)),
			(3, ConstantEntry::Float(1.0))
		]))
		.blend_state(&[AttachmentBlendState::Disabled]);
	let ps = GraphicsPipelines::new(&engine, &[&ps_mold])?.pop().unwrap();

	// create descriptor sets
	let descriptor_sets = DescriptorSets::new(&engine, &[&dsl_cam])?;
	let ubuf_info = BufferInfo(&dev, bp.offset(0) .. bp.offset(1));
	engine.update_descriptors(&[DescriptorSetWriteInfo::UniformBuffer(descriptor_sets[0], 0, vec![ubuf_info])]);

	// transfer data / setting image layout
	let initial_buffer_transfer_barriers = [
		BufferMemoryBarrier { buffer: &stg, range: 0 .. bp.total_size(), dst_access: AccessFlag::TransferRead.into(), .. Default::default() },
		BufferMemoryBarrier { buffer: &dev, range: 0 .. bp.total_size(), dst_access: AccessFlag::TransferWrite.into(), .. Default::default() }
	];
	let initial_buffer_transfer_ret = BufferMemoryBarrier
	{
		src_access: AccessFlag::VertexAttributeRead | AccessFlag::IndexRead | AccessFlag::UniformRead, .. initial_buffer_transfer_barriers[1].clone()
	}.flip();
	ImmediateTransferCommandSubmission::begin(&engine)?
		.pipeline_barrier_on(PipelineStage::Transfer, false, &[], &initial_buffer_transfer_barriers,
			&engine.render_window().render_targets().iter().map(|x| ImageMemoryBarrier::initialize_undef(x, ImageSubresourceRange
			{
				aspect: ImageAspect::Color.into(), .. Default::default()
			}, AccessFlag::MemoryRead.into(), ImageLayout::PresentSrc)).collect::<Vec<_>>())
		.copy_buffer(&stg, &dev, &[BufferCopyRegion(0, 0, bp.total_size())])
		.pipeline_barrier_on(PipelineStage::Transfer, false, &[], &[initial_buffer_transfer_ret], &[]);

	// Draw commands and submit it
	let cb = GraphicsCommandBuffers::new(&engine, engine.render_window().render_targets().len())?;
	for (n, recorder) in cb.begin_all().enumerate()
	{
		recorder?
			.pipeline_barrier_on(PipelineStage::ColorAttachmentOutput, true, &[], &[], &[ImageMemoryBarrier
			{
				image: &engine.render_window().render_targets()[n],
				subresource_range: ImageSubresourceRange { aspect: ImageAspect::Color.into(), .. Default::default() },
				src_access: AccessFlag::MemoryRead.into(), dst_access: AccessFlag::ColorAttachmentWrite.into(),
				src_layout: ImageLayout::PresentSrc, dst_layout: ImageLayout::ColorAttachmentOptimal, .. Default::default()
			}])
			.begin_render_pass(&fb[n], &[AttachmentClearValue::Color(0.0, 0.0, 0.0, 1.0)], false)
			.bind_pipeline(&ps)
			.bind_descriptor_sets(&psl, &[descriptor_sets[0]])
			.bind_vertex_buffers(&[(&dev, bp.offset(1))])
			.bind_index_buffer(&dev, bp.offset(2))
			.draw_indexed(4 * 20 * 3, 1, 0)
			// .draw(80 * 3, 1)
			.end_render_pass()
		.end()?;
	}

	// Update commands
	let updated_matrix_start = bp.offset(0) + size_of::<CMatrix4>();
	let ucb = TransferCommandBuffers::new(&engine, 1)?;
	ucb.begin(0).and_then(|rec|
	{
		let transfer_buffer_barriers = [
			BufferMemoryBarrier
			{
				src_access: AccessFlag::HostWrite.into(), dst_access: AccessFlag::TransferRead.into(),
				buffer: &stg, range: updated_matrix_start .. bp.offset(1), .. Default::default()
			},
			BufferMemoryBarrier
			{
				src_access: AccessFlag::VertexAttributeRead | AccessFlag::IndexRead | AccessFlag::UniformRead, dst_access: AccessFlag::TransferWrite.into(),
				buffer: &dev, range: updated_matrix_start .. bp.offset(1), .. Default::default()
			}
		];
		let transfer_buffer_barriers_ret = transfer_buffer_barriers.iter().map(BufferMemoryBarrier::flip).collect::<Vec<_>>();
		rec.pipeline_barrier_on(PipelineStage::Transfer, false, &[], &transfer_buffer_barriers, &[])
			.copy_buffer(&stg, &dev, &[BufferCopyRegion(updated_matrix_start, updated_matrix_start, bp.offset(1))])
			.pipeline_barrier_on(PipelineStage::Transfer, false, &[], &transfer_buffer_barriers_ret, &[])
			.end()
	})?;

	// Process Loop(Saving CPU usage)
	{
		let window_system = engine.render_window().clone();
		let ordersem = QueueFence::new(&engine)?;
		let (copy_completion, render_completion) = (Fence::new(&engine)?, Fence::new(&engine)?);
		let exit_signal = Arc::new(AtomicBool::new(false));
		let exit_signal_uo = exit_signal.clone();
		let update_event = Event::new("Update Event")?;
		let update_event_uo = update_event.clone();
		let update_observer = unsafe { thread_scoped::scoped(move ||
		{
			let mut frame_index = engine.render_window().acquire_next_target_index(&ordersem).and_then(|f|
				engine.submit_graphics_commands(&[cb[f as usize]], &[(&ordersem, &PipelineStage::ColorAttachmentOutput)],
					None, Some(&render_completion)).map(|_| f)
				).or_crash();
			while !exit_signal_uo.load(Ordering::Acquire)
			{
				render_completion.wait(None).and_then(|()| render_completion.clear()).or_crash();
				engine.submit_transfer_commands(&ucb[..], &[], None, Some(&copy_completion)).or_crash();
				copy_completion.wait(None).and_then(|()| copy_completion.clear()).or_crash();
				update_event_uo.set();
				frame_index = engine.render_window().present(&engine, frame_index, None).and_then(|_|
					engine.render_window().acquire_next_target_index(&ordersem).and_then(|f|
						engine.submit_graphics_commands(&[cb[f as usize]], &[(&ordersem, &PipelineStage::ColorAttachmentOutput)],
							None, Some(&render_completion)).map(|_| f)
						)
					).or_crash();
			}

			engine.wait_device().or_crash();
			engine
		}) };

		let mapped = stg.map()?;
		let mut model_rot = mapped.map_mut::<CMatrix4>(updated_matrix_start);
		let start_time = time::PreciseTime::now();
		loop
		{
			match window_system.process_events_and_messages(&[&update_event])
			{
				ApplicationState::Exited => break,
				ApplicationState::EventArrived(0) =>
				{
					update_event.reset();
					let elapsed = start_time.to(time::PreciseTime::now());
					*model_rot = *Rotation3::new(Vector3::new(0.1, 1.0, 0.0).normalize() * (230.0f32 * elapsed.num_microseconds().unwrap() as f32 / 1_000_000.0f32).to_radians())
						.to_homogeneous().as_ref();
				},
				_ => ()
			}
		}
		exit_signal.store(true, Ordering::Release);
		update_observer.join()
	};

	Ok(())
}

/// Interlude:drafting Generate Icosphere mesh and indices
fn generate_icosphere() -> ([CVector4; 12], [[u16; 3]; 20])
{
	let t = (1.0 + 5.0f32.sqrt()) / 2.0;
	let vertices = [
		[-1.0, t, 0.0, 1.0], [1.0, t, 0.0, 1.0], [-1.0, -t, 0.0, 1.0], [1.0, -t, 0.0, 1.0],
		[0.0, -1.0, t, 1.0], [0.0, 1.0, t, 1.0], [0.0, -1.0, -t, 1.0], [0.0, 1.0, -t, 1.0],
		[t, 0.0, -1.0, 1.0], [t, 0.0, 1.0, 1.0], [-t, 0.0, -1.0, 1.0], [-t, 0.0, 1.0, 1.0]
	];

	(vertices, [
		[0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
		[1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
		[3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
		[4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1]
	])
}
fn associate_vertex_indices<T: Copy>(v: &[T], i: &[[u16; 3]]) -> Vec<[T; 3]>
{
	i.into_iter().map(|iv| [v[iv[0] as usize], v[iv[1] as usize], v[iv[2] as usize]]).collect()
}
fn icosahedron_middle(a: CVector4, b: CVector4) -> CVector4
{
	let temp_v = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5, (a[3] + b[3]) * 0.5];
	let temp_vlen = (temp_v[0].powf(2.0) + temp_v[1].powf(2.0) + temp_v[2].powf(2.0)).sqrt();
	let temp_v = [temp_v[0] / temp_vlen, temp_v[1] / temp_vlen, temp_v[2] / temp_vlen, temp_v[3]];
	let gratio = (1.0 + 5.0f32.sqrt()) / 2.0;
	let offs = (gratio * gratio + 1.0).sqrt();
	[temp_v[0] * offs, temp_v[1] * offs, temp_v[2] * offs, temp_v[3]]
}
fn subdiv_triangle(v: [CVector4; 3]) -> [[CVector4; 3]; 4]
{
	let newv = (icosahedron_middle(v[0], v[1]), icosahedron_middle(v[1], v[2]), icosahedron_middle(v[2], v[0]));

	[
		[v[0], newv.0, newv.2],
		[newv.0, v[1], newv.1],
		[newv.2, newv.1, v[2]],
		[newv.0, newv.1, newv.2]
	]
}
fn subdiv_icosahedron(v: Vec<[CVector4; 3]>) -> Vec<[CVector4; 3]>
{
	v.into_iter().flat_map(|v| Vec::from(&subdiv_triangle(v)[..]).into_iter()).collect()
}
fn index_triangles(v: Vec<[CVector4; 3]>) -> (Vec<CVector4>, Vec<u16>)
{
	let (mut verts, mut indices) = (Vec::new(), Vec::new());

	for t in v.into_iter()
	{
		for n in 0 .. 3
		{
			match verts.iter().enumerate().find(|&(_, x): &(usize, &CVector4)| ((x[0usize] as f32 - t[n][0]).powf(2.0) + (x[1] as f32 - t[n][1]).powf(2.0) + (x[2] as f32 - t[n][2]).powf(2.0)).sqrt() <= std::f32::EPSILON)
			{
				Some((i, _)) => indices.push(i as u16),
				None =>
				{
					verts.push(t[n]);
					indices.push(verts.len() as u16 - 1);
				}
			}
		}
	}

	(verts, indices)
}

use alga::general::{Real, Ring};
fn view_matrix<N: Scalar + Ring + Real>(eye: Vector3<N>, target: Vector3<N>, up: Vector3<N>) -> Matrix4<N>
{
	let zaxis = (eye - target).normalize();
	let xaxis = up.cross(&zaxis).normalize();
	let yaxis = zaxis.cross(&xaxis);

	Matrix4::new(xaxis.x, xaxis.y, xaxis.z, -eye.dot(&xaxis),
		yaxis.x, yaxis.y, yaxis.z, -eye.dot(&yaxis),
		zaxis.x, zaxis.y, zaxis.z, -eye.dot(&zaxis),
		N::zero(), N::zero(), N::zero(), N::one())
}
