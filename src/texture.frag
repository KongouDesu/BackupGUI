#version 450

layout(location = 0) in vec2 TexCoords;
layout(location = 0) out vec4 color;
layout(set = 1, binding = 0) uniform texture2D t_Color;
layout(set = 1, binding = 1) uniform sampler s_Color;

void main() {
    vec4 tex = texture(sampler2D(t_Color, s_Color), TexCoords);
    color = vec4(1.0, 1.0, 1.0, 1.0) * tex;
}