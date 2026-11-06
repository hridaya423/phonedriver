package dev.phoneuse.agent;

import org.json.JSONObject;

final class Ops {
    static JSONObject run(JSONObject q) throws Exception {
        String op = q.getString("op");
        JSONObject r = new JSONObject();
        switch (op) {
            case "tree":
                return Accessibility.tree();
            case "gesture":
                Input.gesture(q.getJSONArray("pointers"));
                return r;
            case "key":
                Input.key(q.getInt("keycode"), q.optString("action", "press"));
                return r;
            case "text":
                TextInput.run(q.getString("mode"), q.getString("text"), q.optBoolean("sensitive"));
                return r;
            case "clipboard_get":
                CharSequence text = TextInput.clipboardText();
                return Json.put(r, "text", text == null ? JSONObject.NULL : text.toString());
            case "clipboard_set":
                TextInput.setClipboardText(q.getString("text"), q.optBoolean("sensitive"));
                return r;
            case "apps":
                return Json.put(r, "apps", Apps.list(q.optBoolean("all")));
            case "launch":
                Input.collapsePanels();
                Apps.launch(q.getString("package"));
                return r;
            case "open_url":
                Input.collapsePanels();
                return Json.put(r, "package", Apps.openUrl(q.getString("url")));
            case "notifications":
                Input.expandNotifications();
                return r;
            case "state":
                return DeviceState.state();
            case "power":
                return DeviceState.power();
            case "shell":
                return Shell.run(q.getString("command"), q.optLong("timeout_ms", 30000));
            case "sync_frame":
                Video.requestSyncFrame();
                return r;
            case "wake":
                Accessibility.allowKick();
                Input.key(android.view.KeyEvent.KEYCODE_WAKEUP, "press");
                return r;
            default:
                throw new IllegalArgumentException("unknown op " + op);
        }
    }

    private Ops() {}
}
