
glslc -O .\src\render\shaders\shader.vert --target-env=vulkan1.3 -o .\shaders\vert.spv
glslc -O .\src\render\shaders\shader.frag --target-env=vulkan1.3 -o .\shaders\frag.spv

dxc.exe -spirv -T ps_6_0 -E main .\src\render\shaders\slug_pixel_shader.hlsl -Fo .\shaders\slug_pixel.spv
dxc.exe -spirv -T vs_6_0 -E main .\src\render\shaders\slug_vertex_shader.hlsl -Fo .\shaders\slug_vertex.spv