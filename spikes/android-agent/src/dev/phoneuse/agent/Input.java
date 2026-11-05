package dev.phoneuse.agent;

import android.os.SystemClock;
import android.view.InputDevice;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.MotionEvent;
import com.genymobile.scrcpy.wrappers.InputManager;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;
import org.json.JSONArray;
import org.json.JSONObject;

final class Input {
    static boolean inject(android.view.InputEvent e, int mode) {
        InputManager.setDisplayId(e, 0);
        return ServiceManager.getInputManager().injectInputEvent(e, mode);
    }

    static void key(int keycode, String action) {
        long now = SystemClock.uptimeMillis();
        int mode = InputManager.INJECT_INPUT_EVENT_MODE_WAIT_FOR_FINISH;
        if (!"up".equals(action)) inject(new KeyEvent(now, now, KeyEvent.ACTION_DOWN, keycode, 0), "down".equals(action) ? mode : InputManager.INJECT_INPUT_EVENT_MODE_ASYNC);
        if (!"down".equals(action)) inject(new KeyEvent(now, now, KeyEvent.ACTION_UP, keycode, 0), mode);
    }

    static final class P {
        int id;
        int[] t, x, y;
        boolean down;
    }

    static void gesture(JSONArray pointers) throws Exception {
        List<P> ps = new ArrayList<>();
        TreeSet<Integer> times = new TreeSet<>();
        for (int i = 0; i < pointers.length(); i++) {
            JSONObject o = pointers.getJSONObject(i);
            JSONArray path = o.getJSONArray("path");
            P p = new P();
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
            ps.add(p);
        }
        List<P> active = new ArrayList<>();
        int[] cx = new int[ps.size()], cy = new int[ps.size()];
        long start = SystemClock.uptimeMillis();
        long downTime = start;
        boolean first = true;
        Integer last = times.last();
        for (int T : times) {
            long due = start + T;
            long wait = due - SystemClock.uptimeMillis();
            if (wait > 0) SystemClock.sleep(wait);
            for (P p : ps) {
                int idx = ps.indexOf(p);
                if (p.t[0] == T) {
                    cx[idx] = p.x[0];
                    cy[idx] = p.y[0];
                    active.add(p);
                    int action = active.size() == 1 ? MotionEvent.ACTION_DOWN : (MotionEvent.ACTION_POINTER_DOWN | ((active.size() - 1) << 8));
                    if (first) {
                        downTime = SystemClock.uptimeMillis();
                        first = false;
                    }
                    send(action, active, ps, cx, cy, downTime, false);
                }
            }
            boolean moved = false;
            for (P p : ps) {
                int idx = ps.indexOf(p);
                for (int k = 1; k < p.t.length - 1 + (p.t.length == 1 ? 1 : 0); k++) {
                    if (p.t[k] == T) {
                        cx[idx] = p.x[k];
                        cy[idx] = p.y[k];
                        moved = true;
                    }
                }
            }
            if (moved) send(MotionEvent.ACTION_MOVE, active, ps, cx, cy, downTime, false);
            for (P p : ps) {
                int idx = ps.indexOf(p);
                int n = p.t.length - 1;
                if (n > 0 && p.t[n] == T) {
                    cx[idx] = p.x[n];
                    cy[idx] = p.y[n];
                }
                if (p.t[n] == T && active.contains(p)) {
                    int ai = active.indexOf(p);
                    int action = active.size() == 1 ? MotionEvent.ACTION_UP : (MotionEvent.ACTION_POINTER_UP | (ai << 8));
                    boolean lastEvent = active.size() == 1 && T == last;
                    send(action, active, ps, cx, cy, downTime, lastEvent);
                    active.remove(p);
                }
            }
        }
    }

    static void send(int action, List<P> active, List<P> all, int[] cx, int[] cy, long downTime, boolean finish) {
        int n = active.size();
        MotionEvent.PointerProperties[] props = new MotionEvent.PointerProperties[n];
        MotionEvent.PointerCoords[] coords = new MotionEvent.PointerCoords[n];
        for (int i = 0; i < n; i++) {
            P p = active.get(i);
            props[i] = new MotionEvent.PointerProperties();
            props[i].id = p.id;
            props[i].toolType = MotionEvent.TOOL_TYPE_FINGER;
            coords[i] = new MotionEvent.PointerCoords();
            int idx = all.indexOf(p);
            coords[i].x = cx[idx];
            coords[i].y = cy[idx];
            coords[i].pressure = action == MotionEvent.ACTION_UP ? 0f : 1f;
            coords[i].size = 1f;
        }
        MotionEvent ev = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), action, n, props, coords, 0, 0, 1f, 1f, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0);
        inject(ev, finish ? InputManager.INJECT_INPUT_EVENT_MODE_WAIT_FOR_FINISH : InputManager.INJECT_INPUT_EVENT_MODE_ASYNC);
        ev.recycle();
    }

    static String keys(String text) {
        KeyCharacterMap kcm = KeyCharacterMap.load(KeyCharacterMap.VIRTUAL_KEYBOARD);
        List<KeyEvent[]> all = new ArrayList<>();
        for (int i = 0; i < text.length(); ) {
            int cp = text.codePointAt(i);
            String ch = new String(Character.toChars(cp));
            KeyEvent[] evs = kcm.getEvents(ch.toCharArray());
            if (evs == null) return "unmappable character U+" + Integer.toHexString(cp).toUpperCase() + " at " + i;
            all.add(evs);
            i += Character.charCount(cp);
        }
        for (KeyEvent[] evs : all) {
            for (int k = 0; k < evs.length; k++) {
                inject(evs[k], k == evs.length - 1 ? InputManager.INJECT_INPUT_EVENT_MODE_WAIT_FOR_FINISH : InputManager.INJECT_INPUT_EVENT_MODE_ASYNC);
            }
        }
        return null;
    }
}
