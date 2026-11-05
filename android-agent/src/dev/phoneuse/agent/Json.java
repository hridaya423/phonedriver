package dev.phoneuse.agent;

import org.json.JSONException;
import org.json.JSONObject;

final class Json {
    static JSONObject put(JSONObject o, String key, Object value) {
        try {
            return o.put(key, value);
        } catch (JSONException e) {
            throw new IllegalStateException(e);
        }
    }

    static JSONObject parse(byte[] utf8) {
        try {
            return new JSONObject(new String(utf8, "UTF-8"));
        } catch (JSONException | java.io.UnsupportedEncodingException e) {
            throw new IllegalArgumentException("bad json frame: " + e.getMessage());
        }
    }

    static byte[] bytes(JSONObject o) {
        try {
            return o.toString().getBytes("UTF-8");
        } catch (java.io.UnsupportedEncodingException e) {
            throw new IllegalStateException(e);
        }
    }

    private Json() {}
}
