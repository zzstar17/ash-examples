#!/bin/bash

DIR=$(dirname "$0")

VK_ENV="vulkan1.3"

glslc -O $DIR/src/render/shaders/shader.vert --target-env=$VK_ENV -o $DIR/shaders/vert.spv
glslc -O $DIR/src/render/shaders/shader.frag --target-env=$VK_ENV -o $DIR/shaders/frag.spv

glslc -O $DIR/src/render/shaders/shader.vert --target-env=$VK_ENV -o $DIR/shaders/vert_debug.spv -g
glslc -O $DIR/src/render/shaders/shader.frag --target-env=$VK_ENV -o $DIR/shaders/frag_debug.spv -g

glslc -O $DIR/src/render/shaders/compute/shader.comp --target-env=$VK_ENV -o $DIR/shaders/compute/shader.spv
glslc -O $DIR/src/render/shaders/compute/shader.comp --target-env=$VK_ENV -o $DIR/shaders/compute/shader_debug.spv -g

dxc -spirv -T ps_6_0 -E main $DIR/src/render/shaders/slug_pixel_shader.hlsl -Fo $DIR/shaders/slug_pixel.spv
dxc -spirv -T vs_6_0 -E main $DIR/src/render/shaders/slug_vertex_shader.hlsl -Fo $DIR/shaders/slug_vertex.spv

dxc -spirv -T ps_6_0 -E main $DIR/src/render/shaders/slug_pixel_shader.hlsl -Fo $DIR/shaders/slug_pixel_debug.spv -Zi -fspv-debug=vulkan-with-source
dxc -spirv -T vs_6_0 -E main $DIR/src/render/shaders/slug_vertex_shader.hlsl -Fo $DIR/shaders/slug_vertex_debug.spv -Zi -fspv-debug=vulkan-with-source
