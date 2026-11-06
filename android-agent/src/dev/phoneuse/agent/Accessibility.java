package dev.phoneuse.agent;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.app.UiAutomation;
import android.graphics.Rect;
import android.os.Build;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction;
import android.view.accessibility.AccessibilityWindowInfo;
import com.genymobile.scrcpy.display.DisplayInfo;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import java.lang.reflect.Method;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

final class Accessibility {
    private static final int FIRST_WINDOWS_RETRIES = 10;
    private static final int EMPTY_TREE_RETRIES = 40;
    private static final int KICK_MS = 600;
    // Without UNINTERRUPTIBLE the next getChild cancels the prefetch in flight, so every node past the root's
    // first batch costs its own IPC: Settings main (155 nodes) took 78 to 103 ms in getChild alone, 41 to 46 ms with it.
    private static final int PREFETCH = AccessibilityNodeInfo.FLAG_PREFETCH_DESCENDANTS_DEPTH_FIRST
            | AccessibilityNodeInfo.FLAG_PREFETCH_UNINTERRUPTIBLE;

    private static UiAutomation automation;
    private static Method sourceNodeId;
    private static boolean kicked;
    private static int screenW;
    private static int screenH;

    static void connect() throws Exception {
        HandlerThread thread = new HandlerThread("uiautomation");
        thread.start();
        Object connection = Class.forName("android.app.UiAutomationConnection")
                .getDeclaredConstructor().newInstance();
        automation = UiAutomation.class
                .getDeclaredConstructor(Looper.class, Class.forName("android.app.IUiAutomationConnection"))
                .newInstance(thread.getLooper(), connection);
        Method connect = UiAutomation.class.getDeclaredMethod("connect", int.class);
        connect.setAccessible(true);
        try {
            connect.invoke(automation, UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES);
        } catch (java.lang.reflect.InvocationTargetException e) {
            if (e.getCause() instanceof IllegalStateException) {
                throw (IllegalStateException) e.getCause();
            }
            throw e;
        }
        AccessibilityServiceInfo info = automation.getServiceInfo();
        info.flags |= AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS
                | AccessibilityServiceInfo.FLAG_INCLUDE_NOT_IMPORTANT_VIEWS
                | AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
        info.eventTypes = AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED | AccessibilityEvent.TYPE_WINDOWS_CHANGED;
        automation.setServiceInfo(info);
        automation.setOnAccessibilityEventListener(Accessibility::onEvent);
        try {
            sourceNodeId = AccessibilityNodeInfo.class.getDeclaredMethod("getSourceNodeId");
            sourceNodeId.setAccessible(true);
        } catch (NoSuchMethodException e) {
            System.err.println("getSourceNodeId unavailable, refs will not survive recycling");
        }
    }

    private static void onEvent(AccessibilityEvent event) {
        Control c = Main.control;
        if (c == null) {
            return;
        }
        JSONObject e = Json.put(new JSONObject(), "ev", "window_changed");
        Json.put(e, "w", event.getWindowId());
        if (event.getPackageName() != null) {
            Json.put(e, "pkg", event.getPackageName().toString());
        }
        Json.put(e, "t", event.getEventTime());
        c.event(e);
    }

    private static void clearCache() throws Exception {
        if (Build.VERSION.SDK_INT >= 33) {
            automation.clearCache();
            return;
        }
        Class<?> cls = Class.forName("android.view.accessibility.AccessibilityInteractionClient");
        cls.getMethod("clearCache").invoke(cls.getMethod("getInstance").invoke(null));
    }

    static List<AccessibilityWindowInfo> windows() {
        List<AccessibilityWindowInfo> windows = automation.getWindows();
        // spike A1: the first getWindows() right after connect can be empty for about 20 ms
        for (int i = 0; windows.isEmpty() && i < FIRST_WINDOWS_RETRIES; i++) {
            SystemClock.sleep(20);
            windows = automation.getWindows();
        }
        return windows;
    }

    static String packageOf(AccessibilityWindowInfo w) {
        AccessibilityNodeInfo root = w.getRoot();
        return root == null || root.getPackageName() == null ? null : root.getPackageName().toString();
    }

    static AccessibilityNodeInfo focusedInput() throws Exception {
        clearCache();
        return automation.findFocus(AccessibilityNodeInfo.FOCUS_INPUT);
    }

    // Task 3.1: for 50 to 300 ms after a navigation (Back from a second activity, API 37) getWindows() lists only the
    // status bar, or the closing activity's window with a root that has no children left, so the read comes back
    // with at most one node per window. A real screen with nothing but one node per window costs these retries too.
    //
    // On API 26 a connection made while the screen animates forever (the testbed's Busy tab) can see no window at
    // all until the first window change, and neither waiting nor setServiceInfo again nor a tap or key changes
    // that. Opening and closing the notification shade is a window change, and it repairs the connection. It is
    // tried once per connection, because a hollow tree is also what a dark screen looks like.
    static JSONObject tree() throws Exception {
        clearCache();
        DisplayInfo di = ServiceManager.getDisplayManager().getDisplayInfo(0);
        screenW = di.getSize().getWidth();
        screenH = di.getSize().getHeight();
        JSONObject tree = read();
        for (int i = 0; hollow(tree) && i < EMPTY_TREE_RETRIES; i++) {
            SystemClock.sleep(20);
            tree = read();
            if (i == EMPTY_TREE_RETRIES / 2 && !kicked) {
                kicked = true;
                ServiceManager.getStatusBarManager().expandNotificationsPanel();
                SystemClock.sleep(KICK_MS);
                ServiceManager.getStatusBarManager().collapsePanels();
                SystemClock.sleep(KICK_MS);
            }
        }
        return tree;
    }

    // Also hollow: an application window whose root has no children. After the screen has been off, API 26 lists
    // the app's window with an empty root until the next window change.
    private static boolean hollow(JSONObject tree) throws Exception {
        JSONArray windows = tree.getJSONArray("windows");
        JSONArray nodes = tree.getJSONArray("nodes");
        if (nodes.length() <= windows.length()) {
            return true;
        }
        for (int i = 0; i < windows.length(); i++) {
            JSONObject w = windows.getJSONObject(i);
            if (!"application".equals(w.getString("type"))) {
                continue;
            }
            int count = 0;
            for (int j = 0; j < nodes.length(); j++) {
                if (nodes.getJSONObject(j).optInt("w", -1) == w.getInt("id")) {
                    count++;
                }
            }
            if (count <= 1) {
                return true;
            }
        }
        return false;
    }

    static void allowKick() {
        kicked = false;
    }

    private static JSONObject read() {
        JSONArray windows = new JSONArray();
        JSONArray nodes = new JSONArray();
        for (AccessibilityWindowInfo w : automation.getWindows()) {
            Rect b = new Rect();
            w.getBoundsInScreen(b);
            if (isEdgeBar(w, b)) {
                continue;
            }
            AccessibilityNodeInfo root = Build.VERSION.SDK_INT >= 33
                    ? w.getRoot(PREFETCH)
                    : w.getRoot();
            JSONObject wj = new JSONObject();
            Json.put(wj, "id", w.getId());
            Json.put(wj, "type", windowType(w.getType()));
            if (root != null && root.getPackageName() != null) {
                Json.put(wj, "package", root.getPackageName().toString());
            }
            Json.put(wj, "layer", w.getLayer());
            Json.put(wj, "bounds", rect(b));
            windows.put(wj);
            if (root != null) {
                walk(root, -1, nodes);
            }
        }
        return Json.put(Json.put(new JSONObject(), "windows", windows), "nodes", nodes);
    }

    // Status and navigation bars are full-width system windows of at most a tenth of the screen height on the
    // top or bottom edge. Walking them cost about 37 of 192 nodes on Settings (spike A1) and agents never act on them.
    private static boolean isEdgeBar(AccessibilityWindowInfo w, Rect b) {
        return w.getType() == AccessibilityWindowInfo.TYPE_SYSTEM
                && b.width() == screenW
                && b.height() * 10 <= screenH
                && (b.top == 0 || b.bottom == screenH);
    }

    private static JSONArray rect(Rect b) {
        return new JSONArray().put(b.left).put(b.top).put(b.right).put(b.bottom);
    }

    private static String windowType(int type) {
        switch (type) {
            case AccessibilityWindowInfo.TYPE_APPLICATION: return "application";
            case AccessibilityWindowInfo.TYPE_INPUT_METHOD: return "input_method";
            case AccessibilityWindowInfo.TYPE_SYSTEM: return "system";
            case AccessibilityWindowInfo.TYPE_ACCESSIBILITY_OVERLAY: return "accessibility_overlay";
            case AccessibilityWindowInfo.TYPE_SPLIT_SCREEN_DIVIDER: return "split_screen_divider";
            default: return "type" + type;
        }
    }

    private static void walk(AccessibilityNodeInfo n, int parent, JSONArray nodes) {
        int index = nodes.length();
        nodes.put(node(n, parent));
        int count = n.getChildCount();
        for (int i = 0; i < count; i++) {
            AccessibilityNodeInfo child = Build.VERSION.SDK_INT >= 33
                    ? n.getChild(i, PREFETCH)
                    : n.getChild(i);
            if (child != null) {
                walk(child, index, nodes);
            }
        }
    }

    private static void putText(JSONObject o, String key, CharSequence value) {
        if (value != null && value.length() > 0) {
            Json.put(o, key, value.toString());
        }
    }

    private static JSONObject node(AccessibilityNodeInfo n, int parent) {
        JSONObject o = new JSONObject();
        Json.put(o, "p", parent);
        Json.put(o, "w", n.getWindowId());
        if (n.getViewIdResourceName() != null) {
            Json.put(o, "id", n.getViewIdResourceName());
        }
        putText(o, "cls", n.getClassName());
        putText(o, "text", n.getText());
        putText(o, "desc", n.getContentDescription());
        putText(o, "hint", n.getHintText());
        if (Build.VERSION.SDK_INT >= 30) {
            putText(o, "sd", n.getStateDescription());
        }
        Rect b = new Rect();
        n.getBoundsInScreen(b);
        Json.put(o, "b", rect(b));
        StringBuilder f = new StringBuilder();
        if (b.right > 0 && b.bottom > 0 && b.left < screenW && b.top < screenH && b.width() > 0 && b.height() > 0) {
            f.append('v');
        }
        if (n.isVisibleToUser()) f.append('V');
        if (n.isCheckable()) f.append('c');
        if (n.isChecked()) f.append('C');
        if (n.isEnabled()) f.append('e');
        if (n.isEditable()) f.append('E');
        if (n.isFocused()) f.append('F');
        if (n.isSelected()) f.append('s');
        if (n.isPassword()) f.append('P');
        if (n.isScrollable()) f.append('S');
        if (Build.VERSION.SDK_INT >= 28 && n.isHeading()) f.append('H');
        if (n.isShowingHintText()) f.append('h');
        JSONArray actions = new JSONArray();
        for (AccessibilityAction a : n.getActionList()) {
            int id = a.getId();
            if (id == AccessibilityNodeInfo.ACTION_CLICK) actions.put("tap");
            else if (id == AccessibilityNodeInfo.ACTION_LONG_CLICK) actions.put("long_tap");
            else if (id == AccessibilityNodeInfo.ACTION_SET_TEXT) actions.put("set_text");
            else if (id == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD) actions.put("scroll_fwd");
            else if (id == AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD) actions.put("scroll_back");
            else if (id == AccessibilityAction.ACTION_SCROLL_UP.getId()) actions.put("scroll_up");
            else if (id == AccessibilityAction.ACTION_SCROLL_DOWN.getId()) actions.put("scroll_down");
            else if (id == AccessibilityAction.ACTION_SCROLL_LEFT.getId()) actions.put("scroll_left");
            else if (id == AccessibilityAction.ACTION_SCROLL_RIGHT.getId()) actions.put("scroll_right");
            else if (id == AccessibilityNodeInfo.ACTION_EXPAND) f.append('x');
            else if (id == AccessibilityNodeInfo.ACTION_COLLAPSE) f.append('X');
        }
        Json.put(o, "f", f.toString());
        if (actions.length() > 0) {
            Json.put(o, "a", actions);
        }
        AccessibilityNodeInfo.RangeInfo r = n.getRangeInfo();
        if (r != null) {
            JSONObject rj = new JSONObject();
            Json.put(rj, "min", r.getMin());
            Json.put(rj, "max", r.getMax());
            Json.put(rj, "cur", r.getCurrent());
            Json.put(rj, "type", r.getType() == AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_INT ? "int"
                    : r.getType() == AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_PERCENT ? "percent" : "float");
            Json.put(o, "range", rj);
        }
        AccessibilityNodeInfo.CollectionInfo ci = n.getCollectionInfo();
        if (ci != null) {
            Json.put(o, "coll", Json.put(Json.put(new JSONObject(), "rows", ci.getRowCount()), "cols", ci.getColumnCount()));
        }
        AccessibilityNodeInfo.CollectionItemInfo ii = n.getCollectionItemInfo();
        if (ii != null) {
            Json.put(o, "item", Json.put(Json.put(new JSONObject(), "row", ii.getRowIndex()), "col", ii.getColumnIndex()));
        }
        if (sourceNodeId != null) {
            try {
                Json.put(o, "n", Long.toString((Long) sourceNodeId.invoke(n)));
            } catch (ReflectiveOperationException e) {
                System.err.println("getSourceNodeId failed: " + e);
            }
        }
        return o;
    }

    private Accessibility() {}
}
