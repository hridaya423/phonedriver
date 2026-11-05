package dev.phoneuse.agent;

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
                out.writeInt(frame.length);
                out.write(frame);
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
                ExecutorService target = "gesture".equals(q.optString("op")) ? gestures : requests;
                target.execute(() -> event(answer(q)));
            }
        } finally {
            if (Main.control == this) {
                Main.control = null;
            }
            requests.shutdownNow();
            gestures.shutdownNow();
            outbox.add(CLOSE);
        }
    }

    private static JSONObject helloReply(JSONObject reply) {
        DisplayInfo di = ServiceManager.getDisplayManager().getDisplayInfo(0);
        JSONObject display = new JSONObject();
        Json.put(display, "w", di.getSize().getWidth());
        Json.put(display, "h", di.getSize().getHeight());
        Json.put(display, "rotation", di.getRotation());
        Json.put(reply, "ok", true);
        Json.put(reply, "api", Build.VERSION.SDK_INT);
        Json.put(reply, "model", Build.MODEL);
        Json.put(reply, "density", di.getDpi() / 160.0);
        Json.put(reply, "display", display);
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
