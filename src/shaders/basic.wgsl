struct VSIn {
  @location(0) position : vec3<f32>,
};

struct VSOut {
  @builtin(position) clip_position : vec4<f32>,
  @location(0) color : vec3<f32>,
};

struct CameraData {
  view_proj : mat4x4<f32>,
};

struct ObjData {
  model : mat4x4<f32>,
  color : vec4<f32>,
};

@group(0) @binding(0)
var<uniform> Camera : CameraData;

@group(1) @binding(0)
var<uniform> U : ObjData;

@vertex
fn vs_main(in: VSIn) -> VSOut {
  var out : VSOut;
  let world = U.model * vec4<f32>(in.position, 1.0);
  out.clip_position = Camera.view_proj * world;
  out.color = U.color.xyz;
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  return vec4<f32>(in.color, 1.0);
}
