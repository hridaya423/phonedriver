package dev.phonedriver.testbed;

import android.annotation.SuppressLint;
import android.app.Activity;
import android.app.DatePickerDialog;
import android.app.TimePickerDialog;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.WebView;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;

import java.util.Locale;

/** Date and time pickers of each kind an app shows, each opened on 2026-01-15 or 09:05. */
final class Pickers {
    private static TextView shown;

    static void picked(String field, String value) {
        if (shown != null) {
            shown.setText(field + " = " + value);
        }
        Events.emit(Events.put(Events.put(Events.event("picked"), "field", field), "value", value));
    }

    private static Button button(Activity a, String text, View.OnClickListener l) {
        Button b = new Button(a);
        b.setText(text);
        b.setAllCaps(false);
        b.setOnClickListener(l);
        return b;
    }

    private static void date(Activity a, String field, int theme) {
        new DatePickerDialog(
                        a,
                        theme,
                        (v, y, m, d) ->
                                picked(field, String.format(Locale.ROOT, "%04d-%02d-%02d", y, m + 1, d)),
                        2026,
                        0,
                        15)
                .show();
    }

    private static void time(Activity a, String field, int theme, boolean wholeDay) {
        new TimePickerDialog(
                        a,
                        theme,
                        (v, h, m) -> picked(field, String.format(Locale.ROOT, "%02d:%02d", h, m)),
                        9,
                        5,
                        wholeDay)
                .show();
    }

    @SuppressLint({"SetJavaScriptEnabled", "JavascriptInterface"})
    static View screen(Activity a) {
        LinearLayout root = new LinearLayout(a);
        root.setOrientation(LinearLayout.VERTICAL);
        shown = new TextView(a);
        shown.setText("nothing picked");
        root.addView(shown);
        int spinner = android.R.style.Theme_Holo_Light_Dialog;
        LinearLayout dates = new LinearLayout(a);
        dates.addView(button(a, "Date", v -> date(a, "date", 0)));
        dates.addView(button(a, "Date spinner", v -> date(a, "date_spinner", spinner)));
        root.addView(dates);
        LinearLayout times = new LinearLayout(a);
        times.addView(button(a, "Time", v -> time(a, "time", 0, false)));
        times.addView(button(a, "Time 24h", v -> time(a, "time_24h", 0, true)));
        times.addView(button(a, "Time spinner", v -> time(a, "time_spinner", spinner, false)));
        root.addView(times);
        root.addView(ComposePickers.create(a));
        WebView w = new WebView(a);
        w.getSettings().setJavaScriptEnabled(true);
        w.addJavascriptInterface(new Screens.WebBridge(), "Testbed");
        w.loadUrl("file:///android_asset/pickers.html");
        root.addView(w, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        return root;
    }

    private Pickers() {}
}
