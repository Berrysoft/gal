<script setup lang="ts">
import '../live2d/Core/live2dcubismcore'
import { CubismFramework } from '../live2d/Framework/live2dcubismframework';
</script>

<script>
export default {
    data() {
        return {
            gl: null as WebGLRenderingContext | null,
            buffer: null as WebGLFramebuffer | null,
        }
    },
    async created() {
        const canvas = this.$refs.canvas as HTMLCanvasElement
        this.gl = canvas.getContext("webgl")
        if (this.gl) {
            this.buffer = this.gl.getParameter(this.gl.FRAMEBUFFER_BINDING)
            this.gl.enable(this.gl.BLEND)
            this.gl.blendFunc(this.gl.SRC_ALPHA, this.gl.ONE_MINUS_SRC_ALPHA)
            if (!CubismFramework.isStarted()) {
                CubismFramework.startUp()
            }
            if (!CubismFramework.isInitialized()) {
                CubismFramework.initialize()
            }
        }
    },
}
</script>

<template>
    <canvas ref="canvas"></canvas>
</template>
