package dev.phonedriver.agent;

import android.os.Build;
import android.os.Looper;
import com.genymobile.scrcpy.display.DisplayInfo;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.LinkedBlockingQueue;
import org.json.JSONObject;

final class Control {
    private static final int MAX_REQUEST_BYTES = 16 << 20;
    private static final byte[] CLOSE = new byte[0];

    private final DataInputStream in;
    private final DataOutputStream out;
    private final LinkedBlockingQueue<byte[]> outbox = new LinkedBlockingQueue<>();
    private final ExecutorService requests = Executors.newSingleThreadExecutor(Control::looperThread);
    private final ExecutorService gestures = Executors.newSingleThreadExecutor(Control::looperThread);
    private final ExecutorService shells = Executors.newCachedThreadPool();

    Control(DataInputStream in, DataOutputStream out) {
        this.in = in;
        this.out = out;
    }

    private static Thread looperThread(Runnable r) {
        return new Thread(() -> {
            Looper.prepare();
            r.run();
        });
    }

    // A screen tree is 20 KB or more of JSON, which is 240 ms of a 780 kbit/s link. A frame this long goes
    // deflated, with the top bit of its length set.
    private static final int DEFLATE_FROM = 1024;
    private static final int DEFLATED = 0x80000000;

    private static byte[] deflate(byte[] frame) {
        java.util.zip.Deflater d = new java.util.zip.Deflater(java.util.zip.Deflater.BEST_SPEED);
        d.setInput(frame);
        d.finish();
        java.io.ByteArrayOutputStream small = new java.io.ByteArrayOutputStream(frame.length / 4 + 64);
        byte[] chunk = new byte[8192];
        while (!d.finished()) {
            small.write(chunk, 0, d.deflate(chunk));
        }
        d.end();
        return small.toByteArray();
    }

    void event(JSONObject event) {
        outbox.add(Json.bytes(event));
    }

    private void writer() {
        try {
            while (true) {
                byte[] frame = outbox.take();
                if (frame == CLOSE) {
                    return;
                }
                if (frame.length >= DEFLATE_FROM) {
                    byte[] small = deflate(frame);
                    out.writeInt(small.length | DEFLATED);
                    out.write(small);
                } else {
                    out.writeInt(frame.length);
                    out.write(frame);
                }
                out.flush();
            }
        } catch (IOException | InterruptedException e) {
            System.err.println("control writer ended: " + e);
        }
    }

    void run(JSONObject hello) throws IOException {
        Thread writer = new Thread(this::writer);
        writer.start();
        try {
            JSONObject reply = Json.put(new JSONObject(), "id", hello.optInt("id"));
            if (!Main.version.equals(hello.optString("version"))) {
                Json.put(reply, "ok", false);
                Json.put(reply, "error", "agent is version " + Main.version + ", host is " + hello.optString("version"));
                event(reply);
                return;
            }
            Main.control = this;
            event(helloReply(reply));
            while (true) {
                int n = in.readInt();
                if (n < 0 || n > MAX_REQUEST_BYTES) {
                    throw new IOException("request length " + n);
                }
                byte[] frame = new byte[n];
                in.readFully(frame);
                JSONObject q = Json.parse(frame);
                String op = q.optString("op");
                ExecutorService target = "gesture".equals(op) ? gestures : "shell".equals(op) ? shells : requests;
                target.execute(() -> event(answer(q)));
            }
        } finally {
            if (Main.control == this) {
                Main.control = null;
            }
            requests.shutdownNow();
            gestures.shutdownNow();
            shells.shutdownNow();
            outbox.add(CLOSE);
        }
    }

    private static JSONObject helloReply(JSONObject reply) {
        DisplayInfo di = ServiceManager.getDisplayManager().getDisplayInfo(Hidden.displayId());
        JSONObject display = new JSONObject();
        Json.put(display, "w", di.getSize().getWidth());
        Json.put(display, "h", di.getSize().getHeight());
        Json.put(display, "rotation", di.getRotation());
        Json.put(reply, "ok", true);
        Json.put(reply, "api", Build.VERSION.SDK_INT);
        Json.put(reply, "model", Build.MODEL);
        Json.put(reply, "density", di.getDpi() / 160.0);
        Json.put(reply, "display", display);
        if (Hidden.active()) {
            Json.put(reply, "hidden_display", Hidden.displayId());
        }
        return reply;
    }

    private static JSONObject answer(JSONObject q) {
        JSONObject reply = Json.put(new JSONObject(), "id", q.optInt("id"));
        try {
            JSONObject result = Ops.run(q);
            Json.put(reply, "ok", true);
            for (java.util.Iterator<String> keys = result.keys(); keys.hasNext(); ) {
                String k = keys.next();
                Json.put(reply, k, result.opt(k));
            }
        } catch (Throwable t) {
            Json.put(reply, "ok", false);
            Json.put(reply, "error", t.toString());
        }
        return reply;
    }
}
