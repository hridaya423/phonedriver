package dev.phoneuse.agent;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.app.UiAutomation;
import android.content.ClipData;
import android.content.ClipDescription;
import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.os.Build;
import android.os.Bundle;
import android.os.Looper;
import android.os.PersistableBundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import com.genymobile.scrcpy.Workarounds;
import com.genymobile.scrcpy.display.DisplayInfo;
import com.genymobile.scrcpy.wrappers.ClipboardManager;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import dev.phoneuse.spike.Ui;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.concurrent.atomic.AtomicInteger;
import org.json.JSONArray;
import org.json.JSONObject;

public final class Main {
    static String token;
    static final AtomicInteger streams = new AtomicInteger();
    static volatile int everConnected;

    public static void main(String[] a) {
        try {
            Looper.prepareMainLooper();
            run(a);
        } catch (Throwable t) {
            t.printStackTrace();
            System.exit(1);
        }
    }

    static String readLine(InputStream in) throws IOException {
        StringBuilder sb = new StringBuilder();
        int c;
        while ((c = in.read()) >= 0 && c != '\n') sb.append((char) c);
        return sb.toString();
    }

    static void run(String[] a) throws Exception {
        String name = a[1];
        if (a.length > 2 && a[2].equals("notoken")) token = "";
        else token = readLine(System.in);
        System.err.println("agent version " + a[0] + " token length " + token.length());
        Workarounds.apply();
        Ui.initScreen();
        Ui.connect(UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES, AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED | AccessibilityEvent.TYPE_WINDOWS_CHANGED);
        final LocalServerSocket ss = new LocalServerSocket(name);
        System.err.println("agent listening on @" + name + " pid " + android.os.Process.myPid());
        new Thread(new Runnable() {
            @Override
            public void run() {
                SystemClock.sleep(10_000);
                if (everConnected == 0) {
                    System.err.println("nothing connected within 10 s, exiting");
                    System.exit(3);
                }
            }
        }).start();
        while (true) {
            final LocalSocket s = ss.accept();
            new Thread(new Runnable() {
                @Override
                public void run() {
                    try {
                        handle(s);
                    } catch (Throwable t) {
                        System.err.println("stream ended: " + t);
                    } finally {
                        try {
                            s.close();
                        } catch (IOException ignored) {
                        }
                        if (streams.decrementAndGet() <= 0 && everConnected > 0) {
                            System.err.println("all streams closed, exiting");
                            System.exit(0);
                        }
                    }
                }
            }).start();
        }
    }

    static byte[] readFrame(DataInputStream in) throws IOException {
        int n = in.readInt();
        byte[] b = new byte[n];
        in.readFully(b);
        return b;
    }

    static void handle(LocalSocket s) throws Exception {
        if (Looper.myLooper() == null) Looper.prepare();
        DataInputStream in = new DataInputStream(s.getInputStream());
        DataOutputStream out = new DataOutputStream(new java.io.BufferedOutputStream(s.getOutputStream(), 1 << 16));
        int kind = in.read();
        streams.incrementAndGet();
        if (kind == 'C') {
            byte[] f = readFrame(in);
            JSONObject hello = new JSONObject(new String(f, "UTF-8"));
            if (!token.equals(hello.optString("token"))) {
                System.err.println("control: bad token, dropping");
                return;
            }
            everConnected++;
            control(in, out, hello);
        } else if (kind == 'V') {
            byte[] f = readFrame(in);
            JSONObject o = new JSONObject(new String(f, "UTF-8"));
            if (!token.equals(o.optString("token"))) {
                System.err.println("video: bad token, dropping");
                return;
            }
            everConnected++;
            Video v = new Video(out);
            if (o.has("max_size")) v.maxSize = o.getInt("max_size");
            v.run();
        } else {
            System.err.println("unknown stream kind " + kind);
        }
    }

    static void reply(DataOutputStream out, JSONObject o) throws IOException {
        byte[] b = o.toString().getBytes("UTF-8");
        synchronized (out) {
            out.writeInt(b.length);
            out.write(b);
            out.flush();
        }
    }

    static void control(DataInputStream in, DataOutputStream out, JSONObject hello) throws Exception {
        DisplayInfo di = ServiceManager.getDisplayManager().getDisplayInfo(0);
        JSONObject r = new JSONObject();
        r.put("id", hello.optInt("id"));
        r.put("ok", true);
        r.put("api", Build.VERSION.SDK_INT);
        r.put("model", Build.MODEL);
        JSONObject d = new JSONObject();
        d.put("w", di.getSize().getWidth());
        d.put("h", di.getSize().getHeight());
        d.put("rotation", di.getRotation());
        r.put("display", d);
        reply(out, r);
        while (true) {
            JSONObject q = new JSONObject(new String(readFrame(in), "UTF-8"));
            JSONObject res = new JSONObject();
            res.put("id", q.optInt("id"));
            try {
                res.put("ok", true);
                op(q, res);
            } catch (Throwable t) {
                res.put("ok", false);
                res.put("error", t.toString());
            }
            reply(out, res);
        }
    }

    static AccessibilityNodeInfo focusedInput() {
        return Ui.ua.findFocus(AccessibilityNodeInfo.FOCUS_INPUT);
    }

    static String nodeText() {
        AccessibilityNodeInfo n = focusedInput();
        if (n == null) return null;
        CharSequence t = n.getText();
        return t == null ? "" : t.toString();
    }

    static void op(JSONObject q, JSONObject res) throws Exception {
        String op = q.getString("op");
        switch (op) {
            case "tree": {
                if (!q.optBoolean("keep_cache")) Ui.clearCache();
                Ui.Tree t = Ui.dump(q.optString("prefetch", "default"));
                res.put("windows", t.windows);
                res.put("nodes", t.nodes);
                break;
            }
            case "gesture": {
                long g0 = System.nanoTime();
                Input.gesture(q.getJSONArray("pointers"));
                res.put("agent_us", (System.nanoTime() - g0) / 1000);
                break;
            }
            case "key": Input.key(q.getInt("keycode"), q.optString("action", "press")); break;
            case "sync_frame": Video.syncRequested = true; break;
            case "wake": Input.key(224, "press"); break;
            case "clipboard_set": {
                ClipData c = ClipData.newPlainText(null, q.getString("text"));
                if (q.optBoolean("sensitive") && Build.VERSION.SDK_INT >= 33) {
                    PersistableBundle pb = new PersistableBundle();
                    pb.putBoolean("android.content.extra.IS_SENSITIVE", true);
                    c.getDescription().setExtras(pb);
                }
                ServiceManager.getClipboardManager().setClip(c);
                break;
            }
            case "clipboard_set_uri": {
                ClipData c = ClipData.newRawUri("image", android.net.Uri.parse(q.getString("uri")));
                ServiceManager.getClipboardManager().setClip(c);
                break;
            }
            case "clipboard_get": {
                CharSequence t = ServiceManager.getClipboardManager().getText();
                res.put("text", t == null ? JSONObject.NULL : t.toString());
                ClipData c = ServiceManager.getClipboardManager().getClip();
                res.put("desc", c == null ? "null" : c.getDescription().toString() + " item0uri=" + c.getItemAt(0).getUri());
                break;
            }
            case "text": text(q, res); break;
            case "state": {
                res.put("text_in_focus", nodeText() == null ? JSONObject.NULL : nodeText());
                break;
            }
            default: throw new IllegalArgumentException("unknown op " + op);
        }
    }

    static void text(JSONObject q, JSONObject res) throws Exception {
        String mode = q.getString("mode");
        String text = q.getString("text");
        long t0 = System.nanoTime();
        if (mode.equals("paste")) {
            ClipboardManager cm = ServiceManager.getClipboardManager();
            ClipData saved = cm.getClip();
            ClipData c = ClipData.newPlainText(null, text);
            if (q.optBoolean("sensitive") && Build.VERSION.SDK_INT >= 33) {
                PersistableBundle pb = new PersistableBundle();
                pb.putBoolean("android.content.extra.IS_SENSITIVE", true);
                c.getDescription().setExtras(pb);
            }
            cm.setClip(c);
            Input.key(279, "press");
            res.put("inject_ms", (System.nanoTime() - t0) / 1_000_000.0);
            int restoreMs = q.optInt("restore_ms", 0);
            if (restoreMs > 0) SystemClock.sleep(restoreMs);
            if (saved != null && restoreMs >= 0) cm.setClip(saved);
            else if (saved == null && restoreMs >= 0) {
                cm.setClip(ClipData.newPlainText(null, ""));
            }
        } else if (mode.equals("set")) {
            AccessibilityNodeInfo n = focusedInput();
            if (n == null) throw new IllegalStateException("no focused input node");
            Bundle b = new Bundle();
            b.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text);
            res.put("performed", n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, b));
        } else if (mode.equals("keys")) {
            String err = Input.keys(text);
            if (err != null) {
                res.put("ok", false);
                res.put("error", err);
            }
        }
        res.put("elapsed_ms", (System.nanoTime() - t0) / 1_000_000.0);
        if (q.optBoolean("readback")) {
            SystemClock.sleep(q.optInt("readback_delay_ms", 150));
            String t = nodeText();
            res.put("readback", t == null ? JSONObject.NULL : t);
        }
    }
}
