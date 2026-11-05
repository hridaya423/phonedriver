package dev.phoneuse.spike;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.app.UiAutomation;
import android.os.Looper;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import com.genymobile.scrcpy.Workarounds;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import org.json.JSONArray;
import org.json.JSONObject;

public final class Main {
    public static void main(String[] args) {
        try {
            Looper.prepareMainLooper();
            run(args);
        } catch (Throwable t) {
            t.printStackTrace();
            System.exit(1);
        }
    }

    static void run(String[] args) throws Exception {
        String cmd = args.length > 0 ? args[0] : "tree";
        if (cmd.equals("echo")) {
            echo(args[1]);
            return;
        }
        int flags = UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES;
        if (System.getenv("SUPPRESS") != null) flags = 0;
        int eventTypes = "events".equals(cmd) && args.length > 2 && args[2].equals("all")
                ? AccessibilityEvent.TYPES_ALL_MASK
                : AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED | AccessibilityEvent.TYPE_WINDOWS_CHANGED;
        Workarounds.apply();
        try {
            Ui.initScreen();
        } catch (Throwable t) {
            System.err.println("initScreen failed " + t);
        }
        Ui.connect(flags, eventTypes);
        System.err.println("connected ctor=" + Ui.ctorUsed + " connect_ms=" + Ui.connectMs + " flags=" + flags + " screen=" + Ui.screenW + "x" + Ui.screenH);
        try {
            switch (cmd) {
                case "tree": {
                    Ui.Tree t = Ui.dump(args.length > 1 ? args[1] : "default");
                    JSONObject o = new JSONObject();
                    o.put("windows", t.windows);
                    o.put("nodes", t.nodes);
                    System.out.println(o.toString(1));
                    System.err.println("nodes=" + t.count + " ms=" + t.ms / 1000.0 + " trips=" + t.tripCount + " sourceId=" + t.sourceIdAvailable + " emptyRetries=" + t.emptyRetries);
                    break;
                }
                case "bench": bench(Integer.parseInt(args[1]), args[2], args.length > 3 && args[3].equals("warm")); break;
                case "events": events(Integer.parseInt(args[1])); break;
                case "recycle": recycle(); break;
                case "hold": Thread.sleep(Long.parseLong(args[1]) * 1000); break;
                case "scroll": scroll(Integer.parseInt(args[1])); break;
                default: System.err.println("unknown " + cmd);
            }
        } finally {
            Ui.disconnect();
        }
        System.exit(0);
    }

    static void echo(String name) throws Exception {
        final android.net.LocalServerSocket ss = new android.net.LocalServerSocket(name);
        System.err.println("echo listening on @" + name);
        while (true) {
            final android.net.LocalSocket c = ss.accept();
            new Thread(new Runnable() {
                @Override
                public void run() {
                    try {
                        java.io.InputStream in = c.getInputStream();
                        java.io.OutputStream out = c.getOutputStream();
                        byte[] b = new byte[65536];
                        int n;
                        while ((n = in.read(b)) > 0) {
                            out.write(b, 0, n);
                            out.flush();
                        }
                    } catch (Exception ignored) {
                    }
                }
            }).start();
        }
    }

    static void bench(int n, String mode, boolean warm) throws Exception {
        List<Long> us = new ArrayList<>();
        int nodes = 0, trips = 0, bytes = 0;
        for (int i = 0; i < n + 1; i++) {
            if (!warm) Ui.clearCache();
            Ui.Tree t = Ui.dump(mode);
            if (i == 0) continue;
            us.add(t.ms);
            nodes = t.count;
            trips = t.tripCount;
            JSONObject o = new JSONObject();
            o.put("windows", t.windows);
            o.put("nodes", t.nodes);
            bytes = o.toString().getBytes("UTF-8").length;
        }
        List<Long> p = Ui.percentiles(us);
        System.out.println("{\"mode\":\"" + mode + "\",\"warm\":" + warm + ",\"runs\":" + n + ",\"p50_ms\":" + p.get(0) / 1000.0 + ",\"p95_ms\":" + p.get(1) / 1000.0
                + ",\"nodes\":" + nodes + ",\"trips\":" + trips + ",\"json_bytes\":" + bytes + ",\"all_us\":" + us + "}");
    }

    static void events(int secs) throws Exception {
        final Map<String, Integer> counts = new TreeMap<>();
        final long t0 = System.nanoTime();
        Ui.ua.setOnAccessibilityEventListener(new UiAutomation.OnAccessibilityEventListener() {
            @Override
            public void onAccessibilityEvent(AccessibilityEvent e) {
                String k = AccessibilityEvent.eventTypeToString(e.getEventType());
                synchronized (counts) {
                    counts.merge(k, 1, Integer::sum);
                }
                System.out.println("ev t=" + (System.nanoTime() - t0) / 1_000_000 + "ms " + k + " pkg=" + e.getPackageName() + " win=" + e.getWindowId());
            }
        });
        Thread.sleep(secs * 1000L);
        synchronized (counts) {
            System.out.println("counts " + counts);
        }
    }

    static AccessibilityNodeInfo findScrollable(AccessibilityNodeInfo n) {
        if (n.isScrollable() && n.getCollectionInfo() != null) return n;
        for (int i = 0; i < n.getChildCount(); i++) {
            AccessibilityNodeInfo c = n.getChild(i);
            if (c == null) continue;
            AccessibilityNodeInfo r = findScrollable(c);
            if (r != null) return r;
        }
        return null;
    }

    static void scroll(int times) throws Exception {
        AccessibilityNodeInfo root = Ui.ua.getRootInActiveWindow();
        AccessibilityNodeInfo s = findScrollable(root);
        for (int i = 0; i < times; i++) {
            s.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            Thread.sleep(600);
        }
        System.out.println("scrolled " + times);
    }

    static Map<String, String> byId(JSONArray nodes) throws Exception {
        Map<String, String> m = new TreeMap<>();
        for (int i = 0; i < nodes.length(); i++) {
            JSONObject o = nodes.getJSONObject(i);
            if (!o.has("n") || !"android.widget.TextView".equals(o.optString("cls")) || !o.has("text")) continue;
            m.put(o.getString("n"), o.getString("text"));
        }
        return m;
    }

    static void recycle() throws Exception {
        Ui.Tree before = Ui.dump("default");
        AccessibilityNodeInfo root = Ui.ua.getRootInActiveWindow();
        AccessibilityNodeInfo s = findScrollable(root);
        if (s == null) {
            System.out.println("no scrollable collection");
            return;
        }
        int rows = s.getCollectionInfo().getRowCount();
        Map<String, String> a = byId(before.nodes);
        int scrolls = 0;
        int same = 0, changed = 0;
        Map<String, String> last = a;
        for (int k = 0; k < 4; k++) {
            s.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            scrolls++;
            Thread.sleep(800);
            Ui.clearCache();
            Ui.Tree after = Ui.dump("default");
            Map<String, String> b = byId(after.nodes);
            int sm = 0, ch = 0;
            StringBuilder ex = new StringBuilder();
            for (Map.Entry<String, String> e : b.entrySet()) {
                String old = last.get(e.getKey());
                if (old == null) continue;
                if (old.equals(e.getValue())) sm++;
                else {
                    ch++;
                    if (ex.length() < 300) ex.append(e.getKey()).append(": \"").append(old).append("\" -> \"").append(e.getValue()).append("\"; ");
                }
            }
            System.out.println("scroll " + scrolls + ": text nodes " + b.size() + ", ids seen before with same text " + sm + ", ids seen before with NEW text " + ch + " e.g. " + ex);
            last = b;
            s = findScrollable(Ui.ua.getRootInActiveWindow());
            if (s == null) break;
        }
        System.out.println("collection rows=" + rows);
    }
}
