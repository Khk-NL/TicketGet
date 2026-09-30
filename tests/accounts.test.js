import test from "node:test";
import assert from "node:assert/strict";
import { accountStorageKey, accountMetadata, emptyAccounts, cleanAccountDraft, loadAccounts, migrateAccounts, upsertAccount, removeAccount, defaultAccount, resolveAccount } from "../src/services/accounts.js";

function storage(initial = {}) {
    const data = new Map(Object.entries(initial).map(([key, value]) => [key, JSON.stringify(value)]));
    return { data, getItem: key => data.get(key) ?? null, setItem: (key, value) => data.set(key, value) };
}
const dm = (name, cookie = "_m_h5_tk=test_123") => ({ platform: "dm", name, cookie });
function vault() {
    const secrets = new Map();
    return { secrets, put: async (id, value) => { secrets.set(id, value); }, get: async id => {
        if (!secrets.has(id)) throw new Error("missing secret");
        return secrets.get(id);
    } };
}

test("multiple accounts share a platform default and updates preserve their identity", () => {
    let state = upsertAccount(emptyAccounts(), dm("常用"), () => "one", 100);
    state = upsertAccount(state, dm("备用", "session=second"), () => "two", 200);
    state = upsertAccount(state, { platform: "bilibili", name: "哔哩", cookie: "SESSDATA=bili" }, () => "three", 300);
    assert.equal(resolveAccount(state, "dm").id, "one");
    assert.equal(resolveAccount(state, "bilibili").id, "three");
    assert.equal(resolveAccount(state, "bilibili", "one"), undefined);
    state = defaultAccount(state, "two");
    assert.equal(resolveAccount(state, "dm").id, "two");
    const original = structuredClone(state);
    state = upsertAccount(state, { ...dm("备用更新", "session=new"), id: "two" }, () => "unused", 400);
    assert.equal(state.items.length, 3);
    assert.equal(resolveAccount(state, "dm").cookie, "session=new");
    assert.equal(resolveAccount(original, "dm").cookie, "session=second");
    state = removeAccount(state, "two");
    assert.equal(resolveAccount(state, "dm").id, "one");
    assert.equal(resolveAccount(state, "dm", "two"), undefined);
    assert.equal(resolveAccount(removeAccount(state, "one"), "dm"), undefined);
});

test("account validation rejects control characters without revealing credentials", () => {
    for (const cookie of ["", "not a cookie", "session=private\nvalue", "session=private\0value"]) {
        assert.throws(() => upsertAccount(emptyAccounts(), dm("test", cookie)), error => !error.message.includes("private"));
    }
    const state = upsertAccount(emptyAccounts(), dm("test"), () => "one");
    assert.throws(() => upsertAccount(state, { ...dm("test"), id: "removed" }), /删除/);
    assert.throws(() => upsertAccount(state, { ...dm("test"), id: "one", platform: "bilibili" }), /更换平台/);
});

test("legacy saved cookies migrate once, deduplicate and keep account references in drafts", async () => {
    const db = storage({
        "tickets.draft.dm": { url: "123", remember: true, cookie: "session=same" },
        "tickets.monitor-draft.dm": { url: "456", remember: true, cookie: "session=same" },
        "tickets.draft.bilibili": { remember: true, cookie: "SESSDATA=bili" },
    });
    let sequence = 0;
    const secure = vault();
    const state = await migrateAccounts(db, secure, () => `account-${++sequence}`, 100);
    assert.equal(state.items.length, 2);
    assert.equal(JSON.parse(db.getItem("tickets.monitor-draft.dm")).accountId, state.items[0].id);
    assert.equal(JSON.parse(db.getItem("tickets.draft.dm")).url, "123");
    for (const [key, value] of db.data) if (key !== accountStorageKey) assert.doesNotMatch(value, /cookie|remember|session=same|SESSDATA/);
    assert.doesNotMatch(db.getItem(accountStorageKey), /session=same|SESSDATA/);
    assert.deepEqual(await migrateAccounts(db, secure, () => assert.fail("should not duplicate accounts")), state);
});

test("different purchase and monitoring accounts are both retained and unremembered cookies are discarded", async () => {
    const db = storage({
        "tickets.draft.dm": { remember: true, cookie: "session=one" },
        "tickets.monitor-draft.dm": { remember: true, cookie: "session=two" },
        "tickets.draft.bilibili": { remember: false, cookie: "session=unsaved" },
    });
    let sequence = 0;
    const state = await migrateAccounts(db, vault(), () => String(++sequence));
    assert.equal(state.items.length, 2);
    assert.notEqual(JSON.parse(db.getItem("tickets.draft.dm")).accountId, JSON.parse(db.getItem("tickets.monitor-draft.dm")).accountId);
    assert.doesNotMatch(db.getItem(accountStorageKey), /unsaved/);
    assert.deepEqual(cleanAccountDraft({ cookie: "private", remember: true, accountId: "one", url: "123" }), { accountId: "one", url: "123" });
});

test("failed migration preserves original credentials and can safely resume", async () => {
    const db = storage({ "tickets.draft.dm": { remember: true, cookie: "session=original" } });
    const original = db.getItem("tickets.draft.dm");
    const secure = vault();
    const write = db.setItem;
    await assert.rejects(migrateAccounts(db, { ...secure, put: async () => { throw new Error("keychain unavailable"); } }, () => "one"), /keychain/);
    assert.equal(db.getItem("tickets.draft.dm"), original);
    db.setItem = () => { throw new Error("full"); };
    await assert.rejects(migrateAccounts(db, secure, () => "one"), /full/);
    assert.equal(db.getItem("tickets.draft.dm"), original);
    db.setItem = (key, value) => { if (key !== accountStorageKey) throw new Error("cleanup failed"); write(key, value); };
    await assert.rejects(migrateAccounts(db, secure, () => "one"), /cleanup/);
    assert.equal(db.getItem("tickets.draft.dm"), original);
    db.setItem = write;
    const state = await migrateAccounts(db, secure, () => assert.fail("already migrated"));
    assert.equal(state.items.length, 1);
    assert.equal(JSON.parse(db.getItem("tickets.draft.dm")).accountId, "one");
});

test("malformed central account data is not overwritten during migration", () => {
    const db = storage({ [accountStorageKey]: { items: "invalid" }, "tickets.draft.dm": { remember: true, cookie: "session=old" } });
    const original = [...db.data.entries()];
    assert.throws(() => loadAccounts(db));
    assert.deepEqual([...db.data.entries()], original);
});

test("central legacy accounts move into encrypted vault before metadata is saved", async () => {
    const db = storage({ [accountStorageKey]: upsertAccount(emptyAccounts(), dm("常用", "session=private"), () => "one") });
    const secure = vault();
    const state = await migrateAccounts(db, secure);
    assert.equal(state.items[0].cookie, "session=private");
    assert.deepEqual(loadAccounts(db), accountMetadata(state));
    assert.doesNotMatch(db.getItem(accountStorageKey), /session=private/);
    assert.equal(secure.secrets.get("one"), "session=private");
});
