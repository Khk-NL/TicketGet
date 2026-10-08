<script setup>
import { computed, onBeforeUnmount, reactive, ref } from "vue";
import { Message } from "@arco-design/web-vue";
import { call, desktop, runtime, saveAccount, deleteAccount, setDefaultAccount, reloadAccounts, errorText } from "../services/runtime";
import { platforms, formatTime } from "../services/platforms";
import UiIcon from "./common/UiIcon.vue";

const platform = ref("dm");
const accounts = computed(() => runtime.accounts.items.filter(item => item.platform === platform.value));
const form = reactive({ id: "", name: "", cookie: "" });
const showCookie = ref(false);
const error = ref("");
const loginOpen = ref(false);
const loginBusy = ref(false);
const windows = navigator.userAgent.includes("Windows");
const browserChoice = ref("auto");
async function browserLogin() {
    error.value = "";
    if (!form.name.trim()) { error.value = "请先填写账号名称"; return; }
    loginBusy.value = true;
    try { await call("start_damai_browser_login", { browserChoice: browserChoice.value }); loginOpen.value = true; }
    catch (value) { error.value = errorText(value); }
    finally { loginBusy.value = false; }
}
async function captureCookie() {
    error.value = "";
    loginBusy.value = true;
    try {
        const cookie = await call("read_damai_browser_cookie");
        loginOpen.value = false;
        await saveAccount({ id: form.id, name: form.name, platform: "dm", cookie });
        reset();
        Message.success("大麦 Cookie 已读取并安全保存");
    } catch (value) { error.value = errorText(value); }
    finally { loginBusy.value = false; }
}
async function cancelLogin() {
    loginBusy.value = true;
    try { await call("cancel_damai_browser_login"); loginOpen.value = false; }
    catch (value) { error.value = errorText(value); }
    finally { loginBusy.value = false; }
}
onBeforeUnmount(() => { if (loginOpen.value) call("cancel_damai_browser_login").catch(() => {}); });
function reset() { Object.assign(form, { id: "", name: "", cookie: "" }); showCookie.value = false; error.value = ""; }
function selectPlatform(id) { platform.value = id; reset(); }
function edit(account) { Object.assign(form, { id: account.id, name: account.name, cookie: account.cookie }); showCookie.value = false; error.value = ""; }
async function save() {
    error.value = "";
    try { await saveAccount({ ...form, platform: platform.value }); reset(); Message.success("账号已保存，购票和监控可直接选用"); }
    catch (value) { error.value = errorText(value); }
}
async function remove(id) {
    try { await deleteAccount(id); if (form.id === id) reset(); Message.success("账号已删除"); }
    catch (value) { error.value = errorText(value); }
}
function makeDefault(id) {
    try { setDefaultAccount(id); Message.success("已设为该平台默认账号"); }
    catch (value) { error.value = errorText(value); }
}
</script>

<template>
    <section id="account-settings" class="panel settings-panel account-settings">
        <div class="section-heading"><span class="section-icon"><UiIcon name="user" /></span><div><h2>账号与 Cookie</h2><p>保存一次，购票和多个监控共用</p></div></div>
        <div class="segmented-control" aria-label="管理账号平台"><button v-for="(meta, id) in platforms" :key="id" :class="{ selected: platform === id }" :aria-pressed="platform === id" :disabled="loginOpen" @click="selectPlatform(id)">{{ meta.name }}</button></div>
        <div v-if="runtime.accountError" class="notice error" role="alert"><span>{{ runtime.accountError }}</span><button class="text-button" @click="reloadAccounts">重新读取</button></div>
        <div class="account-list">
            <article v-for="account in accounts" :key="account.id" class="account-row">
                <div class="account-description"><strong>{{ account.name }}</strong><span v-if="runtime.accounts.defaults[platform] === account.id" class="pill">默认</span><small>Cookie 已保存 · {{ formatTime(account.updatedAt) }}</small></div>
                <div class="account-actions"><button v-if="runtime.accounts.defaults[platform] !== account.id" class="text-button" :disabled="loginOpen" @click="makeDefault(account.id)">设为默认</button><button class="text-button" :disabled="loginOpen" @click="edit(account)">编辑</button><button class="text-button" :disabled="loginOpen" @click="remove(account.id)">删除</button></div>
            </article>
            <p v-if="!accounts.length" class="field-hint">尚无{{ platforms[platform].name }}账号，在下方保存 Cookie 后即可使用。</p>
        </div>
        <div class="section-rule"></div>
        <form @submit.prevent="save">
            <div class="label-row"><h3>{{ form.id ? '更新账号' : '添加账号' }}</h3><button v-if="form.id" type="button" class="text-button" :disabled="loginOpen" @click="reset">取消编辑</button></div>
            <div class="field space-top"><label class="field-label" for="account-name">账号名称</label><input id="account-name" class="text-input" v-model="form.name" maxlength="60" placeholder="例如：我的常用账号" autocomplete="off" :disabled="loginOpen" /></div>
            <div v-if="platform === 'dm' && desktop" class="browser-login space-top">
                <label v-if="windows && !loginOpen" class="field-label" for="login-browser">浏览器</label>
                <select v-if="windows && !loginOpen" id="login-browser" class="text-input browser-choice" v-model="browserChoice" :disabled="loginBusy"><option value="auto">自动</option><option value="edge">Edge</option><option value="chrome">Chrome</option></select>
                <button v-if="!loginOpen" class="button small" type="button" :disabled="loginBusy" @click="browserLogin">{{ form.id ? '浏览器刷新 Cookie' : '浏览器登录并获取 Cookie' }}</button>
                <template v-else><span class="field-hint">请在打开的 Chrome 或 Edge 中登录大麦并访问活动页。</span><button class="button small" type="button" :disabled="loginBusy" @click="captureCookie">读取并保存</button><button class="text-button" type="button" :disabled="loginBusy" @click="cancelLogin">取消</button></template>
            </div>
            <div class="label-row space-top"><label class="field-label" for="account-cookie">账号 Cookie</label><button type="button" class="text-button" @click="showCookie = !showCookie">{{ showCookie ? '隐藏' : '显示' }}</button></div>
            <textarea id="account-cookie" class="text-input cookie-input" :class="{ concealed: !showCookie }" rows="4" v-model="form.cookie" placeholder="粘贴已登录账号的完整 Cookie" autocomplete="off" spellcheck="false"></textarea>
            <p class="field-hint">大麦可在浏览器中自行登录后点击“读取并保存”，无需打开开发者工具。Cookie 使用系统凭据存储保护的密钥加密保存在当前设备。保存只检查格式，登录是否有效以平台查询结果为准。</p>
            <p class="field-hint">更新或删除账号后，已启动的任务继续使用启动时的 Cookie；新查询和新任务使用当前账号。Cookie 过期时，在这里更新一次即可。</p>
            <p v-if="error" class="inline-error" role="alert">{{ error }}</p>
            <button class="button primary small space-top" type="submit" :disabled="!!runtime.accountError || loginOpen">{{ form.id ? '保存更新' : '保存账号' }}<UiIcon name="check" /></button>
        </form>
    </section>
</template>

<style scoped>
.account-settings { margin-bottom: 22px; }
.account-list { display: grid; gap: 12px; }
.account-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 14px; border: 1px solid var(--border); border-radius: 8px; }
.account-description { min-width: 0; }
.account-description strong { font-size: 12px; overflow-wrap: anywhere; }
.account-description .pill { margin-left: 8px; }
.account-description small { display: block; color: var(--muted); font-size: 10px; line-height: 1.7; margin-top: 6px; }
.account-actions { display: flex; gap: 10px; flex-shrink: 0; flex-wrap: wrap; }
.browser-login { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
.browser-choice { width: 110px; }
@media (max-width: 700px) { .account-row { align-items: flex-start; flex-direction: column; } }
</style>
