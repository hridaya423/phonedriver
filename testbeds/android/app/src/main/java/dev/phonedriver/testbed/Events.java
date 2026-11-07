package dev.phonedriver.testbed;

import android.util.Log;
import org.json.JSONException;
import org.json.JSONObject;

final class Events {
    static final String TAG = "PUTB";

    static JSONObject event(String name) {
        JSONObject o = new JSONObject();
        try {
            o.put("ev", name);
        } catch (JSONException e) {
            throw new IllegalStateException(e);
        }
        return o;
    }

    static JSONObject put(JSONObject o, String key, Object value) {
        try {
            o.put(key, value);
        } catch (JSONException e) {
            throw new IllegalStateException(e);
        }
        return o;
    }

    static void emit(JSONObject o) {
        Log.i(TAG, o.toString());
    }

    static void emitRaw(String json) {
        Log.i(TAG, json);
    }

    private Events() {}
}
