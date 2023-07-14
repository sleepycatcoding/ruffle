#import filter

struct Filter {
    // Secretly a vec2<f32> but within alignment rules
    dir_x: f32,
    dir_y: f32,

    // Full size of the blur kernel (from left to right, ie). Must be a whole integer.
    full_size: f32,

    _pad: f32,
}

@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> filter_args: Filter;

@vertex
fn main_vertex(in: filter::VertexInput) -> filter::VertexOutput {
    return filter::main_vertex(in);
}

@fragment
fn main_fragment(in: filter::VertexOutput) -> @location(0) vec4<f32> {
    let direction = vec2<f32>(filter_args.dir_x, filter_args.dir_y);

    // Left edge. Always lands in the middle of the first pixel inside this blur.
    let left_uv = in.uv - (direction * floor(filter_args.full_size / 2.0));

    // Left edge weight. Odd width has a weight of 1, even width has a weight of 0.5.
    let left_weight = ((filter_args.full_size % 2.0) * 0.5) + 0.5;

    // We always start off with the left edge. Everything else is optional.
    var center_length = filter_args.full_size - left_weight;
    var total = textureSample(texture, texture_sampler, left_uv) * left_weight;

    if (filter_args.full_size % 2.0 == 0.0) {
        // If the width is even, we have a right edge of a fixed weight and offset
        center_length -= 1.5;
        total += textureSample(texture, texture_sampler, left_uv + (direction * (filter_args.full_size - 0.75))) * 1.5;
    }

    // At this point, the center_length must be a whole number, divisible by 2.
    center_length /= 2.0;
    for (var i = 0.0; i < center_length; i += 1.0) {
        // The center of the kernel is always going to be 1,1 weight pairs. We can just sample between the two pixels.
        total += textureSample(texture, texture_sampler, left_uv + (direction * (1.5 + (i * 2.0)))) * 2.0;
    }

    // The sum of every weight is full_size
    return total / filter_args.full_size;
}
