#version 450

layout(set = 0, binding = 0) uniform Transform {
    mat4 transform;
};

layout(location = 0) in vec4 vertex; // <vec2 position, vec2 texCoords>
layout(location = 0) out vec2 TexCoords;


void main() {
    TexCoords = vertex.zw;;
    gl_Position = transform * vec4(vertex.xy, 0.0, 1.0);
}