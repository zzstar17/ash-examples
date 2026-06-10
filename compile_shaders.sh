#!/bin/bash

DIR=$(dirname "$0")

VK_ENV="vulkan1.3"

glslc -O $DIR/src/render/shaders/shader.vert --target-env=$VK_ENV -o $DIR/shaders/vert.spv
glslc -O $DIR/src/render/shaders/shader.frag --target-env=$VK_ENV -o $DIR/shaders/frag.spv

glslc -O $DIR/src/render/shaders/compute/shader.comp --target-env=$VK_ENV -o $DIR/shaders/compute/shader.spv
