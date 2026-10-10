//! Additive blending (`Graphics::SetDrawMode(DRAWMODE_ADDITIVE)` in the
//! original), used for explosions, sparkles, glows and lights.
//!
//! Bevy sprites only alpha-blend, so additive draws are quads with a
//! 2D material that reuses Bevy's `ColorMaterial` shader and only changes the
//! pipeline's blend state.

use bevy::asset::AssetPath;
use bevy::math::{Affine2, Mat3, Vec4};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    AsBindGroup, AsBindGroupShaderType, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::render::texture::GpuImage;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPipeline};

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
#[uniform(0, AdditiveUniform)]
pub struct AdditiveMaterial {
    pub color: LinearRgba,
    /// Maps the quad's UVs to a sub-rectangle of the texture.
    pub uv_transform: Affine2,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
}

/// Same layout as Bevy's `ColorMaterialUniform`.
#[derive(Clone, Default, ShaderType)]
pub struct AdditiveUniform {
    pub color: Vec4,
    pub uv_transform: Mat3,
    pub flags: u32,
    pub alpha_cutoff: f32,
}

impl AsBindGroupShaderType<AdditiveUniform> for AdditiveMaterial {
    fn as_bind_group_shader_type(&self, _images: &RenderAssets<GpuImage>) -> AdditiveUniform {
        // TEXTURE flag | ALPHA_MODE_BLEND (2 << 30), as in color_material.wesl.
        let flags = if self.texture.is_some() { 1 } else { 0 } | (2u32 << 30);
        AdditiveUniform { color: self.color.to_vec4(), uv_transform: self.uv_transform.into(), flags, alpha_cutoff: 0.5 }
    }
}

impl Material2d for AdditiveMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(AssetPath::parse("embedded://bevy_sprite_render/mesh2d/color_material.wesl").into_owned())
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        _pipeline: &Material2dPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let add = BlendState {
            color: BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
            alpha: BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(add);
            }
        }
        Ok(())
    }
}
