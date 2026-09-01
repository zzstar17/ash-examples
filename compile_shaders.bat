
glslc -O .\src\render\shaders\shader.vert --target-env=vulkan1.3 -o .\shaders\vert.spv
glslc -O .\src\render\shaders\shader.frag --target-env=vulkan1.3 -o .\shaders\frag.spv

glslc -O .\src\render\shaders\shader.vert --target-env=vulkan1.3 -o .\shaders\vert_debug.spv -g
glslc -O .\src\render\shaders\shader.frag --target-env=vulkan1.3 -o .\shaders\frag_debug.spv -g

dxc.exe -spirv -T ps_6_0 -E main .\src\render\shaders\slug_pixel_shader.hlsl -Fo .\shaders\slug_pixel.spv
dxc.exe -spirv -T vs_6_0 -E main .\src\render\shaders\slug_vertex_shader.hlsl -Fo .\shaders\slug_vertex.spv

dxc.exe -spirv -T ps_6_0 -E main .\src\render\shaders\slug_pixel_shader.hlsl -Fo .\shaders\slug_pixel_debug.spv -Zi -fspv-debug=vulkan-with-source
dxc.exe -spirv -T vs_6_0 -E main .\src\render\shaders\slug_vertex_shader.hlsl -Fo .\shaders\slug_vertex_debug.spv -Zi -fspv-debug=vulkan-with-source

glslc -O .\src\render\shaders\compute\shader.comp --target-env=vulkan1.3 -o .\shaders\compute\shader.spv
glslc -O .\src\render\shaders\compute\shader.comp --target-env=vulkan1.3 -o .\shaders\compute\shader_debug.spv -g
