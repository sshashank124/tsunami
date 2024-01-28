#version 460
#extension GL_EXT_buffer_reference2 : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_shader_explicit_arithmetic_types_int64 : require

#include "inputs.h.glsl"
#include "scene.h.glsl"

layout(push_constant) uniform _PushConstants { RasterizerConstants constants; };

layout(set=0, binding=1) uniform _SceneDesc { SceneDesc scene_desc; };

layout(location=0) out vec4 color;

layout(buffer_reference, scalar) buffer Materials { Material m[]; };

void main() {
  Materials materials = Materials(scene_desc.materials_address);
  color = materials.m[constants.material_index].color;
}
