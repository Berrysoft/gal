<script setup lang="ts">
import { getName, getTauriVersion, getVersion } from '@tauri-apps/api/app'
import { info } from '../interop'
import IconButton from '../components/IconButton.vue';
</script>

<script lang="ts">
export default {
    emits: ["quit"],
    data() {
        return {
            title: "",
            author: "",
            app_name: "",
            app_ver: "",
            tauri_ver: ""
        }
    },
    async created() {
        const res = await info()
        this.title = res.title
        this.author = res.author
        this.app_name = await getName();
        this.app_ver = await getVersion();
        this.tauri_ver = await getTauriVersion();
    }
}
</script>

<template>
    <div class="content">
        <h1>{{ title }}</h1>
        <p>Author: {{ author }}</p>
        <h2>{{ app_name }}</h2>
        <p>Version {{ app_ver }}</p>
        <p>This is a sample GUI frontend of gal project.</p>
        <h2>Tauri</h2>
        <p>Version {{ tauri_ver }}</p>
        <p>
            This is an awesome framework to build cross-platform GUI applications,
            <br />
            with HTML frontend and Rust backend.
        </p>
    </div>
    <div>
        <IconButton icon="arrow-left" @click="$router.back()"></IconButton>
    </div>
</template>
