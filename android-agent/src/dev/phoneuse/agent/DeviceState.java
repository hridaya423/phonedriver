package dev.phoneuse.agent;

import android.view.accessibility.AccessibilityWindowInfo;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import java.util.List;
import org.json.JSONObject;

final class DeviceState {
    static JSONObject state() {
        boolean imeVisible = false;
        String foreground = null;
        List<AccessibilityWindowInfo> windows = Accessibility.windows();
        for (AccessibilityWindowInfo w : windows) {
            if (w.getType() == AccessibilityWindowInfo.TYPE_INPUT_METHOD) {
                imeVisible = true;
            }
            if (foreground == null && w.getType() == AccessibilityWindowInfo.TYPE_APPLICATION && w.isActive()) {
                foreground = Accessibility.packageOf(w);
            }
        }
        JSONObject r = new JSONObject();
        Json.put(r, "screen_on", ServiceManager.getPowerManager().isScreenOn(0));
        Json.put(r, "locked", Keyguard.locked());
        Json.put(r, "secure_keyguard", Keyguard.secure());
        Json.put(r, "ime_visible", imeVisible);
        Json.put(r, "foreground", foreground == null ? JSONObject.NULL : foreground);
        return r;
    }

    static JSONObject power() {
        JSONObject r = new JSONObject();
        Json.put(r, "screen_on", ServiceManager.getPowerManager().isScreenOn(0));
        Json.put(r, "locked", Keyguard.locked());
        Json.put(r, "secure_keyguard", Keyguard.secure());
        boolean focused = false;
        for (AccessibilityWindowInfo w : Accessibility.windows()) {
            focused |= w.isFocused();
        }
        Json.put(r, "focused", focused);
        return r;
    }

    private DeviceState() {}
}
