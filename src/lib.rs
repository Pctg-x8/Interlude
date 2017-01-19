// Interlude crate: In-home Multimedia Framework powered by Vulkan

// External Crates
extern crate libc;
#[macro_use] extern crate log;
#[cfg(windows)] extern crate winapi;
#[cfg(windows)] extern crate kernel32;
#[cfg(windows)] extern crate user32;
#[cfg(windows)] extern crate widestring;
extern crate nalgebra;
extern crate freetype_sys;
extern crate unicode_normalization;
extern crate ansi_term;
#[cfg(unix)] extern crate xcb;
#[cfg(unix)] extern crate epoll;

// LowLevel APIs
#[macro_use] mod vk;

// Interlude
mod error;
mod engine;
mod ginterface;
mod device;
mod command;
mod resource;
mod framebuffer;
mod synchronize;
mod shading;
mod render_surface;
mod descriptor;
mod input;
mod data;
mod internal_traits;
mod concurrent;
mod tuple_tools;

// platform dependents
#[cfg(unix)] mod linux;
#[cfg(windows)] mod win32;
#[cfg(unix)] pub use linux::NativeInput as Input;
#[cfg(windows)] pub use win32::NativeInput as Input;

/// Application State(has exited?)
#[derive(PartialEq)]
pub enum ApplicationState { Continue, EventArrived(u32), Exited }

// Extra Objects
mod debug_info;

// --- Exported APIs --- //
pub use self::error::*;
pub use self::engine::EngineBuilder;
pub use self::ginterface::GraphicsInterface;
pub use self::framebuffer::{AttachmentDesc, AttachmentRef, PassDesc, PassDependency, AttachmentClearValue, PreciseRenderPass};
pub use self::command::{MemoryBarrier, BufferMemoryBarrier, ImageMemoryBarrier, IndirectCallParameter, BufferCopyRegion, ImageCopyRegion, ImageBlitRegion};
pub use self::resource::{
	ImageSubresourceRange, ImageSubresourceLayers, BufferContent,
	ImageDescriptor1, ImageDescriptor2, ImageDescriptor3, ImagePreallocator,
	SamplerState, ComponentSwizzle, ComponentMapping, Filter
};
pub use self::shading::{
	PipelineShaderProgram, ConstantEntry,
	VertexBinding, VertexAttribute, PushConstantDesc,
	PrimitiveTopology, ViewportWithScissorRect, RasterizerState, AttachmentBlendState,
	GraphicsPipelineBuilder
};
pub use self::descriptor::{ShaderStage, Descriptor, BufferInfo, ImageInfo, DescriptorSetWriteInfo};
pub use self::debug_info::DebugLine;
pub use self::input::*;
pub use self::data::*;
pub use self::vk::ffi;
pub use self::concurrent::*;
pub use self::render_surface::*;
// Transient or Stateful APIs //
pub use self::command::{GraphicsCommandRecorder, TransferCommandRecorder};

// traits
pub use self::engine::{AssetProvider, CommandSubmitter};
pub use self::command::{PrimaryCommandBuffers, SecondaryCommandBuffers, DrawingCommandRecorder};
pub use self::resource::{ImageView, BufferResource, ImageResource};
pub use self::resource::{ImageViewFactory};
pub use self::tuple_tools::{TupleFlatR};
// exported objects
pub use self::engine::Engine;
pub use self::synchronize::{QueueFence, Fence};
pub use self::framebuffer::{RenderPass, Framebuffer};
pub use self::command::{GraphicsCommandBuffers, BundledCommandBuffers, TransferCommandBuffers, TransientTransferCommandBuffers, TransientGraphicsCommandBuffers};
pub use self::resource::{
	Buffer, Image1D, Image2D, Image3D, LinearImage2D, DeviceBuffer, StagingBuffer,
	DeviceImage, StagingImage, MemoryMappedRange, ImageView1D, ImageView2D, ImageView3D,
	Sampler, BufferPreallocator
};
pub use self::shading::{ShaderProgram, PipelineLayout, GraphicsPipelines, GraphicsPipeline};
pub use self::descriptor::{DescriptorSetLayout, DescriptorSets};
pub use self::debug_info::DebugInfo;

// For internal exports //
mod internals
{
	pub use super::internal_traits::*;
	pub use super::engine::*;
	pub use super::error::*;
	pub use super::device::*;
	pub use super::command::*;
	pub use super::resource::*;
	pub use super::synchronize::*;
	pub use super::shading::*;
	pub use super::descriptor::*;
	pub use super::debug_info::*;
	pub use super::data::*;
	pub use super::input::*;
	pub use super::concurrent::*;
	pub use super::tuple_tools::*;
}

type EngineResult<T> = Result<T, EngineError>;
// Result<_, EngineError> as Unrecoverable(Crashes immediately)
#[macro_export]
macro_rules! Unrecoverable
{
	($e: expr) => {match $e
	{
		Err(e) => $crate::crash(e),
		Ok(o) => o
	}}
}
pub trait UnrecoverableExt<T> { fn or_crash(self) -> T; }
impl<T> UnrecoverableExt<T> for EngineResult<T>
{
	fn or_crash(self) -> T
	{
		match self { Err(e) => self::crash(e), Ok(o) => o }
	}
}
