package dev.phoneuse.testbed;

import android.Manifest;
import android.animation.ValueAnimator;
import android.annotation.SuppressLint;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.Context;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
import android.util.TypedValue;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.JavascriptInterface;
import android.webkit.WebView;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.Switch;
import android.widget.TextView;
import androidx.recyclerview.widget.LinearLayoutManager;
import androidx.recyclerview.widget.RecyclerView;
import org.json.JSONArray;
import org.json.JSONObject;

final class Screens {
    private static int id(Context c, String name) {
        return c.getResources().getIdentifier(name, "id", c.getPackageName());
    }

    private static float dp(View v, float px) {
        return px / v.getResources().getDisplayMetrics().density;
    }

    private static LinearLayout column(Context c) {
        LinearLayout l = new LinearLayout(c);
        l.setOrientation(LinearLayout.VERTICAL);
        return l;
    }

    @SuppressLint("ClickableViewAccessibility")
    static View taps(Activity a) {
        LinearLayout root = column(a);
        Switch toggle = new Switch(a);
        toggle.setId(id(a, "toggle"));
        toggle.setText("Dark theme");
        toggle.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        toggle.setOnCheckedChangeListener(
                (v, on) -> Events.emit(Events.put(Events.event("toggle"), "on", on)));
        root.addView(toggle, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        for (int row = 0; row < 6; row++) {
            LinearLayout line = new LinearLayout(a);
            line.setOrientation(LinearLayout.HORIZONTAL);
            for (int col = 0; col < 4; col++) {
                int n = row * 4 + col;
                String name = "b" + n;
                Button b = new Button(a);
                b.setId(id(a, name));
                b.setText(name);
                b.setAllCaps(false);
                b.setOnTouchListener(
                        (v, ev) -> {
                            if (ev.getActionMasked() == MotionEvent.ACTION_UP) {
                                JSONObject o = Events.event("tap");
                                Events.put(o, "id", name);
                                Events.put(o, "x_dp", dp(v, ev.getRawX()));
                                Events.put(o, "y_dp", dp(v, ev.getRawY()));
                                Events.emit(o);
                            }
                            return false;
                        });
                line.addView(b, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1f));
            }
            root.addView(line, new LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        }
        return root;
    }

    private static EditText field(Activity a, String name, int type, boolean multiline) {
        EditText e = new EditText(a);
        e.setId(id(a, name));
        e.setHint(name);
        e.setInputType(type);
        if (multiline) {
            e.setMinLines(3);
            e.setGravity(android.view.Gravity.TOP);
        }
        e.addTextChangedListener(
                new TextWatcher() {
                    @Override
                    public void beforeTextChanged(CharSequence s, int st, int c, int af) {}

                    @Override
                    public void onTextChanged(CharSequence s, int st, int b, int c) {}

                    @Override
                    public void afterTextChanged(Editable s) {
                        JSONObject o = Events.event("text");
                        Events.put(o, "field", name);
                        if (name.equals("password")) {
                            Events.put(o, "len", s.length());
                        } else {
                            Events.put(o, "value", s.toString());
                        }
                        Events.emit(o);
                    }
                });
        return e;
    }

    static View text(Activity a) {
        LinearLayout root = column(a);
        root.addView(field(a, "plain", InputType.TYPE_CLASS_TEXT, false));
        root.addView(field(a, "multiline",
                InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE, true));
        root.addView(field(a, "password",
                InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD, false));
        return root;
    }

    static View list(Activity a) {
        RecyclerView rv = new RecyclerView(a);
        rv.setId(id(a, "rows"));
        LinearLayoutManager lm = new LinearLayoutManager(a);
        rv.setLayoutManager(lm);
        rv.setAdapter(
                new RecyclerView.Adapter<RecyclerView.ViewHolder>() {
                    @Override
                    public RecyclerView.ViewHolder onCreateViewHolder(ViewGroup p, int type) {
                        TextView t = new TextView(p.getContext());
                        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, 20);
                        t.setPadding(48, 36, 48, 36);
                        t.setLayoutParams(new RecyclerView.LayoutParams(
                                ViewGroup.LayoutParams.MATCH_PARENT,
                                ViewGroup.LayoutParams.WRAP_CONTENT));
                        return new RecyclerView.ViewHolder(t) {};
                    }

                    @Override
                    public void onBindViewHolder(RecyclerView.ViewHolder h, int pos) {
                        ((TextView) h.itemView).setText("Row " + pos);
                    }

                    @Override
                    public int getItemCount() {
                        return 1000;
                    }
                });
        rv.addOnScrollListener(
                new RecyclerView.OnScrollListener() {
                    @Override
                    public void onScrollStateChanged(RecyclerView r, int state) {
                        if (state == RecyclerView.SCROLL_STATE_IDLE) {
                            JSONObject o = Events.event("scroll");
                            Events.put(o, "first", lm.findFirstVisibleItemPosition());
                            Events.put(o, "last", lm.findLastVisibleItemPosition());
                            Events.emit(o);
                        }
                    }
                });
        return rv;
    }

    @SuppressLint("ClickableViewAccessibility")
    static View gestures(Activity a) {
        TextView surface = new TextView(a);
        surface.setId(id(a, "gesture_surface"));
        surface.setText("Gesture surface");
        surface.setGravity(android.view.Gravity.CENTER);
        surface.setOnTouchListener(
                (v, ev) -> {
                    JSONObject o = Events.event("touch");
                    Events.put(o, "action", MotionEvent.actionToString(ev.getActionMasked()));
                    Events.put(o, "t", ev.getEventTime());
                    float ox = ev.getRawX() - ev.getX();
                    float oy = ev.getRawY() - ev.getY();
                    JSONArray ptrs = new JSONArray();
                    for (int i = 0; i < ev.getPointerCount(); i++) {
                        JSONObject p = new JSONObject();
                        Events.put(p, "id", ev.getPointerId(i));
                        Events.put(p, "x_dp", dp(v, ev.getX(i) + ox));
                        Events.put(p, "y_dp", dp(v, ev.getY(i) + oy));
                        ptrs.put(p);
                    }
                    Events.put(o, "ptrs", ptrs);
                    Events.put(o, "index", ev.getActionIndex());
                    Events.emit(o);
                    return true;
                });
        return surface;
    }

    static View busy(Activity a) {
        LinearLayout root = column(a);
        root.setGravity(android.view.Gravity.CENTER);
        ProgressBar bar = new ProgressBar(a);
        bar.setId(id(a, "busy_progress"));
        bar.setIndeterminate(true);
        root.addView(bar);
        TextView spinner = new TextView(a);
        spinner.setText("Spinning");
        spinner.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        ValueAnimator anim = ValueAnimator.ofFloat(0f, 360f);
        anim.setDuration(1000);
        anim.setRepeatCount(ValueAnimator.INFINITE);
        anim.setInterpolator(new android.view.animation.LinearInterpolator());
        anim.addUpdateListener(x -> spinner.setRotation((float) x.getAnimatedValue()));
        root.addOnAttachStateChangeListener(
                new View.OnAttachStateChangeListener() {
                    @Override
                    public void onViewAttachedToWindow(View v) {
                        anim.start();
                    }

                    @Override
                    public void onViewDetachedFromWindow(View v) {
                        anim.cancel();
                    }
                });
        root.addView(spinner);
        return root;
    }

    static View secure(Activity a) {
        TextView t = new TextView(a);
        t.setId(id(a, "secret_label"));
        t.setText("SECRET code 123456");
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, 24);
        t.setGravity(android.view.Gravity.CENTER);
        return t;
    }

    static View dialogs(Activity a) {
        LinearLayout root = column(a);
        Button open = new Button(a);
        open.setId(id(a, "open_dialog"));
        open.setText("Open dialog");
        open.setAllCaps(false);
        open.setOnClickListener(
                v -> new AlertDialog.Builder(a)
                        .setTitle("Delete conversation?")
                        .setMessage("This cannot be undone.")
                        .setPositiveButton("Delete", (d, w) -> dialogResult("delete"))
                        .setNegativeButton("Cancel", (d, w) -> dialogResult("cancel"))
                        .show());
        Button ask = new Button(a);
        ask.setId(id(a, "ask_permission"));
        ask.setText("Ask permission");
        ask.setAllCaps(false);
        ask.setOnClickListener(
                v -> a.requestPermissions(new String[] {Manifest.permission.READ_CONTACTS}, 1));
        root.addView(open);
        root.addView(ask);
        return root;
    }

    private static void dialogResult(String button) {
        Events.emit(Events.put(Events.event("dialog"), "button", button));
    }

    static final class WebBridge {
        @JavascriptInterface
        public void log(String json) {
            Events.emitRaw(json);
        }
    }

    @SuppressLint({"SetJavaScriptEnabled", "JavascriptInterface"})
    static View web(Activity a) {
        WebView w = new WebView(a);
        w.setId(id(a, "web"));
        w.getSettings().setJavaScriptEnabled(true);
        w.addJavascriptInterface(new WebBridge(), "Testbed");
        w.loadUrl("file:///android_asset/form.html");
        return w;
    }

    private Screens() {}
}
