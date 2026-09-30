<script setup>
import { computed, onMounted, ref } from "vue";
import { Message } from "@arco-design/web-vue";
import UiIcon from "../components/common/UiIcon.vue";
import { call, desktop, errorText, refreshAndroidEnvironment, runtime, saveAndroidSettings, taskList } from "../services/runtime";

const checking = ref(false);
const installing = ref(false);
const error = ref("");
const androidTasks = computed(() => taskList.value.filter(task => task.executor === "android"));
async function refresh() {
    if (!desktop) return;
    checking.value = true; error.value = "";
    try { await refreshAndroidEnvironment(); }
    catch (value) { error.value = errorText(value); }
    finally { checking.value = false; }
}
async function install() {
    installing.value = true; error.value = "";
    try {
        const result = await call("setup_android_environment", { pythonPath: runtime.android.pythonPath });
        Message.success(result);
        await refresh();
    } catch (value) { error.value = errorText(value); }
    finally { installing.value = false; }
}
function save() { saveAndroidSettings(runtime.android); }
onMounted(refresh);
</script>

<template>
    <div class="android-page">
        <div class="page-heading"><div><div class="eyebrow">ANDROID EXECUTOR</div><h1>Android 设备</h1><p>连接真机并检查 ADB、Python 与 UIAutomator2。</p></div><button class="button secondary" :disabled="checking || !desktop" @click="refresh"><UiIcon name="refresh" :class="{ spinning: checking }" />{{ checking ? '检查中…' : '刷新状态' }}</button></div>
        <div v-if="error" class="notice error" role="alert"><UiIcon name="info" />{{ error }}</div>
        <section class="panel"><div class="section-heading"><div><h2>运行环境</h2><p>HaTickets Mobile 需要 Python 3.10–3.13；ADB 来自 Android SDK Platform Tools。</p></div></div>
            <div class="two-fields"><div class="field"><label class="field-label" for="android-python">Python 路径</label><input id="android-python" class="text-input" v-model="runtime.android.pythonPath" @change="save" placeholder="python（或已安装的 Python 3.10–3.13 路径）" /></div><div class="field"><label class="field-label" for="android-adb">ADB 路径</label><input id="android-adb" class="text-input" v-model="runtime.android.adbPath" @change="save" placeholder="adb（或 platform-tools/adb 路径）" /></div></div>
            <div v-if="runtime.android.environment" class="android-environment"><span :class="runtime.android.environment.pythonSupported ? 'status-good' : 'status-bad'">Python {{ runtime.android.environment.pythonVersion || '未找到' }}</span><span :class="runtime.android.environment.uiautomator2Ready ? 'status-good' : 'status-bad'">UIAutomator2 {{ runtime.android.environment.uiautomator2Ready ? '已就绪' : '未就绪' }}</span><span :class="runtime.android.environment.adbReady ? 'status-good' : 'status-bad'">ADB {{ runtime.android.environment.adbReady ? '已就绪' : '未找到' }}</span></div>
            <p class="field-hint">安装依赖会在应用数据目录创建独立 Python 虚拟环境，不修改现有项目依赖。请先安装受支持的 Python；ADB 需自行安装并在手机开启 USB 调试。</p>
            <button class="button secondary small" :disabled="installing || !desktop" @click="install">{{ installing ? '正在安装…' : '安装或修复 UIAutomator2 环境' }}</button>
        </section>
        <section class="panel space-top"><div class="section-heading"><div><h2>已连接设备</h2><p>只有状态为 device 的设备可以执行购票；请先在手机上授权 USB 调试。</p></div></div>
            <div v-if="runtime.android.environment?.devices?.length" class="device-list"><label v-for="device in runtime.android.environment.devices" :key="device.serial" class="buyer-card"><input type="radio" name="android-device" :value="device.serial" v-model="runtime.android.serial" :disabled="device.state !== 'device'" @change="save" /><div><strong>{{ device.serial }}</strong><small>{{ device.state }} · {{ device.detail }}</small></div></label></div>
            <p v-else class="field-hint">尚未发现设备。连接手机后点击“刷新状态”。</p>
        </section>
        <section class="panel space-top"><div class="section-heading"><div><h2>执行日志</h2><p>本次应用运行中的 Android 日志，仅保留最近 200 行；下单结果以任务状态为准。</p></div></div>
            <div v-if="androidTasks.length" class="android-runs"><article v-for="task in androidTasks" :key="task.id"><h3>{{ task.title }} · {{ task.status }}</h3><pre>{{ (runtime.android.logs[task.id] || []).join('\n') || '暂无日志' }}</pre></article></div>
            <p v-else class="field-hint">尚无 Android 执行任务。</p>
        </section>
    </div>
</template>

<style scoped>
.android-page > .panel { margin-top: 18px; }
.android-environment { display: flex; flex-wrap: wrap; gap: 12px; margin: 16px 0; font-size: 12px; }
.status-good { color: var(--success); }
.status-bad { color: var(--danger); }
.device-list { display: flex; flex-wrap: wrap; gap: 10px; }
.android-runs { display: grid; gap: 16px; }
.android-runs pre { max-height: 220px; overflow: auto; white-space: pre-wrap; overflow-wrap: anywhere; padding: 12px; background: var(--surface-alt, #f6f7f8); border-radius: 8px; font-size: 11px; }
</style>
