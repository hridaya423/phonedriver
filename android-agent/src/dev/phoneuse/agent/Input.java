package dev.phoneuse.agent;

import android.os.SystemClock;
import android.view.InputDevice;
import android.view.InputEvent;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.MotionEvent;
import com.genymobile.scrcpy.wrappers.InputManager;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

final class Input {
    private static final int ASYNC = InputManager.INJECT_INPUT_EVENT_MODE_ASYNC;
    private static final int FINISH = InputManager.INJECT_INPUT_EVENT_MODE_WAIT_FOR_FINISH;

    private static void inject(InputEvent e, int mode) {
        InputManager.setDisplayId(e, 0);
        if (!ServiceManager.getInputManager().injectInputEvent(e, mode)) {
            throw new IllegalStateException("the system rejected an input event");
        }
    }

    static void key(int keycode, String action) {
        long now = SystemClock.uptimeMillis();
        if (!"up".equals(action)) {
            inject(new KeyEvent(now, now, KeyEvent.ACTION_DOWN, keycode, 0), "down".equals(action) ? FINISH : ASYNC);
        }
        if (!"down".equals(action)) {
            inject(new KeyEvent(now, now, KeyEvent.ACTION_UP, keycode, 0), FINISH);
        }
    }

    static void expandNotifications() {
        ServiceManager.getStatusBarManager().expandNotificationsPanel();
    }

    private static final class Pointer {
        int id;
        int[] t;
        int[] x;
        int[] y;
        int curX;
        int curY;
    }

    private static Pointer[] parse(JSONArray pointers, TreeSet<Integer> times) throws JSONException {
        Pointer[] ps = new Pointer[pointers.length()];
        for (int i = 0; i < ps.length; i++) {
            JSONObject o = pointers.getJSONObject(i);
            JSONArray path = o.getJSONArray("path");
            Pointer p = new Pointer();
            p.id = o.optInt("id", i);
            p.t = new int[path.length()];
            p.x = new int[path.length()];
            p.y = new int[path.length()];
            for (int k = 0; k < path.length(); k++) {
                JSONObject pt = path.getJSONObject(k);
                p.t[k] = pt.getInt("t_ms");
                p.x[k] = pt.getInt("x");
                p.y[k] = pt.getInt("y");
                times.add(p.t[k]);
            }
            ps[i] = p;
        }
        return ps;
    }

    static void gesture(JSONArray pointers) throws JSONException {
        TreeSet<Integer> times = new TreeSet<>();
        Pointer[] ps = parse(pointers, times);
        List<Pointer> active = new ArrayList<>();
        long start = SystemClock.uptimeMillis();
        long downTime = 0;
        int lastT = times.last();
        for (int t : times) {
            long wait = start + t - SystemClock.uptimeMillis();
            if (wait > 0) {
                SystemClock.sleep(wait);
            }
            for (Pointer p : ps) {
                if (p.t[0] != t) {
                    continue;
                }
                p.curX = p.x[0];
                p.curY = p.y[0];
                active.add(p);
                if (active.size() == 1) {
                    downTime = SystemClock.uptimeMillis();
                    send(MotionEvent.ACTION_DOWN, active, downTime, false);
                } else {
                    send(MotionEvent.ACTION_POINTER_DOWN | ((active.size() - 1) << 8), active, downTime, false);
                }
            }
            boolean moved = false;
            for (Pointer p : active) {
                for (int k = 1; k < p.t.length - 1; k++) {
                    if (p.t[k] == t) {
                        p.curX = p.x[k];
                        p.curY = p.y[k];
                        moved = true;
                    }
                }
            }
            if (moved) {
                send(MotionEvent.ACTION_MOVE, active, downTime, false);
            }
            for (Pointer p : ps) {
                int last = p.t.length - 1;
                if (p.t[last] != t || !active.contains(p)) {
                    continue;
                }
                if (last > 0) {
                    p.curX = p.x[last];
                    p.curY = p.y[last];
                }
                boolean finish = active.size() == 1 && t == lastT;
                int action = active.size() == 1
                        ? MotionEvent.ACTION_UP
                        : MotionEvent.ACTION_POINTER_UP | (active.indexOf(p) << 8);
                send(action, active, downTime, finish);
                active.remove(p);
            }
        }
    }

    private static void send(int action, List<Pointer> active, long downTime, boolean finish) {
        int n = active.size();
        MotionEvent.PointerProperties[] props = new MotionEvent.PointerProperties[n];
        MotionEvent.PointerCoords[] coords = new MotionEvent.PointerCoords[n];
        for (int i = 0; i < n; i++) {
            Pointer p = active.get(i);
            props[i] = new MotionEvent.PointerProperties();
            props[i].id = p.id;
            props[i].toolType = MotionEvent.TOOL_TYPE_FINGER;
            coords[i] = new MotionEvent.PointerCoords();
            coords[i].x = p.curX;
            coords[i].y = p.curY;
            coords[i].pressure = action == MotionEvent.ACTION_UP ? 0f : 1f;
            coords[i].size = 1f;
        }
        MotionEvent ev = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), action, n, props, coords,
                0, 0, 1f, 1f, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0);
        try {
            inject(ev, finish ? FINISH : ASYNC);
        } finally {
            ev.recycle();
        }
    }

    static void keys(String text) {
        KeyCharacterMap map = KeyCharacterMap.load(KeyCharacterMap.VIRTUAL_KEYBOARD);
        List<KeyEvent[]> all = new ArrayList<>();
        for (int i = 0; i < text.length(); ) {
            int cp = text.codePointAt(i);
            KeyEvent[] events = map.getEvents(Character.toChars(cp));
            if (events == null) {
                throw new IllegalArgumentException("unmappable character U+" + Integer.toHexString(cp).toUpperCase() + " at " + i);
            }
            all.add(events);
            i += Character.charCount(cp);
        }
        for (KeyEvent[] events : all) {
            for (int k = 0; k < events.length; k++) {
                inject(events[k], k == events.length - 1 ? FINISH : ASYNC);
            }
        }
    }

    private Input() {}
}
