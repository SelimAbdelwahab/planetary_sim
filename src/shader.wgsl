struct ObjData {
  model : mat4x4<f32>,
  color : vec3<f32>,
  _pad  : f32,
};
@group(0) @binding(0) var<uniform> U : ObjData;

struct VSIn  { @location(0) position: vec3<f32>; }
struct VSOut { @builtin(position) clip_position: vec4<f32>, @location(0) color: vec3<f32> };

@vertex
fn vs_main(in: VSIn) -> VSOut {
  var out: VSOut;
  out.clip_position = U.model * vec4<f32>(in.position, 1.0);
  out.color = U.color;
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  return vec4<f32>(in.color, 1.0);
}