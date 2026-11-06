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
    private static final int SHADE_WAIT_MS = 5000;
    // Without UNINTERRUPTIBLE the next getChild cancels the prefetch in flight, so every node past the root's
    // first batch costs its own IPC: Settings main (155 nodes) took 78 to 103 ms in getChild alone, 41 to 46 ms with it.
    private static final int PREFETCH = AccessibilityNodeInfo.FLAG_PREFETCH_DESCENDANTS_DEPTH_FIRST
            | AccessibilityNodeInfo.FLAG_PREFETCH_UNINTERRUPTIBLE;

    private static UiAutomation automation;
    private static Method sourceNodeId;
    private static boolean kicked;
    private static boolean afterWake;
    private static final java.util.Set<String> waitedWebViews = new java.util.HashSet<>();
    private static final int WEBVIEW_WAIT_MS = 2500;
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
        for (int i = 0; hollow(tree, afterWake) && i < EMPTY_TREE_RETRIES; i++) {
            SystemClock.sleep(20);
            tree = read();
            if (i == EMPTY_TREE_RETRIES / 2 && !kicked) {
                kicked = true;
                kick();
            }
        }
        afterWake = false;
        java.util.Set<String> pending = emptyWebViews(tree);
        for (int waited = 0; !pending.isEmpty() && waited < WEBVIEW_WAIT_MS; waited += 100) {
            SystemClock.sleep(100);
            tree = read();
            pending = emptyWebViews(tree);
        }
        waitedWebViews.addAll(pending);
        return tree;
    }

    // A collapse sent on a timer was lost when System UI was stalled (a cold boot, M2 trial t16): the expand and the
    // collapse reached it back to back and the shade stayed open over the next fourteen scenarios. So the collapse
    // waits until the shade is seen open, and the kick returns once it is seen closed.
    private static void kick() {
        ServiceManager.getStatusBarManager().expandNotificationsPanel();
        waitForShade(true);
        ServiceManager.getStatusBarManager().collapsePanels();
        waitForShade(false);
    }

    private static void waitForShade(boolean open) {
        for (int waited = 0; shadeOpen() != open && waited < SHADE_WAIT_MS; waited += 50) {
            SystemClock.sleep(50);
        }
    }

    // The open shade is the system window that holds input focus (NotificationShade on API 37). A test for "a system
    // window larger than an edge bar" was true before the shade had moved, so the collapse went out early and was lost.
    // The window's root is not read, because that call blocks while System UI is stalled.
    private static boolean shadeOpen() {
        for (AccessibilityWindowInfo w : automation.getWindows()) {
            if (w.getType() == AccessibilityWindowInfo.TYPE_SYSTEM && w.isFocused()) {
                return true;
            }
        }
        return false;
    }

    // Chromium builds a WebView's accessibility tree on the first query, which takes about a second (a page is a
    // leaf node until then), and Compose does the same for its host view, so the first read of each of them waits
    // for its children, at most WEBVIEW_WAIT_MS.
    private static java.util.Set<String> emptyWebViews(JSONObject tree) throws Exception {
        JSONArray nodes = tree.getJSONArray("nodes");
        java.util.Set<String> pending = new java.util.HashSet<>();
        for (int i = 0; i < nodes.length(); i++) {
            JSONObject n = nodes.getJSONObject(i);
            String cls = n.optString("cls");
            if (!"android.webkit.WebView".equals(cls) && !"androidx.compose.ui.platform.AndroidComposeView".equals(cls)) {
                continue;
            }
            boolean hasChild = false;
            for (int j = 0; j < nodes.length() && !hasChild; j++) {
                hasChild = nodes.getJSONObject(j).optInt("p", -1) == i;
            }
            String key = n.optString("n") + "@" + n.optInt("w");
            if (hasChild) {
                waitedWebViews.add(key);
            } else if (!waitedWebViews.contains(key)) {
                pending.add(key);
            }
        }
        return pending;
    }

    // Right after a wake an application window whose root has no children is hollow too: after the screen has been
    // off, API 26 lists the app's window with an empty root until the next window change. Only then, because a cold
    // start looks the same for a moment and the shade kick would cover the app.
    private static boolean hollow(JSONObject tree, boolean emptyAppWindowCounts) throws Exception {
        JSONArray windows = tree.getJSONArray("windows");
        JSONArray nodes = tree.getJSONArray("nodes");
        if (nodes.length() <= windows.length()) {
            return true;
        }
        if (!emptyAppWindowCounts) {
            return false;
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
        afterWake = true;
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
            AccessibilityNodeInfo child;
            try {
                child = Build.VERSION.SDK_INT >= 33 ? n.getChild(i, PREFETCH) : n.getChild(i);
            } catch (NullPointerException gone) {
                // API 37: a WebView that is still building its tree makes getChild throw from the platform's own
                // integrity check (AccessibilityInteractionClient) for about a second after the app starts.
                child = null;
            }
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
        android.os.Bundle extras = n.getExtras();
        CharSequence roleDescription = extras == null ? null : extras.getCharSequence("AccessibilityNodeInfo.roleDescription");
        if (roleDescription != null && roleDescription.length() > 0) {
            Json.put(o, "rd", roleDescription.toString());
        }
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
        // API 26 Settings: the platform reports an unknown count or index as a negative number (-2 seen).
        AccessibilityNodeInfo.CollectionInfo ci = n.getCollectionInfo();
        if (ci != null) {
            Json.put(o, "coll", Json.put(Json.put(new JSONObject(), "rows", Math.max(0, ci.getRowCount())), "cols", Math.max(0, ci.getColumnCount())));
        }
        AccessibilityNodeInfo.CollectionItemInfo ii = n.getCollectionItemInfo();
        if (ii != null) {
            Json.put(o, "item", Json.put(Json.put(new JSONObject(), "row", Math.max(0, ii.getRowIndex())), "col", Math.max(0, ii.getColumnIndex())));
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
