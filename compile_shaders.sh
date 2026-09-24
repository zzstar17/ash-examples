#!/bin/bash

DIR=$(dirname "$0")
OUT_DIR=$DIR/assets/shaders

VK_ENV="vulkan1.3"

glslc -O $DIR/src/render/shaders/shader.vert --target-env=$VK_ENV -o $OUT_DIR/vert.spv
glslc -O $DIR/src/render/shaders/shader.frag --target-env=$VK_ENV -o $OUT_DIR/frag.spv

glslc -O $DIR/src/render/shaders/shader.vert --target-env=$VK_ENV -o $OUT_DIR/vert_debug.spv -g
glslc -O $DIR/src/render/shaders/shader.frag --target-env=$VK_ENV -o $OUT_DIR/frag_debug.spv -g

dxc -spirv -T ps_6_0 -E main $DIR/src/render/shaders/slug_pixel_shader.hlsl -Fo $OUT_DIR/slug_pixel.spv
dxc -spirv -T vs_6_0 -E main $DIR/src/render/shaders/slug_vertex_shader.hlsl -Fo $OUT_DIR/slug_vertex.spv

dxc -spirv -T ps_6_0 -E main $DIR/src/render/shaders/slug_pixel_shader.hlsl -Fo $OUT_DIR/slug_pixel_debug.spv -Zi -fspv-debug=vulkan-with-source
dxc -spirv -T vs_6_0 -E main $DIR/src/render/shaders/slug_vertex_shader.hlsl -Fo $OUT_DIR/slug_vertex_debug.spv -Zi -fspv-debug=vulkan-with-source