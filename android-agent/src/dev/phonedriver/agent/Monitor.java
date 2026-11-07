package dev.phonedriver.agent;

import android.os.SystemClock;
import com.genymobile.scrcpy.display.DisplayInfo;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import org.json.JSONObject;

final class Monitor {
    private static final long POLL_MS = 100;
    private static final long KEYGUARD_POLL_MS = 500;

    static void start() {
        Thread t = new Thread(Monitor::loop, "monitor");
        t.setDaemon(true);
        t.start();
    }

    private static void emit(JSONObject event) {
        Control c = Main.control;
        if (c != null) {
            c.event(event);
        }
    }

    private static void loop() {
        DisplayInfo last = null;
        Boolean locked = null;
        long keyguardAt = 0;
        while (true) {
            try {
                DisplayInfo now = ServiceManager.getDisplayManager().getDisplayInfo(0);
                if (last != null && (!now.getSize().equals(last.getSize()) || now.getRotation() != last.getRotation()
                        || now.getDpi() != last.getDpi())) {
                    JSONObject e = Json.put(new JSONObject(), "ev", "display_changed");
                    Json.put(e, "w", now.getSize().getWidth());
                    Json.put(e, "h", now.getSize().getHeight());
                    Json.put(e, "rotation", now.getRotation());
                    Json.put(e, "density", now.getDpi() / 160.0);
                    emit(e);
                }
                last = now;
                if (SystemClock.uptimeMillis() - keyguardAt >= KEYGUARD_POLL_MS) {
                    keyguardAt = SystemClock.uptimeMillis();
                    boolean l = Keyguard.locked();
                    if (locked != null && l != locked) {
                        emit(Json.put(Json.put(new JSONObject(), "ev", "state"), "locked", l));
                    }
                    locked = l;
                }
            } catch (Throwable t) {
                System.err.println("monitor: " + t);
                SystemClock.sleep(KEYGUARD_POLL_MS);
            }
            SystemClock.sleep(POLL_MS);
        }
    }

    private Monitor() {}
}
