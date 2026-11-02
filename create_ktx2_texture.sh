#!/bin/bash

mkdir -p ./assets/tex
ktx create --format R8G8B8A8_SRGB --generate-mipmap ./assets_dev/models/niko/tex/baked.png ./assets/tex/Niko.ktx2
ktx create --format R8G8B8A8_SRGB --generate-mipmap ./assets_dev/models/Kakyoin.png ./assets/tex/Kakyoin.ktx2
ktx create --format R8G8B8A8_SRGB --generate-mipmap ./assets_dev/Ground_02.png ./assets/tex/Ground_02.ktx2
ktx create --format R8G8B8A8_SRGB --cubemap ./assets_dev/skybox/bluecloud_ft.jpg ./assets_dev/skybox/bluecloud_bk.jpg ./assets_dev/skybox/bluecloud_up.jpg ./assets_dev/skybox/bluecloud_dn.jpg ./assets_dev/skybox/bluecloud_rt.jpg ./assets_dev/skybox/bluecloud_lf.jpg ./assets/skybox.ktx2