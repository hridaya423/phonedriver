package dev.phoneuse.spike;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.app.UiAutomation;
import android.content.Context;
import android.graphics.Rect;
import android.os.HandlerThread;
import android.os.Looper;
import android.util.SparseArray;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction;
import android.view.accessibility.AccessibilityWindowInfo;
import com.genymobile.scrcpy.FakeContext;
import com.genymobile.scrcpy.Workarounds;
import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

public final class Ui {
    public static UiAutomation ua;
    public static String ctorUsed = "";
    public static int connectFlags;
    public static volatile int trips;
    public static int screenW, screenH;
    public static long connectMs;

    public static void connect(int flags, int eventTypes) throws Exception {
        long t0 = System.nanoTime();
        connectFlags = flags;
        HandlerThread ht = new HandlerThread("uia");
        ht.start();
        Looper looper = ht.getLooper();
        Class<?> connCls = Class.forName("android.app.UiAutomationConnection");
        Class<?> ifaceCls = Class.forName("android.app.IUiAutomationConnection");
        Object conn = connCls.getDeclaredConstructor().newInstance();
        StringBuilder sigs = new StringBuilder();
        UiAutomation made = null;
        Exception last = null;
        for (Constructor<?> c : UiAutomation.class.getDeclaredConstructors()) {
            sigs.append(c.toString()).append(" | ");
        }
        String want = System.getenv("CTOR") != null ? System.getenv("CTOR") : "UiAutomation(android.os.Looper";
        for (Constructor<?> c : UiAutomation.class.getDeclaredConstructors()) {
            if (want != null && !c.toString().contains(want)) continue;
            Class<?>[] pt = c.getParameterTypes();
            Object[] args = new Object[pt.length];
            boolean ok = true;
            for (int i = 0; i < pt.length; i++) {
                if (pt[i] == Looper.class) args[i] = looper;
                else if (pt[i] == ifaceCls) args[i] = conn;
                else if (pt[i] == Context.class) {
                    Workarounds.apply();
                    args[i] = FakeContext.get();
                } else if (pt[i] == int.class) args[i] = 0;
                else ok = false;
            }
            if (!ok) continue;
            try {
                c.setAccessible(true);
                made = (UiAutomation) c.newInstance(args);
                ctorUsed = c.toString();
                break;
            } catch (Exception e) {
                last = e;
            }
        }
        if (made == null) throw new IllegalStateException("no UiAutomation constructor worked; declared: " + sigs + " last=" + last);
        System.err.println("ctors: " + sigs);
        ua = made;
        Method m = null;
        try {
            m = UiAutomation.class.getDeclaredMethod("connect", int.class);
        } catch (NoSuchMethodException ignored) {
        }
        if (m != null) {
            m.setAccessible(true);
            m.invoke(ua, flags);
        } else {
            UiAutomation.class.getDeclaredMethod("connect").invoke(ua);
        }
        AccessibilityServiceInfo info = ua.getServiceInfo();
        info.flags |= AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS
                | AccessibilityServiceInfo.FLAG_INCLUDE_NOT_IMPORTANT_VIEWS
                | AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
        info.eventTypes = eventTypes;
        ua.setServiceInfo(info);
        connectMs = (System.nanoTime() - t0) / 1_000_000;
        installTripCounter();
    }

    public static void clearCache() throws Exception {
        if (android.os.Build.VERSION.SDK_INT >= 33) {
            ua.clearCache();
            return;
        }
        Class<?> cls = Class.forName("android.view.accessibility.AccessibilityInteractionClient");
        Object client = cls.getMethod("getInstance").invoke(null);
        cls.getMethod("clearCache").invoke(client);
    }

    public static void disconnect() {
        try {
            Method m = UiAutomation.class.getDeclaredMethod("disconnect");
            m.setAccessible(true);
            m.invoke(ua);
        } catch (Throwable t) {
            try {
                Method m = UiAutomation.class.getDeclaredMethod("destroy");
                m.setAccessible(true);
                m.invoke(ua);
            } catch (Throwable ignored) {
            }
        }
    }

    @SuppressWarnings("unchecked")
    static void installTripCounter() {
        try {
            Class<?> cls = Class.forName("android.view.accessibility.AccessibilityInteractionClient");
            java.lang.reflect.Field f = cls.getDeclaredField("sConnectionCache");
            f.setAccessible(true);
            SparseArray<Object> cache = (SparseArray<Object>) f.get(null);
            Class<?> iface = Class.forName("android.accessibilityservice.IAccessibilityServiceConnection");
            for (int i = 0; i < cache.size(); i++) {
                final Object real = cache.valueAt(i);
                if (Proxy.isProxyClass(real.getClass())) continue;
                Object proxy = Proxy.newProxyInstance(iface.getClassLoader(), new Class<?>[] {iface}, new InvocationHandler() {
                    @Override
                    public Object invoke(Object p, Method method, Object[] args) throws Throwable {
                        if (method.getName().startsWith("findAccessibilityNodeInfo")) trips++;
                        try {
                            return method.invoke(real, args);
                        } catch (java.lang.reflect.InvocationTargetException e) {
                            throw e.getCause();
                        }
                    }
                });
                cache.setValueAt(i, proxy);
            }
        } catch (Throwable t) {
            System.err.println("trip counter unavailable: " + t);
            trips = -1;
        }
    }

    public static int prefetchFlag(String mode) {
        switch (mode) {
            case "hybrid": return AccessibilityNodeInfo.FLAG_PREFETCH_DESCENDANTS_HYBRID;
            case "df": return AccessibilityNodeInfo.FLAG_PREFETCH_DESCENDANTS_DEPTH_FIRST;
            case "bf": return AccessibilityNodeInfo.FLAG_PREFETCH_DESCENDANTS_BREADTH_FIRST;
            case "none": return 0;
            default: return -1;
        }
    }

    public static final class Tree {
        public JSONArray windows = new JSONArray();
        public JSONArray nodes = new JSONArray();
        public int count;
        public long ms;
        public int tripCount;
        public boolean sourceIdAvailable;
        public int emptyRetries;
    }

    static Method getSourceNodeId;
    static int childFlag = 0;

    public static Tree dump(String prefetch) throws Exception {
        Tree t = new Tree();
        int t0trips = trips;
        long t0 = System.nanoTime();
        List<AccessibilityWindowInfo> wins = ua.getWindows();
        int empty = 0;
        while (wins.isEmpty() && empty < 50) {
            Thread.sleep(20);
            wins = ua.getWindows();
            empty++;
        }
        t.emptyRetries = empty;
        childFlag = 0;
        if (prefetch.endsWith("+child")) {
            prefetch = prefetch.substring(0, prefetch.length() - 6);
            childFlag = prefetchFlag(prefetch);
        }
        int flag = prefetchFlag(prefetch);
        for (AccessibilityWindowInfo w : wins) {
            AccessibilityNodeInfo root;
            if (flag >= 0 && android.os.Build.VERSION.SDK_INT >= 33) root = w.getRoot(flag);
            else root = w.getRoot();
            Rect wb = new Rect();
            w.getBoundsInScreen(wb);
            JSONObject wj = new JSONObject();
            wj.put("id", w.getId());
            wj.put("type", windowType(w.getType()));
            if (root != null && root.getPackageName() != null) wj.put("package", root.getPackageName().toString());
            wj.put("layer", w.getLayer());
            wj.put("bounds", new JSONArray(new int[] {wb.left, wb.top, wb.right, wb.bottom}));
            t.windows.put(wj);
            if (root != null) walk(root, -1, t);
        }
        t.ms = (System.nanoTime() - t0) / 1000;
        t.tripCount = trips - t0trips;
        t.count = t.nodes.length();
        return t;
    }

    static String windowType(int type) {
        switch (type) {
            case AccessibilityWindowInfo.TYPE_APPLICATION: return "application";
            case AccessibilityWindowInfo.TYPE_INPUT_METHOD: return "input_method";
            case AccessibilityWindowInfo.TYPE_SYSTEM: return "system";
            case AccessibilityWindowInfo.TYPE_ACCESSIBILITY_OVERLAY: return "accessibility_overlay";
            case AccessibilityWindowInfo.TYPE_SPLIT_SCREEN_DIVIDER: return "split_screen_divider";
            default: return "type" + type;
        }
    }

    static void walk(AccessibilityNodeInfo n, int parent, Tree t) throws Exception {
        int idx = t.nodes.length();
        t.nodes.put(toJson(n, parent, t));
        int cc = n.getChildCount();
        for (int i = 0; i < cc; i++) {
            AccessibilityNodeInfo c = (childFlag > 0 && android.os.Build.VERSION.SDK_INT >= 33) ? n.getChild(i, childFlag) : n.getChild(i);
            if (c != null) walk(c, idx, t);
        }
    }

    static JSONObject toJson(AccessibilityNodeInfo n, int parent, Tree t) throws Exception {
        JSONObject o = new JSONObject();
        o.put("p", parent);
        o.put("w", n.getWindowId());
        if (n.getViewIdResourceName() != null) o.put("id", n.getViewIdResourceName());
        if (n.getClassName() != null) o.put("cls", n.getClassName().toString());
        if (n.getText() != null && n.getText().length() > 0) o.put("text", n.getText().toString());
        if (n.getContentDescription() != null && n.getContentDescription().length() > 0) o.put("desc", n.getContentDescription().toString());
        if (n.getHintText() != null && n.getHintText().length() > 0) o.put("hint", n.getHintText().toString());
        CharSequence sd = null;
        try {
            sd = n.getStateDescription();
        } catch (Throwable ignored) {
        }
        if (sd != null && sd.length() > 0) o.put("sd", sd.toString());
        Rect b = new Rect();
        n.getBoundsInScreen(b);
        o.put("b", new JSONArray(new int[] {b.left, b.top, b.right, b.bottom}));
        StringBuilder f = new StringBuilder();
        boolean onscreen = b.right > 0 && b.bottom > 0 && b.left < screenW && b.top < screenH && b.width() > 0 && b.height() > 0;
        if (onscreen) f.append('v');
        if (n.isVisibleToUser()) f.append('V');
        if (n.isCheckable()) f.append('c');
        if (n.isChecked()) f.append('C');
        if (n.isEnabled()) f.append('e');
        if (n.isEditable()) f.append('E');
        if (n.isFocused()) f.append('F');
        if (n.isSelected()) f.append('s');
        if (n.isPassword()) f.append('P');
        if (n.isScrollable()) f.append('S');
        if (android.os.Build.VERSION.SDK_INT >= 28 && n.isHeading()) f.append('H');
        if (n.isShowingHintText()) f.append('h');
        JSONArray acts = new JSONArray();
        boolean exp = false, col = false;
        for (AccessibilityAction a : n.getActionList()) {
            int id = a.getId();
            if (id == AccessibilityNodeInfo.ACTION_CLICK) acts.put("tap");
            else if (id == AccessibilityNodeInfo.ACTION_LONG_CLICK) acts.put("long_tap");
            else if (id == AccessibilityNodeInfo.ACTION_SET_TEXT) acts.put("set_text");
            else if (id == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD) acts.put("scroll_fwd");
            else if (id == AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD) acts.put("scroll_back");
            else if (id == AccessibilityAction.ACTION_SCROLL_UP.getId()) acts.put("scroll_up");
            else if (id == AccessibilityAction.ACTION_SCROLL_DOWN.getId()) acts.put("scroll_down");
            else if (id == AccessibilityAction.ACTION_SCROLL_LEFT.getId()) acts.put("scroll_left");
            else if (id == AccessibilityAction.ACTION_SCROLL_RIGHT.getId()) acts.put("scroll_right");
            else if (id == AccessibilityNodeInfo.ACTION_EXPAND) exp = true;
            else if (id == AccessibilityNodeInfo.ACTION_COLLAPSE) col = true;
        }
        if (exp) f.append('x');
        if (col) f.append('X');
        o.put("f", f.toString());
        if (acts.length() > 0) o.put("a", acts);
        AccessibilityNodeInfo.RangeInfo r = n.getRangeInfo();
        if (r != null) {
            JSONObject rj = new JSONObject();
            rj.put("min", r.getMin());
            rj.put("max", r.getMax());
            rj.put("cur", r.getCurrent());
            rj.put("type", r.getType() == AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_INT ? "int" : r.getType() == AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_PERCENT ? "percent" : "float");
            o.put("range", rj);
        }
        AccessibilityNodeInfo.CollectionInfo ci = n.getCollectionInfo();
        if (ci != null) {
            JSONObject cj = new JSONObject();
            cj.put("rows", ci.getRowCount());
            cj.put("cols", ci.getColumnCount());
            o.put("coll", cj);
        }
        AccessibilityNodeInfo.CollectionItemInfo ii = n.getCollectionItemInfo();
        if (ii != null) {
            JSONObject ij = new JSONObject();
            ij.put("row", ii.getRowIndex());
            ij.put("col", ii.getColumnIndex());
            o.put("item", ij);
        }
        long sid = sourceId(n);
        if (sid != Long.MIN_VALUE) {
            o.put("n", Long.toString(sid));
            t.sourceIdAvailable = true;
        }
        return o;
    }

    static boolean sourceIdTried;

    static long sourceId(AccessibilityNodeInfo n) {
        if (!sourceIdTried) {
            sourceIdTried = true;
            try {
                getSourceNodeId = AccessibilityNodeInfo.class.getDeclaredMethod("getSourceNodeId");
                getSourceNodeId.setAccessible(true);
            } catch (Throwable e) {
                System.err.println("getSourceNodeId unavailable: " + e);
            }
        }
        if (getSourceNodeId == null) return Long.MIN_VALUE;
        try {
            return (Long) getSourceNodeId.invoke(n);
        } catch (Throwable e) {
            return Long.MIN_VALUE;
        }
    }

    public static void initScreen() throws Exception {
        android.view.Display d = null;
        com.genymobile.scrcpy.display.DisplayInfo di = com.genymobile.scrcpy.wrappers.ServiceManager.getDisplayManager().getDisplayInfo(0);
        screenW = di.getSize().getWidth();
        screenH = di.getSize().getHeight();
    }

    public static List<Long> percentiles(List<Long> xs) {
        List<Long> s = new ArrayList<>(xs);
        java.util.Collections.sort(s);
        List<Long> out = new ArrayList<>();
        out.add(s.get(s.size() / 2));
        out.add(s.get(Math.min(s.size() - 1, (int) Math.ceil(s.size() * 0.95) - 1)));
        return out;
    }
}
