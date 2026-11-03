#!/bin/bash

DIR=$(dirname "$0")
OUT_DIR=$DIR/assets/shaders

VK_ENV="vulkan1.3"

mkdir -p $OUT_DIR

glslc -O $DIR/src/render/shaders/main.vert --target-env=$VK_ENV -o $OUT_DIR/main_vert.spv
glslc -O $DIR/src/render/shaders/main.frag --target-env=$VK_ENV -o $OUT_DIR/main_frag.spv

glslc -O $DIR/src/render/shaders/main.vert --target-env=$VK_ENV -o $OUT_DIR/main_vert_debug.spv -g
glslc -O $DIR/src/render/shaders/main.frag --target-env=$VK_ENV -o $OUT_DIR/main_frag_debug.spv -g

glslc -O $DIR/src/render/shaders/skybox.vert --target-env=$VK_ENV -o $OUT_DIR/skybox_vert.spv
glslc -O $DIR/src/render/shaders/skybox.frag --target-env=$VK_ENV -o $OUT_DIR/skybox_frag.spv

glslc -O $DIR/src/render/shaders/skybox.vert --target-env=$VK_ENV -o $OUT_DIR/skybox_vert_debug.spv -g
glslc -O $DIR/src/render/shaders/skybox.frag --target-env=$VK_ENV -o $OUT_DIR/skybox_frag_debug.spv -g

glslc -O $DIR/src/render/shaders/color.vert --target-env=$VK_ENV -o $OUT_DIR/color_vert.spv
glslc -O $DIR/src/render/shaders/color.frag --target-env=$VK_ENV -o $OUT_DIR/color_frag.spv

glslc -O $DIR/src/render/shaders/color.vert --target-env=$VK_ENV -o $OUT_DIR/color_vert_debug.spv -g
glslc -O $DIR/src/render/shaders/color.frag --target-env=$VK_ENV -o $OUT_DIR/color_frag_debug.spv -g

dxc -spirv -T ps_6_0 -E main $DIR/src/render/shaders/slug_pixel_shader.hlsl -Fo $OUT_DIR/slug_pixel.spv
dxc -spirv -T vs_6_0 -E main $DIR/src/render/shaders/slug_vertex_shader.hlsl -Fo $OUT_DIR/slug_vertex.spv

dxc -spirv -T ps_6_0 -E main $DIR/src/render/shaders/slug_pixel_shader.hlsl -Fo $OUT_DIR/slug_pixel_debug.spv -Zi -fspv-debug=vulkan-with-source
dxc -spirv -T vs_6_0 -E main $DIR/src/render/shaders/slug_vertex_shader.hlsl -Fo $OUT_DIR/slug_vertex_debug.spv -Zi -fspv-debug=vulkan-with-source